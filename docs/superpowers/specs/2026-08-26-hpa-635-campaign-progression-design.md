# HPA-635 Mission 1 Campaign and Progression Loop Design

## Context

HPA-632 is complete and the validated Mission 1 combat slice is on `main`. Current `main` is `4c06ded`, which adds Codecov coverage on top of the already-green Rust/Bevy build; it does not change the combat/application seams used here.

HPA-635 wraps the retained battle in the smallest complete linear game loop:

> Title / Continue → short pre-mission VN → briefing → Mission 1 → result/reward → aftermath → mech upgrades → Mission 2 unlocked state.

It also adds one permanent progression model and one signature once-per-mission pilot skill for Vanguard, Gunner, and Interceptor. The delivery remains one implementation PR and one application crate.

The existing seams to preserve are:

- `BattleState` remains canonical plain-Rust combat state.
- `mission::mission_one` remains the owner of Mission 1 board, enemies, opening plan, and deployment.
- `presentation::interaction` remains the battle input adapter.
- native Bevy UI remains the only player-facing UI framework.
- `app.rs` remains the Bevy composition root.

## Non-goals

Do not add a generic mission registry/plugin system, dialogue engine, scripting language, status/ability framework, repository/service layer, extra crate, database/backend, cloud save, multiple save slots, migration compatibility, mid-battle resume, checkpoints, rewind, equipment/parts, ammo progression, SP/morale/pilot levels, branching story, mission select, or Mission 2 combat/content.

## Architecture

### 1. Bevy state belongs at the application/presentation boundary

Use Bevy 0.19 `States` for top-level screen lifecycle:

```rust
#[derive(States, Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum GameScreen {
    #[default]
    Title,
    PreMissionStory,
    Briefing,
    Battle,
    Aftermath,
    Upgrade,
    NextMission,
}
```

`GameScreen` and Bevy resource wrappers live in `app.rs` (or presentation code that is already Bevy-aware), not under `campaign`.

`app.rs` owns `OnEnter`/`OnExit`, battle construction, and `.run_if(in_state(GameScreen::Battle))` gating for existing battle systems. `ScreenRoot` remains separate from `PresentationRoot` so leaving a VN/campaign screen cannot despawn the 3D battlefield.

The `campaign` module stays Bevy-free. It contains plain state, save I/O, progression rules, and save-backed transition helpers only.

### 2. Campaign state is one small JSON document

Persist only stable facts with a current consumer:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CampaignState {
    pub next_mission: MissionId,
    pub credits: u32,
    pub upgrades: SquadUpgrades,
}
```

`CampaignState::new_game()` starts at Mission 1 with zero credits and zero upgrade levels. Do not add a campaign-complete field until a final-mission consumer exists.

One concrete `SaveFile { path: PathBuf }` uses `serde_json`:

- macOS: `$HOME/Library/Application Support/Scorpius/campaign.json`
- Windows: `%APPDATA%/Scorpius/campaign.json`
- Linux/Unix: `$XDG_DATA_HOME/scorpius/campaign.json`, falling back to `$HOME/.local/share/scorpius/campaign.json`

Missing file is `Ok(None)`. Invalid JSON or real I/O failure is an error. New Game deliberately overwrites current pre-release progress. There is no migration/fallback parser.

Stable mutations use copy → mutate → persist → replace so a failed write never advances in-memory state ahead of disk.

### 3. Keep the session plain and use one public flow-error shape

`campaign::session` owns a plain Rust session:

```rust
pub struct CampaignSession {
    pub state: Option<CampaignState>,
    pub save: SaveFile,
    pub last_completion: Option<CompletionReceipt>,
}
```

`CompletionReceipt` is retained because Aftermath displays the exact base/optional/total reward that was persisted. There is no `PurchaseReceipt`: the upgrade UI re-renders from persisted campaign state and currently has no consumer for a separate receipt object.

All four save-backed session calls return `FlowError`, with `SaveError` internal to `save.rs`:

```rust
pub fn start_new_game(session: &mut CampaignSession) -> Result<(), FlowError>;
pub fn continue_game(session: &mut CampaignSession) -> Result<MissionId, FlowError>;
pub fn complete_current_mission(
    session: &mut CampaignSession,
    definition: &MissionDefinition,
    result: MissionResult,
) -> Result<CompletionReceipt, FlowError>;
pub fn persist_purchase(
    session: &mut CampaignSession,
    mech: PlayerMech,
    track: UpgradeTrack,
) -> Result<(), FlowError>;
```

The Bevy layer wraps the session only where needed:

```rust
#[derive(Resource)]
pub struct CampaignRuntime(pub CampaignSession);
```

This keeps campaign model/save/progression/session tests runnable without `App`, `World`, or Bevy scheduling.

### 4. `MissionDefinition` is the complete authored dispatch boundary

Mission identity and authored content live under `mission`. Battle construction is also per-mission authored behavior, so it belongs in the same row rather than being named separately by `app.rs` and restart code.

```rust
pub type MissionBuilder = fn(u64, &SquadUpgrades) -> BattleState;

