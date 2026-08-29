use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use scorpius::campaign::model::{CampaignState, PlayerMech, UpgradeTrack};
use scorpius::campaign::progression::{CampaignError, UPGRADE_COSTS};
use scorpius::campaign::save::SaveFile;
use scorpius::campaign::session::{
    CampaignSession, FlowError, complete_current_mission, continue_game, persist_purchase,
    start_new_game,
};
use scorpius::domain::model::MissionResult;
use scorpius::mission::{MissionId, mission_definition};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

fn temp_path() -> PathBuf {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("scorpius-campaign-{}-{n}.json", std::process::id()))
}

fn mission_result(victory: bool, optional_complete: bool) -> MissionResult {
    MissionResult {
        victory,
        optional_complete,
        rounds: 3,
    }
}

#[test]
fn completion_advances_once_from_the_supplied_definition() {
    let mut state = CampaignState::new_game();
    let definition = mission_definition(MissionId::One).unwrap();
    let result = MissionResult {
        victory: true,
        optional_complete: true,
        rounds: 3,
    };

    let receipt = state.complete_mission(definition, result).unwrap();
    assert_eq!(receipt.total_reward, 400);
    assert_eq!(state.credits, 400);
    assert_eq!(state.next_mission, MissionId::Two);

    let snapshot = state.clone();
    assert!(state.complete_mission(definition, result).is_err());
    assert_eq!(state, snapshot);
}

#[test]
fn defeat_is_rejected_without_state_changes() {
    let mut state = CampaignState::new_game();
    let before = state.clone();
    assert!(matches!(
        state.complete_mission(
            mission_definition(MissionId::One).unwrap(),
            mission_result(false, false)
        ),
        Err(CampaignError::MissionNotWon)
    ));
    assert_eq!(state, before);
}

#[test]
fn base_reward_only_when_turnabout_missed() {
    let mut state = CampaignState::new_game();
    let receipt = state
        .complete_mission(
            mission_definition(MissionId::One).unwrap(),
            mission_result(true, false),
        )
        .unwrap();
    assert_eq!(receipt.base_reward, 300);
    assert_eq!(receipt.optional_reward, 0);
    assert_eq!(receipt.total_reward, 300);
    assert_eq!(receipt.credits_after, 300);
}

#[test]
fn missing_save_loads_none() {
    let save = SaveFile::new(temp_path());
    assert_eq!(save.load().unwrap(), None);
}

#[test]
fn valid_json_round_trips() {
    let save = SaveFile::new(temp_path());
    let mut state = CampaignState::new_game();
    state
        .complete_mission(
            mission_definition(MissionId::One).unwrap(),
            mission_result(true, true),
        )
        .unwrap();
    state
        .purchase_upgrade(PlayerMech::Gunner, UpgradeTrack::Weapon)
        .unwrap();

    save.store(&state).unwrap();
    assert_eq!(save.load().unwrap(), Some(state));
}

#[test]
fn invalid_json_errors() {
    let path = temp_path();
    std::fs::write(&path, b"{ not json").unwrap();
    let save = SaveFile::new(path);
    assert!(save.load().is_err());
}

#[test]
fn upgrade_costs_are_200_400_600() {
    assert_eq!(UPGRADE_COSTS, [200, 400, 600]);
}

#[test]
fn unaffordable_purchase_is_atomic_noop() {
    let mut state = CampaignState::new_game();
    state.credits = 100;
    let before = state.clone();
    assert!(matches!(
        state.purchase_upgrade(PlayerMech::Vanguard, UpgradeTrack::Hp),
        Err(CampaignError::InsufficientCredits {
            required: 200,
            available: 100
        })
    ));
    assert_eq!(state, before);
}

#[test]
fn maxed_purchase_is_atomic_noop() {
    let mut state = CampaignState::new_game();
    state.credits = 9_999;
    state.upgrades.vanguard.hp = 3;
    let before = state.clone();
    assert!(matches!(
        state.purchase_upgrade(PlayerMech::Vanguard, UpgradeTrack::Hp),
        Err(CampaignError::MaxLevel)
    ));
    assert_eq!(state, before);
}

#[test]
fn session_constructors_exist_for_platform_default() {
    let _session = CampaignSession::new(SaveFile::platform_default());
}

#[test]
fn start_new_game_stores_and_continue_resumes() {
    let path = temp_path();
    let mut session = CampaignSession::new(SaveFile::new(path.clone()));
    start_new_game(&mut session).unwrap();
    assert_eq!(session.state.as_ref().unwrap().next_mission, MissionId::One);

    let mut resumed = CampaignSession::new(SaveFile::new(path));
    assert_eq!(continue_game(&mut resumed).unwrap(), MissionId::One);
    assert_eq!(resumed.state.unwrap(), CampaignState::new_game());
}

