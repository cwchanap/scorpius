use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use scorpius::campaign::model::{
    CampaignState, PlayerMech, SquadUpgrades, UpgradeLevels, UpgradeTrack,
};
use scorpius::campaign::progression::{CampaignError, UPGRADE_COSTS};
use scorpius::campaign::save::SaveFile;
use scorpius::campaign::session::{
    CampaignSession, FlowError, complete_current_mission, continue_game, persist_purchase,
    start_new_game,
};
use scorpius::domain::model::MissionResult;
use scorpius::mission::{MissionId, mission_definition, mission_five, mission_four, squad};

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
fn final_mission_completion_is_persisted_and_idempotent() {
    let mut state = CampaignState {
        next_mission: MissionId::Seven,
        credits: 3300,
        upgrades: SquadUpgrades::default(),
        completed: false,
    };
    let definition = mission_definition(MissionId::Seven).unwrap();
    let result = mission_result(true, true);

    let receipt = state.complete_mission(definition, result).unwrap();
    assert_eq!(receipt.total_reward, 1300);
    assert_eq!(state.credits, 4600);
    assert_eq!(state.next_mission, MissionId::Seven);
    assert!(state.completed);

    let snapshot = state.clone();
    assert!(matches!(
        state.complete_mission(definition, result),
        Err(CampaignError::CampaignComplete)
    ));
    assert_eq!(state, snapshot);

    // The terminal completed state round-trips with `completed: true`.
    let save = SaveFile::new(temp_path());
    save.store(&state).unwrap();
    assert_eq!(save.load().unwrap(), Some(state));
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
fn campaign_progresses_through_four_on_base_rewards_alone() {
    let mut state = CampaignState::new_game();
    for (id, base) in [
        (MissionId::One, 300),
        (MissionId::Two, 400),
        (MissionId::Three, 500),
    ] {
        let definition = mission_definition(id).unwrap();
        let receipt = state
            .complete_mission(definition, mission_result(true, false))
            .unwrap();
        assert_eq!(receipt.base_reward, base);
        assert_eq!(receipt.optional_reward, 0);
    }

    assert_eq!(state.credits, 1200);
    assert_eq!(state.next_mission, MissionId::Four);
    // All seven missions are authored as of HPA-386; Seven is the terminal.
    assert!(mission_definition(MissionId::Five).is_some());
    assert!(mission_definition(MissionId::Six).is_some());
    assert!(mission_definition(MissionId::Seven).is_some());
}

#[test]
fn seven_terminal_state_round_trips_with_upgrades_and_credits() {
    let save = SaveFile::new(temp_path());
    let state = CampaignState {
        next_mission: MissionId::Seven,
        credits: 1234,
        upgrades: SquadUpgrades {
            vanguard: UpgradeLevels {
                hp: 1,
                armor: 2,
                mobility: 0,
                weapon: 3,
            },
            gunner: UpgradeLevels::default(),
            interceptor: UpgradeLevels {
                hp: 2,
                armor: 1,
                mobility: 1,
                weapon: 2,
            },
        },
        completed: false,
    };
    save.store(&state).unwrap();
    assert_eq!(save.load().unwrap(), Some(state));
}

#[test]
fn save_at_four_round_trips_purchased_upgrades() {
    let path = temp_path();
    let mut session = CampaignSession::new(SaveFile::new(path.clone()));
    start_new_game(&mut session).unwrap();
    for id in [MissionId::One, MissionId::Two, MissionId::Three] {
        complete_current_mission(
            &mut session,
            mission_definition(id).unwrap(),
            mission_result(true, true),
        )
        .unwrap();
    }
    persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Weapon).unwrap();
    persist_purchase(&mut session, PlayerMech::Gunner, UpgradeTrack::Hp).unwrap();

    let mut resumed = CampaignSession::new(SaveFile::new(path));
    assert_eq!(continue_game(&mut resumed).unwrap(), MissionId::Four);
    let state = resumed.state.as_ref().unwrap();
    assert_eq!(state.next_mission, MissionId::Four);
    assert_eq!(state.upgrades.vanguard.weapon, 1);
    assert_eq!(state.upgrades.gunner.hp, 1);
    // 400+500+650 all-optionals rewards minus two 200-credit level-1 purchases.
    assert_eq!(state.credits, 1150);
}