pub struct MissionDefinition {
    pub id: MissionId,
    pub unlocks: MissionId,
    pub build: MissionBuilder,
    pub title: &'static str,
    pub primary_objective: &'static str,
    pub optional_objective: &'static str,
    pub base_reward: u32,
    pub optional_reward: u32,
    pub pre_mission: DialogueScene,
    pub aftermath: DialogueScene,
}
```

Do not derive `Eq`/`PartialEq` for `MissionDefinition`; comparing function-pointer identity is not a useful content contract. Tests assert the authored fields and exercise `definition.build` instead.

Mission 1 defines:

- `id = MissionId::One`
- `unlocks = MissionId::Two`
- `build = mission_one_for_campaign`
- title: `Mission 1 — Turnabout at Relay Nine`
- primary: `Eliminate all enemies.`
- optional: `Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.`
- base reward: 300 credits
- optional reward: 100 credits

Expose one small dispatch function:

```rust
pub fn mission_definition(id: MissionId) -> Option<&'static MissionDefinition>;
```

HPA-635 returns `Some(&MISSION_ONE_DEFINITION)` for `One` and `None` for `Two` because Mission 2 content is deliberately not authored here. HPA-637 adds the Mission 2/3 rows. This is a typed lookup, not a mission registry framework.

Every growing authored boundary consumes the row:

- battle entry calls `(definition.build)(seed, &upgrades)`;
- restart calls the active row's `build` function;
- completion advances to `definition.unlocks`;
- briefing/HUD/aftermath consume the row's copy/dialogue;
- rewards come from the row.

### 5. Track the mission being played separately from the mission unlocked next

`CampaignState.next_mission` changes as soon as victory completion is persisted, but HUD/result/Aftermath still need the definition for the mission that was actually played. Do not re-derive displayed mission identity from `next_mission` after completion.

At battle entry, resolve the row once and retain it as a Bevy resource:

```rust
#[derive(Resource, Clone, Copy)]
pub struct ActiveMission(pub &'static MissionDefinition);
```

`OnEnter(GameScreen::Battle)`:

1. reads `CampaignRuntime.0.state.next_mission`;
2. resolves `mission_definition(id)`;
3. inserts `ActiveMission(definition)`;
4. builds battle with `(definition.build)(seed, &upgrades)`.

`update_hud`, `restart_battle`, `ContinueVictory`, and Aftermath all read `ActiveMission`. `next_mission` is used for selecting the next battle/story at stable entry points and for the `AlreadyAdvanced` guard—not as a proxy for the mission currently on screen.

`ActiveMission` is transient and never serialized.

### 6. Mission owns deployment; the shared squad builder owns only roster/upgrades

HPA-637 is already a concrete second consumer of the fixed Vanguard/Gunner/Interceptor roster, so extracting the player roster/weapons is justified. Mission-specific cells are not.

Use one small named deployment value:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SquadDeployment {
    pub vanguard: GridPos,
    pub gunner: GridPos,
    pub interceptor: GridPos,
}

pub fn build_player_squad(
    upgrades: &SquadUpgrades,
    deployment: SquadDeployment,
) -> (Vec<UnitState>, Vec<WeaponSpec>);
```

`mission_one.rs` owns:

```rust
const MISSION_ONE_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};
```

The builder uses `SquadUpgrades::levels(PlayerMech)` directly when constructing each player unit/weapon group; it does not remap `UnitArchetype` back to `PlayerMech` after construction.

`mission_one(seed)` remains the zero-upgrade fixture. `mission_one_for_campaign(seed, upgrades)` passes `MISSION_ONE_DEPLOYMENT` to the shared builder, then adds Mission 1 board/enemies/weapons.

### 7. Permanent upgrades are shallow authored arithmetic

Each mech has HP, Armor, Mobility, Weapon tracks with levels 0–3. Costs for purchasing levels 1/2/3 are 200/400/600 credits.

Effects per level:

| Track | Effect |
| --- | --- |
| HP | +3 max HP |
| Armor | +1 armor |
| Mobility | +5 evasion |
| Weapon | +1 base damage to all three owned weapons |

Mobility intentionally changes evasion, not movement. Overdrive is the only HPA-635 mechanic that increases movement range.

`CampaignState::purchase_upgrade` lives in `campaign/progression.rs` beside completion rules. It validates max level and affordability before mutating and returns `Result<(), CampaignError>`. Failed/maxed purchases are no-ops and do not write the save.

### 8. Completion advances exactly once, but the caller supplies the active definition

`complete_current_mission(session, definition, result)` clones campaign state, calls `next.complete_mission(definition, result)`, persists the clone, then replaces the live state and records `last_completion`.

`CampaignState::complete_mission` lives in `campaign/progression.rs` and:

1. requires victory;
2. requires `self.next_mission == definition.id`;
3. grants `definition.base_reward + optional bonus`;
4. sets `self.next_mission = definition.unlocks`;
5. returns the `CompletionReceipt` consumed by Aftermath.

Repeated victory Continue after the first persisted advancement fails `AlreadyAdvanced` and cannot pay twice.

### 9. Pilot skills stay explicit in `BattleState`

Add one battle-local `PilotSkillState`:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PilotSkillState {
    pub aegis_used: bool,
    pub aegis_target: Option<UnitId>,
    pub focus_used: bool,
    pub focus_pending: bool,
    pub overdrive_used: bool,
    pub overdrive_active: bool,
}
```

A fresh `BattleState` always starts with `PilotSkillState::default()`. It is never serialized.

#### Aegis

Vanguard uses it once per mission on a living orthogonally adjacent player ally. The target gets the same 3-point post-armor reduction as Guard for the entire next enemy resolution. Guard + Aegis does not stack.

`resolve_enemy_profile_against` passes whether the target matches `aegis_target` into `incoming_attack_values`.

Cleanup is structural rather than duplicated at each return site:

```rust
pub fn resolve_enemy_phase(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
    let result = self.resolve_enemy_phase_inner();
    if result.is_ok() {
        self.pilot_skills.aegis_target = None;
    }
    result
}
```

The existing phase logic moves unchanged into `resolve_enemy_phase_inner`. Successful normal or terminal exits clear the target once; errors leave it unchanged. `aegis_used` remains true.

#### Focus

Gunner uses it once per mission. The next validated player-initiated Action attack previews/resolves at 100% hit chance and consumes `focus_pending` on commit. Counter always calls attack resolution with `force_hit = false`, never consumes Focus, and never receives its benefit.

#### Overdrive

Interceptor uses it once before Move is spent. `movement_allowance` returns base movement +2 for that activation; `reachable_cells` consumes that allowance. `overdrive_active` clears when the activation finishes; `overdrive_used` remains true.

### 10. Pilot testing proves public resolution paths

Focused tests cover:

- both Mission 1 constructors start with `PilotSkillState::default()`;
- Aegis changes actual `DamageApplied` from `resolve_intent_for_test`;
- Guard + Aegis is still one 3-point reduction;
- one complete `resolve_enemy_phase` clears `aegis_target` and keeps `aegis_used`;
- Focus makes player Action preview/resolution 100% and is consumed on Action commit;
- the same seeded Counter resolution has identical roll/hit/crit with or without Focus pending, and Focus remains pending after Counter;
- Overdrive adds +2 reachable range only during the current activation.

For the Aegis damage test, sweep a small deterministic seed range for a control hit. If no seed hits, set the test defender's evasion to `0` and assert against the resulting deterministic chance instead of leaving a magic-seed hunt in the plan.

### 11. One context-sensitive PILOT command with unambiguous HUD labels

Keep keyboard `P` for the context-sensitive pilot command; `KeyP` is otherwise free in battle input.

The current objective display already uses `[P]` for primary, so remove the pseudo-key labels there. Objectives render as:

```text
PRIMARY  <objective/progress>
BONUS    <optional/progress>
```

The pilot button is the only `[P]` command:

- `[P] AEGIS`
- `[P] FOCUS`
- `[P] OVERDRIVE`

The battle status/control hint is updated at the same time to include `[P] PILOT` alongside Move, weapons, stance, Finish, and Resolve.

HUD shows Aegis/Focus/Overdrive with READY / ACTIVE / USED. No generic ability list or status renderer is introduced.

### 12. Native Bevy UI owns Title/VN/Briefing/Aftermath/Upgrade/NextMission

`presentation::campaign_ui` owns disposable `ScreenRoot` UI built with `Node`, `Text`, pointer buttons, and `ImageNode`.

Checked-in HPA-635 assets are only:

- `assets/vn/relay_nine_bg.png`
- `assets/vn/control_neutral.png`
- `assets/vn/control_alert.png`
- `assets/vn/vanguard_neutral.png`

No asset-generation pipeline is added.

PreMissionStory and Aftermath use the same local dialogue renderer because both are `DialogueScene + DialogueCursor + portrait/speaker/text`. Implement one `spawn_dialogue_screen(commands, asset_server, scene, action)` helper inside `campaign_ui.rs`; the two `OnEnter` hooks choose the scene and the action/next state. This is reuse inside one UI module, not a dialogue framework.

Title uses the plain session functions through `CampaignRuntime`. New Game persists a clean state before entering story. Continue loads stable state; in HPA-635 Mission 1 resumes at pre-story and post-Mission-1 `MissionId::Two` resumes at Upgrade.

Briefing resolves the definition from the stable `next_mission` before battle. HUD/Aftermath use `ActiveMission`. Upgrade rows use persisted state only. NextMission is a saved Mission 2 unlocked summary, not Mission 2 content.

### 13. Battle lifecycle and restart stay at the composition/presentation boundary

`app.rs` stops constructing Mission 1 during plugin build. `OnEnter(GameScreen::Battle)` resolves the current definition, inserts `ActiveMission`, builds through `definition.build`, begins the round, resets transient battle resources, and spawns the existing battlefield/HUD.

Leaving Battle despawns battle presentation/HUD roots and clears transient interaction/playback state.

`restart_battle` reads `ActiveMission` plus current upgrades and calls the active definition's `build` function. Remove `BattleState::restart_mission`; the domain restart test reconstructs with `battle = mission_one(11)` instead. Renderer-free presentation fixtures that call restart insert a default `CampaignRuntime` and `ActiveMission` so restart cannot panic on missing resources.

### 14. Victory Continue is tested at the command boundary

Defeat continues to show Restart only. Victory shows Continue only.

The actual `CommandAction::ContinueVictory` path consumes `ActiveMission.0` and must be covered by renderer-free command tests:

- success: completion/reward persists, session state advances, and `NextState<GameScreen>` becomes Aftermath;
- failed store: current screen remains Battle, in-memory campaign state stays unchanged, and the save error appears in the battle status line.

Aftermath reads `ActiveMission` plus the saved `CompletionReceipt`; it does not try to resolve `session.state.next_mission`, which already points to Mission 2.

## Testing strategy

Use pure Rust tests for campaign model/save/session/progression and most combat rules. Use existing renderer-free Bevy `App`/module tests only for screen scheduling, battle lifecycle, command routing, and UI view-model handoff.

The application scheduling migration gets a manual checkpoint in the task where it happens, not one task later. During the lifecycle task only, `Battle` is temporarily the default state and `CampaignRuntime` is temporarily seeded with `CampaignState::new_game()` so `cargo run` still opens the validated battle. The next UI task changes the default back to `Title` and removes the temporary seed before adding Title/VN/Briefing.

Required regression coverage includes all existing HPA-632 tests plus definition/build/deployment/persistence/progression/pilot/Continue tests.

Final local gates remain:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

GitHub CI keeps the existing `Build + lint` job and coverage-backed `Unit test` job.

Manual acceptance proves New Game → VN → Briefing → Mission 1 → Victory Continue → Aftermath → Upgrade → Mission 2 unlocked, plus process restart/Continue with no duplicate reward.

## Delivery rule

One ticket = one PR. Keep campaign code Bevy-free, application state in the composition/presentation layer, combat in `BattleState`, and authored mission differences in `MissionDefinition` plus each mission module. HPA-637 should add Mission 2/3 definition rows and authored mission modules—not edit application/restart code to teach it how to construct another mission.