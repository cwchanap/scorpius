use scorpius::{
    app::GameScreen,
    campaign::{model::UpgradeLevels, progression::CompletionReceipt},
    domain::{
        board::GridPos,
        model::{BattlePhase, MissionResult, PrimaryObjective, Reaction, UnitId, WeaponId},
    },
    mission::{
        MissionId,
        mission_four::ids as m4,
        mission_one::ids as m1,
        mission_seven::ids as m7,
        mission_six::ids as m6,
        mission_three::{self, ids as m3},
        mission_two::ids as m2,
    },
    presentation::{
        assets::AssetLoadStatus,
        battle_menu::{MenuAction, MenuState},
        campaign_ui::CampaignUiAction,
        interaction::{CommandAction, InteractionMode},
    },
};

/// Capture configuration that is independent of the executable command line.
/// The campaign values are typed so a renamed mission or upgrade field fails
/// this table at compile time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureProfile {
    pub initial_screen: GameScreen,
    pub mission: MissionId,
    pub next_mission: MissionId,
    pub credits: u32,
    pub completed: bool,
    pub upgrades: UpgradePreset,
    pub receipt: Option<CompletionReceipt>,
    pub save: SavePreset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpgradePreset {
    pub vanguard: UpgradeLevels,
    pub gunner: UpgradeLevels,
    pub interceptor: UpgradeLevels,
}

impl UpgradePreset {
    pub const EMPTY: Self = Self {
        vanguard: UpgradeLevels {
            hp: 0,
            armor: 0,
            mobility: 0,
            weapon: 0,
        },
        gunner: UpgradeLevels {
            hp: 0,
            armor: 0,
            mobility: 0,
            weapon: 0,
        },
        interceptor: UpgradeLevels {
            hp: 0,
            armor: 0,
            mobility: 0,
            weapon: 0,
        },
    };