#[test]
fn upgrades_and_credits_survive_four_five_entry_and_the_six_handoff() {
    let path = temp_path();
    let mut session = CampaignSession::new(SaveFile::new(path.clone()));
    start_new_game(&mut session).unwrap();
    // All-optionals rewards: 400 + 500 + 650 = 1550, then one 200-credit
    // level-1 purchase leaves 1350 banked before Mission 4.
    for id in [MissionId::One, MissionId::Two, MissionId::Three] {
        complete_current_mission(
            &mut session,
            mission_definition(id).unwrap(),
            mission_result(true, true),
        )
        .unwrap();
    }
    persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Weapon).unwrap();
    assert_eq!(session.state.as_ref().unwrap().credits, 1350);

    let mut resumed = CampaignSession::new(SaveFile::new(path.clone()));
    assert_eq!(continue_game(&mut resumed).unwrap(), MissionId::Four);
    let upgrades = resumed.state.as_ref().unwrap().upgrades.clone();

    // Constructing Mission 4 with the reloaded state projects the purchase.
    let battle = mission_four::mission_four_for_campaign(7, &upgrades);
    assert_eq!(
        battle.weapon(squad::ids::REPULSOR_RAM).unwrap().base_damage,
        6,
        "Weapon level 1 lifts the Ram from 5 to 6"
    );

    // 1350 + all-optionals 750 = 2100 at the Mission 5 handoff.
    complete_current_mission(
        &mut resumed,
        mission_definition(MissionId::Four).unwrap(),
        mission_result(true, true),
    )
    .unwrap();
    assert_eq!(resumed.state.as_ref().unwrap().credits, 2100);
    assert_eq!(
        resumed.state.as_ref().unwrap().next_mission,
        MissionId::Five
    );

    // The same state constructs Mission 5 with the upgrade still projected.
    let battle = mission_five::mission_five_for_campaign(7, &upgrades);
    assert_eq!(
        battle.weapon(squad::ids::REPULSOR_RAM).unwrap().base_damage,
        6
    );

    // 2100 + all-optionals 900 = 3000; Mission 5 unlocks the Six handoff.
    complete_current_mission(
        &mut resumed,
        mission_definition(MissionId::Five).unwrap(),
        mission_result(true, true),
    )
    .unwrap();
    assert_eq!(resumed.state.as_ref().unwrap().next_mission, MissionId::Six);

    let mut reloaded = CampaignSession::new(SaveFile::new(path));
    assert_eq!(continue_game(&mut reloaded).unwrap(), MissionId::Six);
    let state = reloaded.state.as_ref().unwrap();
    assert_eq!(state.next_mission, MissionId::Six);
    assert_eq!(state.credits, 3000);
    assert_eq!(state.upgrades.vanguard.weapon, 1);
}

#[test]
fn base_only_run_to_four_with_two_purchases_round_trips_800_credits() {
    let path = temp_path();
    let mut session = CampaignSession::new(SaveFile::new(path.clone()));
    start_new_game(&mut session).unwrap();
    // Base rewards only: 300 + 400 + 500 = 1200.
    for id in [MissionId::One, MissionId::Two, MissionId::Three] {
        complete_current_mission(
            &mut session,
            mission_definition(id).unwrap(),
            mission_result(true, false),
        )
        .unwrap();
    }
    persist_purchase(&mut session, PlayerMech::Gunner, UpgradeTrack::Hp).unwrap();
    persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Weapon).unwrap();

    let mut resumed = CampaignSession::new(SaveFile::new(path));
    assert_eq!(continue_game(&mut resumed).unwrap(), MissionId::Four);
    let state = resumed.state.as_ref().unwrap();
    assert_eq!(state.next_mission, MissionId::Four);
    // 1200 base credits minus two 200-credit level-1 purchases.
    assert_eq!(state.credits, 800);
    assert_eq!(state.upgrades.gunner.hp, 1);
    assert_eq!(state.upgrades.vanguard.weapon, 1);
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
