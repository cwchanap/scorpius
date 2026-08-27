use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use scorpius::app::{GameScreen, enter_battle};
use scorpius::campaign::model::{CampaignState, SquadUpgrades, UpgradeLevels};
use scorpius::campaign::save::SaveFile;
use scorpius::campaign::session::CampaignSession;
use scorpius::mission::MissionId;
use scorpius::mission::mission_one::ids;
use scorpius::presentation::{
    ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
    EventPlayback, SelectedCell,
    interaction::{InteractionState, StatusMessage},
};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

fn temp_save_path(label: &str) -> PathBuf {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "scorpius-flow-{label}-{}-{n}.json",
        std::process::id()
    ))
}

fn init_battle_transients(app: &mut App) {
    app.init_resource::<InteractionState>()
        .init_resource::<StatusMessage>()
        .init_resource::<BattleEventQueue>()
        .init_resource::<EventPlayback>()
        .init_resource::<AttackPreviewCells>()
        .init_resource::<SelectedCell>();
}

#[test]
fn battle_entry_builds_the_active_mission_with_campaign_upgrades() {
    let mut app = App::new();
    app.insert_resource(CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::One,
            credits: 0,
            upgrades: SquadUpgrades {
                vanguard: UpgradeLevels {
                    hp: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        }),
        save: SaveFile::new(temp_save_path("battle-entry")),
        last_completion: None,
    }));
    init_battle_transients(&mut app);
    app.add_systems(Update, enter_battle);

    app.update();

    let active = app.world().resource::<ActiveMission>().0;
    assert_eq!(active.id, MissionId::One);
    let battle = &app.world().resource::<BattleRuntime>().0;
    assert_eq!(battle.unit(ids::VANGUARD).unwrap().stats.max_hp, 23);
    assert_eq!(battle.round(), 1);
}

#[test]
fn default_battle_state_runs_the_battle_lifecycle_at_startup() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin)
        .insert_resource(CampaignRuntime(CampaignSession {
            state: Some(CampaignState::new_game()),
            save: SaveFile::new(temp_save_path("state-entry")),
            last_completion: None,
        }));
    init_battle_transients(&mut app);
    app.init_state::<GameScreen>()
        .add_systems(OnEnter(GameScreen::Battle), enter_battle);

    app.update();

    assert_eq!(
        app.world().resource::<State<GameScreen>>().get(),
        &GameScreen::Battle
    );
    assert_eq!(app.world().resource::<ActiveMission>().0.id, MissionId::One);
    assert_eq!(app.world().resource::<BattleRuntime>().0.round(), 1);
}