#[test]
fn new_game_and_continue_clear_stale_last_completion() {
    let mut session = CampaignSession::new(SaveFile::new(temp_path()));
    start_new_game(&mut session).unwrap();
    complete_current_mission(
        &mut session,
        mission_definition(MissionId::One).unwrap(),
        mission_result(true, true),
    )
    .unwrap();
    assert!(session.last_completion.is_some());

    start_new_game(&mut session).unwrap();
    assert!(session.last_completion.is_none());

    complete_current_mission(
        &mut session,
        mission_definition(MissionId::One).unwrap(),
        mission_result(true, true),
    )
    .unwrap();
    assert!(session.last_completion.is_some());

    continue_game(&mut session).unwrap();
    assert!(session.last_completion.is_none());
}

#[test]
fn failed_store_preserves_previous_save() {
    let path = temp_path();
    let save = SaveFile::new(path.clone());
    let state = CampaignState::new_game();
    save.store(&state).unwrap();

    // Occupy the sibling temp path with a directory so the atomic store fails
    // after a healthy save already exists.
    let mut temp = path.clone().into_os_string();
    temp.push(".tmp");
    std::fs::create_dir_all(std::path::PathBuf::from(&temp)).unwrap();

    assert!(save.store(&state).is_err());
    assert_eq!(save.load().unwrap(), Some(state));
}

#[test]
fn continue_without_save_reports_no_active_campaign() {
    let mut session = CampaignSession::new(SaveFile::new(temp_path()));
    assert!(matches!(
        continue_game(&mut session),
        Err(FlowError::NoActiveCampaign)
    ));
}

#[test]
fn successful_purchase_persists_exactly_once() {
    let mut session = CampaignSession::new(SaveFile::new(temp_path()));
    start_new_game(&mut session).unwrap();
    let definition = mission_definition(MissionId::One).unwrap();
    let receipt =
        complete_current_mission(&mut session, definition, mission_result(true, true)).unwrap();
    assert_eq!(receipt.total_reward, 400);

    persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Hp).unwrap();

    let disk = session.save.load().unwrap().unwrap();
    assert_eq!(disk.credits, 200);
    assert_eq!(disk.upgrades.vanguard.hp, 1);
    assert_eq!(Some(disk.clone()), session.state);
    assert_eq!(session.save.load().unwrap(), Some(disk));
}

#[test]
fn failed_completion_leaves_disk_and_memory_unchanged() {
    let mut session = CampaignSession::new(SaveFile::new(temp_path()));
    start_new_game(&mut session).unwrap();
    let definition = mission_definition(MissionId::One).unwrap();
    complete_current_mission(&mut session, definition, mission_result(true, true)).unwrap();

    let memory = session.state.clone().unwrap();
    let disk = session.save.load().unwrap().unwrap();
    assert!(matches!(
        complete_current_mission(&mut session, definition, mission_result(true, true)),
        Err(FlowError::Campaign(CampaignError::AlreadyAdvanced { .. }))
    ));
    assert_eq!(session.state.as_ref().unwrap(), &memory);
    assert_eq!(session.save.load().unwrap(), Some(disk));
    assert_eq!(
        session.last_completion.map(|r| r.mission),
        Some(MissionId::One)
    );
}

#[test]
fn failed_purchase_leaves_disk_and_memory_unchanged() {
    let mut session = CampaignSession::new(SaveFile::new(temp_path()));
    start_new_game(&mut session).unwrap();
    let definition = mission_definition(MissionId::One).unwrap();
    complete_current_mission(&mut session, definition, mission_result(true, true)).unwrap();
    persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Hp).unwrap(); // 400 -> 200

    let memory = session.state.clone().unwrap();
    let disk = session.save.load().unwrap().unwrap();
    assert!(matches!(
        persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Hp),
        Err(FlowError::Campaign(CampaignError::InsufficientCredits {
            required: 400,
            available: 200
        }))
    ));
    assert_eq!(session.state.as_ref().unwrap(), &memory);
    assert_eq!(session.save.load().unwrap(), Some(disk));
}

#[test]
fn failed_purchase_store_leaves_memory_unchanged() {
    // A SaveFile whose parent path is an ordinary file makes store() fail
    // (create_dir_all on a file path errors).
    let blocker = temp_path();
    std::fs::write(&blocker, b"not a directory").unwrap();
    let mut session = CampaignSession::new(SaveFile::new(blocker.join("campaign.json")));
    let mut state = CampaignState::new_game();
    state
        .complete_mission(
            mission_definition(MissionId::One).unwrap(),
            mission_result(true, true),
        )
        .unwrap();
    session.state = Some(state); // 400 credits: the purchase itself is valid

    let memory = session.state.clone().unwrap();
    assert!(matches!(
        persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Hp),
        Err(FlowError::Save(_))
    ));
    assert_eq!(session.state.as_ref().unwrap(), &memory);
    assert_eq!(session.state.as_ref().unwrap().credits, 400);
    assert_eq!(session.state.as_ref().unwrap().upgrades.vanguard.hp, 0);
}