    pub const MAXED: Self = Self {
        vanguard: UpgradeLevels {
            hp: 3,
            armor: 3,
            mobility: 3,
            weapon: 3,
        },
        gunner: UpgradeLevels {
            hp: 3,
            armor: 3,
            mobility: 3,
            weapon: 3,
        },
        interceptor: UpgradeLevels {
            hp: 3,
            armor: 3,
            mobility: 3,
            weapon: 3,
        },
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SavePreset {
    Isolated,
    Error,
}

/// Closed setup variants cover renderer states that cannot be reached by a
/// short authored interaction path, while all user-facing commands remain
/// the production command/action types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureAction {
    Campaign(CampaignUiAction),
    Inspect(UnitId),
    Command(CommandAction),
    Menu(MenuAction),
    ClickCell(GridPos),
    HoverCell(GridPos),
    SetWindow(u32, u32),
    SetBattle(BattleSetup),
    SetAssetStatus(AssetLoadStatus),
    AdvanceRounds(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattleSetup {
    Authored,
    AuthoredWithLog,
    SetUnitHp(UnitId, i16),
    SetBossBelowHp(UnitId, i16),
    Terminal(MissionResult),
    Playback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedFact {
    Screen(GameScreen),
    BattlePhase(BattlePhase),
    Round(u16),
    Menu(MenuState),
    Mode(InteractionMode),
    Inspection(Option<UnitId>),
    Dialogue {
        cursor: usize,
        speaker: &'static str,
        text: &'static str,
    },
    UnitHp(UnitId, i16),
    Primary(PrimaryObjective),
    ObjectiveTrack(ObjectiveFact),
    IntentWeapon(UnitId, WeaponId),
    PilotFocusPending(bool),
    PilotOverdriveActive(bool),
    ReadyToResolve(bool),
    Result(Option<MissionResult>),
    Credits(u32),
    RecentLogEntries(usize),
    PlaybackActive(bool),
    AssetStatus(AssetLoadStatus),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectiveFact {
    EliminateAll {
        remaining: usize,
        total: usize,
    },
    Protect {
        target: UnitId,
        hp: i16,
        max_hp: i16,
        round: u16,
    },
    Intercept {
        target: UnitId,
        distance: u8,
        deadline_round: u16,
        escape: GridPos,
    },
    Target {
        target: UnitId,
        hp: i16,
        max_hp: i16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureFixture {
    pub id: &'static str,
    pub profile: CaptureProfile,
    pub actions: &'static [CaptureAction],
    pub expect: &'static [ExpectedFact],
}

const EMPTY_ACTIONS: &[CaptureAction] = &[];
const ISOLATED: SavePreset = SavePreset::Isolated;
const ERROR_SAVE: SavePreset = SavePreset::Error;
const UPGRADES: UpgradePreset = UpgradePreset::EMPTY;

const fn profile(screen: GameScreen, mission: MissionId) -> CaptureProfile {
    CaptureProfile {
        initial_screen: screen,
        mission,
        next_mission: mission,
        credits: 0,
        completed: false,
        upgrades: UPGRADES,
        receipt: None,
        save: ISOLATED,
    }
}

const fn campaign_profile(
    screen: GameScreen,
    mission: MissionId,
    credits: u32,
    completed: bool,
    save: SavePreset,
) -> CaptureProfile {
    CaptureProfile {
        initial_screen: screen,
        mission,
        next_mission: mission,
        credits,
        completed,
        upgrades: UPGRADES,
        receipt: None,
        save,
    }
}

const fn battle_profile(mission: MissionId) -> CaptureProfile {
    CaptureProfile {
        initial_screen: GameScreen::Battle,
        mission,
        next_mission: mission,
        credits: 0,
        completed: false,
        upgrades: UPGRADES,
        receipt: None,
        save: ISOLATED,
    }
}

const fn aftermission_profile(
    mission: MissionId,
    next_mission: MissionId,
    completed: bool,
    receipt: CompletionReceipt,
) -> CaptureProfile {
    CaptureProfile {
        initial_screen: GameScreen::Aftermath,
        mission,
        next_mission,
        credits: receipt.credits_after,
        completed,
        upgrades: UPGRADES,
        receipt: Some(receipt),
        save: ISOLATED,
    }
}

const RECEIPT_M1: CompletionReceipt = CompletionReceipt {
    mission: MissionId::One,
    base_reward: 300,
    optional_reward: 100,
    total_reward: 400,
    credits_after: 400,
};

const RECEIPT_M7: CompletionReceipt = CompletionReceipt {
    mission: MissionId::Seven,
    base_reward: 1000,
    optional_reward: 300,
    total_reward: 1300,
    credits_after: 3250,
};

const TERMINAL_VICTORY_BONUS: MissionResult = MissionResult {
    victory: true,
    optional_complete: true,
    rounds: 1,
};
const TERMINAL_VICTORY_NO_BONUS: MissionResult = MissionResult {
    victory: true,
    optional_complete: false,
    rounds: 0,
};
const TERMINAL_DEFEAT: MissionResult = MissionResult {
    victory: false,
    optional_complete: false,
    rounds: 0,
};

const TITLE_NO_SAVE_EXPECT: &[ExpectedFact] = &[
    ExpectedFact::Screen(GameScreen::Title),
    ExpectedFact::Credits(0),
];
const BATTLE_IDLE_EXPECT: &[ExpectedFact] = &[
    ExpectedFact::Screen(GameScreen::Battle),
    ExpectedFact::BattlePhase(BattlePhase::Player),
    ExpectedFact::Round(1),
    ExpectedFact::Menu(MenuState::Hidden),
    ExpectedFact::Mode(InteractionMode::Inspect),
    ExpectedFact::Inspection(None),
    ExpectedFact::Primary(PrimaryObjective::EliminateAllEnemies),
    ExpectedFact::ObjectiveTrack(ObjectiveFact::EliminateAll {
        remaining: 4,
        total: 4,
    }),
];
const BATTLE_VANGUARD_EXPECT: &[ExpectedFact] = &[
    ExpectedFact::Screen(GameScreen::Battle),
    ExpectedFact::BattlePhase(BattlePhase::Player),
    ExpectedFact::Menu(MenuState::Root),
    ExpectedFact::Mode(InteractionMode::Inspect),
    ExpectedFact::Inspection(Some(m1::VANGUARD)),
    ExpectedFact::UnitHp(m1::VANGUARD, 20),
];

/// Required capture IDs from the manifest. The table intentionally keeps
/// setup semantics in Rust so JSON remains provenance and ID inventory only.
pub const CAPTURE_FIXTURES: &[CaptureFixture] = &[
    CaptureFixture {
        id: "title-no-save",
        profile: profile(GameScreen::Title, MissionId::One),
        actions: EMPTY_ACTIONS,
        expect: TITLE_NO_SAVE_EXPECT,
    },
    CaptureFixture {
        id: "title-progress-m4",
        profile: campaign_profile(GameScreen::Title, MissionId::Four, 900, false, ISOLATED),
        actions: EMPTY_ACTIONS,
        expect: &[
            ExpectedFact::Screen(GameScreen::Title),
            ExpectedFact::Credits(900),
        ],
    },
    CaptureFixture {
        id: "title-completed",
        profile: campaign_profile(GameScreen::Title, MissionId::Seven, 3250, true, ISOLATED),
        actions: EMPTY_ACTIONS,
        expect: &[
            ExpectedFact::Screen(GameScreen::Title),
            ExpectedFact::Credits(3250),
        ],
    },
    CaptureFixture {
        id: "title-save-error",
        profile: campaign_profile(GameScreen::Title, MissionId::One, 0, false, ERROR_SAVE),
        actions: EMPTY_ACTIONS,
        expect: &[ExpectedFact::Screen(GameScreen::Title)],
    },
    CaptureFixture {
        id: "story-m1-line1",
        profile: profile(GameScreen::PreMissionStory, MissionId::One),
        actions: EMPTY_ACTIONS,
        expect: &[
            ExpectedFact::Screen(GameScreen::PreMissionStory),
            ExpectedFact::Dialogue {
                cursor: 0,
                speaker: "Control",
                text: "Squad, Relay Nine is broadcasting an enemy garrison signal. Four hostiles hold the relay.",
            },
        ],
    },
    CaptureFixture {
        id: "briefing-m1",
        profile: profile(GameScreen::Briefing, MissionId::One),
        actions: EMPTY_ACTIONS,
        expect: &[ExpectedFact::Screen(GameScreen::Briefing)],
    },
    CaptureFixture {
        id: "briefing-m7",
        profile: profile(GameScreen::Briefing, MissionId::Seven),
        actions: EMPTY_ACTIONS,
        expect: &[ExpectedFact::Screen(GameScreen::Briefing)],
    },
    CaptureFixture {
        id: "aftermath-m1-line1",
        profile: aftermission_profile(MissionId::One, MissionId::Two, false, RECEIPT_M1),
        actions: EMPTY_ACTIONS,
        expect: &[
            ExpectedFact::Screen(GameScreen::Aftermath),
            ExpectedFact::Dialogue {
                cursor: 0,
                speaker: "Vanguard",
                text: "Relay Nine is ours. The board is clear and the squad is intact.",
            },
            ExpectedFact::Credits(400),
        ],
    },
    CaptureFixture {
        id: "aftermath-m7-line3",
        profile: aftermission_profile(MissionId::Seven, MissionId::Seven, true, RECEIPT_M7),
        actions: &[
            CaptureAction::Campaign(CampaignUiAction::AdvanceAftermath),
            CaptureAction::Campaign(CampaignUiAction::AdvanceAftermath),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Aftermath),
            ExpectedFact::Dialogue {
                cursor: 2,
                speaker: "Vanguard",
                text: "Copy. Mission complete.",
            },
            ExpectedFact::Credits(3250),
        ],
    },
    CaptureFixture {
        id: "hangar-affordable",
        profile: campaign_profile(GameScreen::Upgrade, MissionId::Two, 400, false, ISOLATED),
        actions: EMPTY_ACTIONS,
        expect: &[
            ExpectedFact::Screen(GameScreen::Upgrade),
            ExpectedFact::Credits(400),
        ],
    },
    CaptureFixture {
        id: "hangar-maxed",
        profile: CaptureProfile {
            upgrades: UpgradePreset::MAXED,
            ..campaign_profile(GameScreen::Upgrade, MissionId::Two, 0, false, ISOLATED)
        },
        actions: EMPTY_ACTIONS,
        expect: &[
            ExpectedFact::Screen(GameScreen::Upgrade),
            ExpectedFact::Credits(0),
        ],
    },
    CaptureFixture {
        id: "hangar-save-error",
        profile: campaign_profile(GameScreen::Upgrade, MissionId::Two, 400, false, ERROR_SAVE),
        actions: &[CaptureAction::Campaign(CampaignUiAction::PurchaseUpgrade(
            scorpius::campaign::model::PlayerMech::Vanguard,
            scorpius::campaign::model::UpgradeTrack::Hp,
        ))],
        expect: &[
            ExpectedFact::Screen(GameScreen::Upgrade),
            ExpectedFact::Credits(400),
        ],
    },
    CaptureFixture {
        id: "ending-complete",
        profile: campaign_profile(GameScreen::Ending, MissionId::Seven, 3250, true, ISOLATED),
        actions: EMPTY_ACTIONS,
        expect: &[
            ExpectedFact::Screen(GameScreen::Ending),
            ExpectedFact::Credits(3250),
        ],
    },
    CaptureFixture {
        id: "battle-idle",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::SetBattle(BattleSetup::Authored),
            CaptureAction::ClickCell(GridPos::new(0, 0)),
        ],
        expect: BATTLE_IDLE_EXPECT,
    },
    CaptureFixture {
        id: "battle-active-vanguard",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::Inspect(m1::VANGUARD)],
        expect: BATTLE_VANGUARD_EXPECT,
    },
    CaptureFixture {
        id: "battle-inspect-enemy",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::Inspect(m1::RIFLEMAN_LEFT)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Inspection(Some(m1::RIFLEMAN_LEFT)),
            ExpectedFact::UnitHp(m1::RIFLEMAN_LEFT, 9),
        ],
    },
    CaptureFixture {
        id: "battle-menu-root",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::Inspect(m1::VANGUARD)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Menu(MenuState::Root),
        ],
    },
    CaptureFixture {
        id: "battle-menu-weapons",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::Inspect(m1::VANGUARD),
            CaptureAction::Menu(MenuAction::Open(MenuState::Weapons)),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Menu(MenuState::Weapons),
        ],
    },
    CaptureFixture {
        id: "battle-menu-stances",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::Inspect(m1::VANGUARD),
            CaptureAction::Menu(MenuAction::Open(MenuState::Stances)),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Menu(MenuState::Stances),
        ],
    },
    CaptureFixture {
        id: "battle-move-targeting",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::Inspect(m1::VANGUARD),
            CaptureAction::Command(CommandAction::Move),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Mode(InteractionMode::Move),
            ExpectedFact::Menu(MenuState::Hidden),
        ],
    },
    CaptureFixture {
        id: "battle-attack-targeting-occupied",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::Inspect(m1::VANGUARD),
            CaptureAction::Command(CommandAction::WeaponSlot(0)),
            CaptureAction::HoverCell(GridPos::new(4, 6)),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Mode(InteractionMode::Attack(m1::PILE_LANCE)),
            ExpectedFact::Inspection(Some(m1::VANGUARD)),
        ],
    },
    CaptureFixture {
        id: "battle-aegis-targeting",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::Inspect(m1::VANGUARD),
            CaptureAction::Command(CommandAction::PilotSkill),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Mode(InteractionMode::AegisTarget),
            ExpectedFact::Menu(MenuState::Hidden),
        ],
    },
    CaptureFixture {
        id: "battle-focus-pending",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::Inspect(m1::GUNNER),
            CaptureAction::Command(CommandAction::PilotSkill),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::PilotFocusPending(true),
            ExpectedFact::Mode(InteractionMode::Inspect),
        ],
    },
    CaptureFixture {
        id: "battle-overdrive-active",
        profile: battle_profile(MissionId::One),
        actions: &[
            CaptureAction::Inspect(m1::INTERCEPTOR),
            CaptureAction::Command(CommandAction::PilotSkill),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::PilotOverdriveActive(true),
            ExpectedFact::Mode(InteractionMode::Inspect),
        ],
    },
    CaptureFixture {
        id: "battle-resolve-ready",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::AdvanceRounds(0)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::ReadyToResolve(true),
            ExpectedFact::BattlePhase(BattlePhase::Player),
        ],
    },
    CaptureFixture {
        id: "battle-playback",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::SetBattle(BattleSetup::Playback)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::PlaybackActive(true),
            ExpectedFact::RecentLogEntries(4),
        ],
    },
    CaptureFixture {
        id: "result-victory-bonus",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::SetBattle(BattleSetup::Terminal(
            TERMINAL_VICTORY_BONUS,
        ))],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::BattlePhase(BattlePhase::Victory),
            ExpectedFact::Result(Some(TERMINAL_VICTORY_BONUS)),
        ],
    },
    CaptureFixture {
        id: "result-victory-no-bonus",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::SetBattle(BattleSetup::Terminal(
            TERMINAL_VICTORY_NO_BONUS,
        ))],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::BattlePhase(BattlePhase::Victory),
            ExpectedFact::Result(Some(TERMINAL_VICTORY_NO_BONUS)),
        ],
    },
    CaptureFixture {
        id: "result-defeat",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::SetBattle(BattleSetup::Terminal(
            TERMINAL_DEFEAT,
        ))],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::BattlePhase(BattlePhase::Defeat),
            ExpectedFact::Result(Some(TERMINAL_DEFEAT)),
        ],
    },
    CaptureFixture {
        id: "result-save-error",
        profile: CaptureProfile {
            save: ERROR_SAVE,
            ..battle_profile(MissionId::One)
        },
        actions: &[
            CaptureAction::SetBattle(BattleSetup::Terminal(TERMINAL_VICTORY_BONUS)),
            CaptureAction::Command(CommandAction::ContinueVictory),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::BattlePhase(BattlePhase::Victory),
        ],
    },
    CaptureFixture {
        id: "asset-loading",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::SetAssetStatus(AssetLoadStatus::Loading)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::AssetStatus(AssetLoadStatus::Loading),
        ],
    },
    CaptureFixture {
        id: "asset-error",
        profile: battle_profile(MissionId::One),
        actions: &[CaptureAction::SetAssetStatus(AssetLoadStatus::Failed(
            "ui/key_art.png",
        ))],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::AssetStatus(AssetLoadStatus::Failed("ui/key_art.png")),
        ],
    },
    CaptureFixture {
        id: "m2-protect-low",
        profile: battle_profile(MissionId::Two),
        actions: &[
            CaptureAction::SetBattle(BattleSetup::SetUnitHp(m2::GUNNER, 5)),
            CaptureAction::Inspect(m2::GUNNER),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::UnitHp(m2::GUNNER, 5),
            ExpectedFact::Primary(PrimaryObjective::ProtectThroughRound {
                target: m2::GUNNER,
                round: 3,
            }),
            ExpectedFact::ObjectiveTrack(ObjectiveFact::Protect {
                target: m2::GUNNER,
                hp: 5,
                max_hp: 15,
                round: 3,
            }),
        ],
    },
    CaptureFixture {
        id: "m2-round-cap",
        profile: battle_profile(MissionId::Two),
        actions: &[CaptureAction::AdvanceRounds(2)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Round(3),
            ExpectedFact::ObjectiveTrack(ObjectiveFact::Protect {
                target: m2::GUNNER,
                hp: 8,
                max_hp: 15,
                round: 3,
            }),
        ],
    },
    CaptureFixture {
        id: "m3-intercept-near",
        profile: battle_profile(MissionId::Three),
        actions: &[CaptureAction::Inspect(m3::COURIER)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Primary(PrimaryObjective::InterceptBeforeEscape {
                target: m3::COURIER,
                escape: mission_three::EXTRACTION,
                deadline_round: 5,
            }),
            ExpectedFact::ObjectiveTrack(ObjectiveFact::Intercept {
                target: m3::COURIER,
                distance: 14,
                deadline_round: 5,
                escape: mission_three::EXTRACTION,
            }),
        ],
    },
    CaptureFixture {
        id: "m3-deadline",
        profile: battle_profile(MissionId::Three),
        actions: &[
            CaptureAction::Inspect(m3::INTERCEPTOR),
            CaptureAction::Command(CommandAction::Move),
            CaptureAction::ClickCell(GridPos::new(8, 8)),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::AdvanceRounds(1),
            CaptureAction::Inspect(m3::INTERCEPTOR),
            CaptureAction::Command(CommandAction::Move),
            CaptureAction::ClickCell(GridPos::new(8, 4)),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::AdvanceRounds(1),
            CaptureAction::Inspect(m3::INTERCEPTOR),
            CaptureAction::Command(CommandAction::Move),
            CaptureAction::ClickCell(mission_three::EXTRACTION),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::Command(CommandAction::Reaction(Reaction::Guard)),
            CaptureAction::Command(CommandAction::FinishUnit),
            CaptureAction::AdvanceRounds(3),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::BattlePhase(BattlePhase::Defeat),
            ExpectedFact::Round(5),
            ExpectedFact::Result(Some(MissionResult {
                victory: false,
                optional_complete: false,
                rounds: 5,
            })),
            ExpectedFact::ObjectiveTrack(ObjectiveFact::Intercept {
                target: m3::COURIER,
                distance: 1,
                deadline_round: 5,
                escape: mission_three::EXTRACTION,
            }),
        ],
    },
    CaptureFixture {
        id: "m4-target-bulwark",
        profile: battle_profile(MissionId::Four),
        actions: &[CaptureAction::Inspect(m4::BULWARK)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Inspection(Some(m4::BULWARK)),
            ExpectedFact::Primary(PrimaryObjective::EliminateTarget {
                target: m4::BULWARK,
            }),
            ExpectedFact::ObjectiveTrack(ObjectiveFact::Target {
                target: m4::BULWARK,
                hp: 16,
                max_hp: 16,
            }),
        ],
    },
    CaptureFixture {
        id: "m5-overlapping-threats",
        profile: battle_profile(MissionId::Five),
        actions: &[CaptureAction::SetBattle(BattleSetup::AuthoredWithLog)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::RecentLogEntries(4),
            ExpectedFact::Primary(PrimaryObjective::EliminateAllEnemies),
        ],
    },
    CaptureFixture {
        id: "m6-boss-high",
        profile: battle_profile(MissionId::Six),
        actions: &[CaptureAction::Inspect(m6::DREADNOUGHT)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Inspection(Some(m6::DREADNOUGHT)),
            ExpectedFact::UnitHp(m6::DREADNOUGHT, 40),
            ExpectedFact::IntentWeapon(m6::DREADNOUGHT, m6::GRAVITON_SALVO),
        ],
    },
    CaptureFixture {
        id: "m6-boss-low",
        profile: battle_profile(MissionId::Six),
        actions: &[
            CaptureAction::SetBattle(BattleSetup::SetBossBelowHp(m6::DREADNOUGHT, 18)),
            CaptureAction::Inspect(m6::DREADNOUGHT),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Inspection(Some(m6::DREADNOUGHT)),
            ExpectedFact::UnitHp(m6::DREADNOUGHT, 16),
            ExpectedFact::IntentWeapon(m6::DREADNOUGHT, m6::OVERLOAD_SALVO),
        ],
    },
    CaptureFixture {
        id: "m7-boss-high",
        profile: battle_profile(MissionId::Seven),
        actions: &[CaptureAction::Inspect(m7::REGENT)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Inspection(Some(m7::REGENT)),
            ExpectedFact::UnitHp(m7::REGENT, 52),
            ExpectedFact::IntentWeapon(m7::REGENT, m7::COMMAND_BARRAGE),
        ],
    },
    CaptureFixture {
        id: "m7-boss-low",
        profile: battle_profile(MissionId::Seven),
        actions: &[
            CaptureAction::SetBattle(BattleSetup::SetBossBelowHp(m7::REGENT, 24)),
            CaptureAction::Inspect(m7::REGENT),
        ],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Inspection(Some(m7::REGENT)),
            ExpectedFact::UnitHp(m7::REGENT, 20),
            ExpectedFact::IntentWeapon(m7::REGENT, m7::RUPTURE_BEAM),
        ],
    },
    CaptureFixture {
        id: "m7-victory-by-round",
        profile: battle_profile(MissionId::Seven),
        actions: &[CaptureAction::AdvanceRounds(5)],
        expect: &[
            ExpectedFact::Screen(GameScreen::Battle),
            ExpectedFact::Round(6),
            ExpectedFact::Primary(PrimaryObjective::EliminateTarget { target: m7::REGENT }),
        ],
    },
    CaptureFixture {
        id: "letterbox-1600x1000",
        profile: profile(GameScreen::Title, MissionId::One),
        actions: &[CaptureAction::SetWindow(1600, 1000)],
        expect: &[ExpectedFact::Screen(GameScreen::Title)],
    },
];

pub fn fixture(id: &str) -> Option<&'static CaptureFixture> {
    CAPTURE_FIXTURES.iter().find(|fixture| fixture.id == id)
}
