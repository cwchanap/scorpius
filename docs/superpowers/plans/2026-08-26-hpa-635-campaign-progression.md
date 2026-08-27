# HPA-635 Mission 1 Campaign and Progression Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wrap the validated Mission 1 combat slice in one native-Bevy linear campaign loop with JSON Continue, authored 2D VN/briefing/aftermath screens, permanent mech upgrades, and explicit Aegis/Focus/Overdrive pilot skills.

**Architecture:** `campaign` stays Bevy-free and owns plain campaign state, JSON persistence, progression, and save-backed transitions. `app.rs` owns Bevy `GameScreen`, `CampaignRuntime`, `ActiveMission`, and state scheduling. `MissionDefinition` is the complete authored row: copy, rewards, unlock, dialogue, and the function that constructs that mission's `BattleState`; mission-owned deployment stays outside the shared squad builder.

**Tech Stack:** Rust 2024, Bevy `0.19`, native Bevy UI/`ImageNode`, `serde = "1"`, `serde_json = "1"`, checked-in PNG VN assets, existing GitHub Actions `Build + lint` and coverage-backed `Unit test` jobs.

**Spec:** `docs/superpowers/specs/2026-08-26-hpa-635-campaign-progression-design.md`

## Global Constraints

- One ticket = one implementation PR. Continue implementation on this HPA-635 PR; do not create prerequisite infrastructure PRs.
- Every task commit must compile and pass its stated tests; do not intentionally leave a broken intermediate tree.
- Keep one Rust application crate and `bevy = "0.19"`.
- Add only `serde` with `derive` and `serde_json` as new Rust dependencies.
- `src/campaign/**` must not import Bevy.
- Persist only `next_mission`, credits, and fixed-squad upgrade levels.
- No save migrations/version compatibility, multiple slots, cloud sync, mid-battle resume, checkpoints, or rewind.
- Mission 1 reward is 300 credits plus 100 for Turnabout.
- Upgrade costs for levels 1/2/3 are 200/400/600 credits.
- Upgrade effects per level: HP +3 max HP; Armor +1; Mobility +5 evasion; Weapon +1 base damage to the mech's three weapons.
- Aegis is one non-stacking Guard-equivalent 3-point reduction for the next enemy resolution.
- Focus gives one player Action attack 100% hit; Counter never consumes or receives Focus.
- Overdrive gives +2 movement for one Interceptor activation and is the only HPA-635 movement-range increase.
- Keep keyboard `P` for PILOT; objective text must not reuse `[P]` as a pseudo-key label.
- Preserve all HPA-632 behavior except explicit campaign/pilot/screen-lifecycle extensions.
- Final local gates remain `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, and `cargo build --release`.

## File Map

| Path | Responsibility |
| --- | --- |
| `src/campaign/model.rs` | Serialized campaign state and closed mech/upgrade enums |
| `src/campaign/save.rs` | One concrete JSON save file |
| `src/campaign/progression.rs` | Pure `CampaignState` completion/purchase rules |
| `src/campaign/session.rs` | Plain save-backed session transitions; one `FlowError` shape |
| `src/mission/mod.rs` | `MissionId`, dialogue types, `MissionDefinition`, `MissionBuilder`, lookup |
| `src/mission/squad.rs` | Fixed player roster/weapons + upgrade projection; accepts deployment |
| `src/mission/mission_one.rs` | Mission 1 deployment, board, enemies, story/objectives/rewards/build function |
| `src/domain/model.rs` | `PilotSkillState` and pilot errors |
| `src/domain/battle.rs` | Pilot activation/lifetime and Overdrive allowance |
| `src/domain/combat.rs` | Focus hit override and Aegis/Guard reduction |
| `src/domain/enemy.rs` | One structural Aegis cleanup around enemy-phase resolution |
| `src/app.rs` | `GameScreen`, `CampaignRuntime`, `ActiveMission`, Bevy state composition/battle entry |
| `src/presentation/campaign_ui.rs` | Title/VN/briefing/aftermath/upgrade/next UI and shared dialogue renderer |
| `src/presentation/interaction.rs` | PILOT, compile-safe restart evolution, victory Continue command |
| `src/presentation/ui.rs` | Definition-driven HUD copy, unambiguous objective/PILOT labels, terminal actions |
| `tests/campaign_model.rs` | Public mission/campaign authored-data/build contract |
| `tests/campaign_persistence.rs` | JSON, completion, purchase, squad projection |
| `tests/campaign_flow.rs` | Plain session transitions + renderer-free application lifecycle |
| `tests/presentation_app.rs` | Existing battle lifecycle plus pilot/restart interaction |
| `assets/vn/*.png` | Four original Mission 1 VN images |
| `docs/validation/hpa-635.md` | Final acceptance evidence |

---

### Task 1: Add mission definitions and serialized campaign state

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/mission/mod.rs`, `src/mission/mission_one.rs`
- Create: `src/campaign/mod.rs`, `src/campaign/model.rs`, `src/campaign/save.rs`, `src/campaign/progression.rs`, `src/campaign/session.rs`, `src/mission/squad.rs`, `tests/campaign_model.rs`

**Interfaces:**
- Produces: `MissionId`, `MissionBuilder`, `MissionDefinition`, `mission_definition`, `MISSION_ONE_DEFINITION`, `CampaignState`, `SquadUpgrades`, `UpgradeLevels`, `PlayerMech`, `UpgradeTrack`.
- Consumes: existing Mission 1 constructor and authored objective/story semantics.

- [ ] **Step 1: Write the public authored-data/build test**

Create `tests/campaign_model.rs`:

```rust
use scorpius::{
    campaign::model::{CampaignState, SquadUpgrades, UpgradeLevels},
    mission::{MissionId, mission_definition},
};

#[test]
fn new_game_and_mission_one_definition_are_locked() {
    let state = CampaignState::new_game();
    assert_eq!(state.next_mission, MissionId::One);
    assert_eq!(state.credits, 0);
    assert_eq!(state.upgrades.vanguard, UpgradeLevels::default());
    assert_eq!(state.upgrades.gunner, UpgradeLevels::default());
    assert_eq!(state.upgrades.interceptor, UpgradeLevels::default());

    let definition = mission_definition(MissionId::One).unwrap();
    assert_eq!(definition.id, MissionId::One);
    assert_eq!(definition.unlocks, MissionId::Two);
    assert_eq!(definition.title, "Mission 1 — Turnabout at Relay Nine");
    assert_eq!(definition.primary_objective, "Eliminate all enemies.");
    assert_eq!(definition.base_reward, 300);
    assert_eq!(definition.optional_reward, 100);
    assert_eq!(definition.pre_mission.lines.len(), 3);
    assert_eq!(definition.aftermath.lines.len(), 2);

    let battle = (definition.build)(7, &SquadUpgrades::default());
    assert_eq!(battle.board().width(), 9);
    assert_eq!(battle.board().height(), 9);
    assert_eq!(mission_definition(MissionId::Two), None);
}
```

- [ ] **Step 2: Verify red state**

Run: `cargo test --test campaign_model`

Expected: compile failure because campaign/definition APIs do not exist.

- [ ] **Step 3: Add serde dependencies and Bevy-free campaign modules**

`Cargo.toml`:

```toml
[dependencies]
bevy = "0.19"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`src/campaign/mod.rs`:

```rust
pub mod model;
pub mod progression;
pub mod save;
pub mod session;
```

Add `pub mod campaign;` to `src/lib.rs`. Create the later campaign files with module documentation comments only. Do not create `campaign/flow.rs`.

- [ ] **Step 4: Define serialized campaign model before mission builder uses it**

`src/campaign/model.rs` defines:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlayerMech { Vanguard, Gunner, Interceptor }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UpgradeTrack { Hp, Armor, Mobility, Weapon }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UpgradeLevels {
    pub hp: u8,
    pub armor: u8,
    pub mobility: u8,
    pub weapon: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SquadUpgrades {
    pub vanguard: UpgradeLevels,
    pub gunner: UpgradeLevels,
    pub interceptor: UpgradeLevels,
}
```

Implement `UpgradeLevels::level`, `UpgradeLevels::level_mut`, `SquadUpgrades::levels`, and `SquadUpgrades::levels_mut` with exhaustive matches over the closed enums.

Then:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CampaignState {
    pub next_mission: MissionId,
    pub credits: u32,
    pub upgrades: SquadUpgrades,
}

impl CampaignState {
    pub fn new_game() -> Self {
        Self {
            next_mission: MissionId::One,
            credits: 0,
            upgrades: SquadUpgrades::default(),
        }
    }
}
```

- [ ] **Step 5: Define the complete authored mission row**

In `src/mission/mod.rs`:

```rust
use crate::{campaign::model::SquadUpgrades, domain::battle::BattleState};

pub type MissionBuilder = fn(u64, &SquadUpgrades) -> BattleState;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum MissionId { One, Two }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueLine {
    pub speaker: &'static str,
    pub text: &'static str,
    pub portrait: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueScene {
    pub background: &'static str,
    pub lines: &'static [DialogueLine],
}

#[derive(Clone, Copy, Debug)]
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

pub fn mission_definition(id: MissionId) -> Option<&'static MissionDefinition> {
    match id {
        MissionId::One => Some(&mission_one::MISSION_ONE_DEFINITION),
        MissionId::Two => None,
    }
}
```

Do not derive `Eq`/`PartialEq` for `MissionDefinition`; its builder function pointer is exercised, not compared.

- [ ] **Step 6: Add Mission 1 metadata and a temporary campaign-builder wrapper**

In `mission_one.rs`, keep the existing `mission_one(seed)` unchanged for this task and add:

```rust
pub fn mission_one_for_campaign(seed: u64, _upgrades: &SquadUpgrades) -> BattleState {
    mission_one(seed)
}
```

Task 3 replaces this temporary zero-upgrade wrapper with the real upgrade-aware squad construction before any player-facing upgrade flow exists.

Set `MISSION_ONE_DEFINITION.build = mission_one_for_campaign` and add the approved three-line pre-story/two-line aftermath plus:

```rust
pub const MISSION_ONE_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::One,
    unlocks: MissionId::Two,
    build: mission_one_for_campaign,
    title: "Mission 1 — Turnabout at Relay Nine",
    primary_objective: "Eliminate all enemies.",
    optional_objective: "Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.",
    base_reward: 300,
    optional_reward: 100,
    pre_mission: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &PRE_MISSION_LINES,
    },
    aftermath: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &AFTERMATH_LINES,
    },
};
```

- [ ] **Step 7: Verify and commit**

Run:

```bash
cargo test --test campaign_model
cargo test --all-targets
```

Expected: the builder row constructs the unchanged zero-upgrade Mission 1 and all HPA-632 tests remain green.

Commit:

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/campaign src/mission tests/campaign_model.rs
git commit -m "feat: define campaign and mission metadata"
```

---

### Task 2: Implement JSON persistence and plain save-backed transitions

**Files:**
- Modify: `src/campaign/save.rs`, `src/campaign/progression.rs`, `src/campaign/session.rs`
- Create: `tests/campaign_persistence.rs`

**Interfaces:**
- Produces: `SaveFile`, `SaveError`, `CompletionReceipt`, `CampaignError`, `FlowError`, `CampaignSession`, `start_new_game`, `continue_game`, `complete_current_mission`, `persist_purchase`.
- Consumes: caller-supplied active `MissionDefinition` for completion; no Bevy types.

- [ ] **Step 1: Write save/completion/purchase tests**

Create `tests/campaign_persistence.rs` with a unique temp-path helper and cover:

```rust
#[test]
fn completion_advances_once_from_the_supplied_definition() {
    let mut state = CampaignState::new_game();
    let definition = mission_definition(MissionId::One).unwrap();
    let result = MissionResult {
        victory: true,
        turnabout_complete: true,
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
```

Also prove missing save returns `None`, valid JSON round-trips, invalid JSON errors, level costs are 200/400/600, unaffordable/maxed purchases are atomic no-ops, and successful purchase changes persisted state exactly once.

- [ ] **Step 2: Verify red state**

Run: `cargo test --test campaign_persistence`

Expected: compile failure for missing save/progression/session APIs.

- [ ] **Step 3: Implement `SaveFile`**

`SaveFile::load()` returns `Ok(None)` only for `NotFound`; `store()` creates the parent and writes `serde_json::to_vec_pretty`. `platform_default()` uses the exact macOS/Windows/Linux paths from the spec.

Keep `SaveError::{Io, Json}` private to the persistence boundary except through `FlowError::Save`.

- [ ] **Step 4: Implement pure progression in `campaign/progression.rs`**

This file owns the inherent `CampaignState` methods; do not move them back into `model.rs`.

```rust
pub const UPGRADE_COSTS: [u32; 3] = [200, 400, 600];

impl CampaignState {
    pub fn complete_mission(
        &mut self,
        definition: &MissionDefinition,
        result: MissionResult,
    ) -> Result<CompletionReceipt, CampaignError> {
        if !result.victory {
            return Err(CampaignError::MissionNotWon);
        }
        if self.next_mission != definition.id {
            return Err(CampaignError::AlreadyAdvanced {
                expected: definition.id,
                actual: self.next_mission,
            });
        }
        let optional_reward = if result.turnabout_complete {
            definition.optional_reward
        } else {
            0
        };
        let total_reward = definition.base_reward + optional_reward;
        self.credits += total_reward;
        self.next_mission = definition.unlocks;
        Ok(CompletionReceipt {
            mission: definition.id,
            base_reward: definition.base_reward,
            optional_reward,
            total_reward,
            credits_after: self.credits,
        })
    }

    pub fn purchase_upgrade(
        &mut self,
        mech: PlayerMech,
        track: UpgradeTrack,
    ) -> Result<(), CampaignError> {
        let current = self.upgrades.levels(mech).level(track);
        if current >= 3 {
            return Err(CampaignError::MaxLevel);
        }
        let cost = UPGRADE_COSTS[current as usize];
        if self.credits < cost {
            return Err(CampaignError::InsufficientCredits {
                required: cost,
                available: self.credits,
            });
        }
        self.credits -= cost;
        *self.upgrades.levels_mut(mech).level_mut(track) = current + 1;
        Ok(())
    }
}
```

There is no `PurchaseReceipt` until a concrete UI consumer needs one.

- [ ] **Step 5: Implement one `FlowError` shape for all session operations**

`src/campaign/session.rs` has no Bevy imports:

```rust
pub struct CampaignSession {
    pub state: Option<CampaignState>,
    pub save: SaveFile,
    pub last_completion: Option<CompletionReceipt>,
}

#[derive(Debug)]
pub enum FlowError {
    NoActiveCampaign,
    Save(SaveError),
    Campaign(CampaignError),
}
```

Implement:

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

`start_new_game`: create → store → replace memory.

`continue_game`: load → require existing save → replace memory → return loaded `next_mission`.

`complete_current_mission`: clone current → `complete_mission(definition, result)` → store clone → replace memory → store `last_completion`.

`persist_purchase`: clone current → `purchase_upgrade` → store clone → replace memory.

- [ ] **Step 6: Verify save-before-memory semantics**

Add tests proving failed completion/purchase stores leave both disk and `session.state` unchanged, and all four public session functions return `FlowError` rather than mixing `SaveError`/`FlowError` at callers.

Run:

```bash
cargo test --test campaign_persistence
cargo test --all-targets
```

Commit:

```bash
git add src/campaign tests/campaign_persistence.rs
git commit -m "feat: persist campaign progression"
```

---

### Task 3: Extract the upgraded squad and remove `restart_mission` without breaking the tree

**Files:**
- Modify: `src/mission/squad.rs`, `src/mission/mission_one.rs`, `src/domain/battle.rs`, `src/presentation/interaction.rs`, `tests/campaign_persistence.rs`
- Modify existing Mission 1 tests only for moved imports/restart construction.

**Interfaces:**
- Produces: `SquadDeployment`, real `build_player_squad`, real `mission_one_for_campaign`.
- Temporarily keeps `restart_battle` campaign-unaware but compiling by rebuilding zero-upgrade `mission_one(seed)`; Task 4 upgrades it to the active mission definition.

- [ ] **Step 1: Write squad projection/deployment tests**

In `src/mission/squad.rs` tests:

```rust
#[test]
fn upgrades_project_once_onto_supplied_deployment() {
    let upgrades = SquadUpgrades {
        vanguard: UpgradeLevels {
            hp: 2,
            armor: 1,
            mobility: 1,
            weapon: 1,
        },
        ..Default::default()
    };
    let deployment = SquadDeployment {
        vanguard: GridPos::new(0, 0),
        gunner: GridPos::new(1, 0),
        interceptor: GridPos::new(2, 0),
    };
    let (units, weapons) = build_player_squad(&upgrades, deployment);
    let vanguard = units.iter().find(|u| u.id == ids::VANGUARD).unwrap();
    assert_eq!(vanguard.position, GridPos::new(0, 0));
    assert_eq!(vanguard.stats.max_hp, 26);
    assert_eq!(vanguard.stats.armor, 4);
    assert_eq!(vanguard.stats.evasion, 10);
    assert_eq!(vanguard.stats.movement, 3);
    assert_eq!(vanguard.hp, 26);
    assert_eq!(weapons.iter().find(|w| w.id == ids::PILE_LANCE).unwrap().base_damage, 9);
}
```

Add a Mission 1 test asserting upgraded construction still places the three player units at `(4,7)/(3,8)/(5,8)`.

- [ ] **Step 2: Verify red state**

Run:

```bash
cargo test mission::squad
cargo test mission::mission_one
```

- [ ] **Step 3: Implement named deployment plus exact roster/weapon data**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SquadDeployment {
    pub vanguard: GridPos,
    pub gunner: GridPos,
    pub interceptor: GridPos,
}
```

`build_player_squad(upgrades, deployment)` starts from these exact HPA-632 base values:

```text
Vanguard    HP20 Armor3 Move3 Acc78 Eva5  EN7  @ deployment.vanguard
Gunner      HP12 Armor1 Move2 Acc86 Eva10 EN9  @ deployment.gunner
Interceptor HP15 Armor1 Move4 Acc82 Eva20 EN8  @ deployment.interceptor

Pile Lance       range1-1 Single damage8  hit+10 crit15 EN0 pushN counterY
Repulsor Ram     range1-1 Single damage5  hit+15 crit5  EN2 pushY counterN
Anchor Cannon    range2-3 Single damage6  hit+0  crit10 EN3 pushY counterN
Rail Rifle       range3-6 Single damage7  hit+15 crit20 EN0 pushN counterY
Burst Missile    range2-5 Cross1 damage5  hit+5  crit10 EN3 pushN counterN
Overcharge Shot  range2-6 Single damage10 hit-15 crit25 EN5 pushN counterN
Arc Blade        range1-1 Single damage6  hit+15 crit15 EN0 pushN counterN
Pulse Carbine    range2-4 Single damage4  hit+20 crit10 EN1 pushN counterY
Vector Pulse     range1-2 Single damage4  hit+10 crit5  EN3 pushY counterN
```

For each mech, get levels through `upgrades.levels(PlayerMech::...)` before construction. Apply HP/Armor/Evasion/Weapon bonuses exactly once; always initialize `hp=max_hp` and `en=max_en`.

- [ ] **Step 4: Replace the temporary Mission 1 campaign wrapper**

`mission_one.rs` owns:

```rust
const MISSION_ONE_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

pub fn mission_one(seed: u64) -> BattleState {
    mission_one_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_one_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_ONE_DEPLOYMENT);
    units.extend(mission_one_enemy_units());
    weapons.extend(mission_one_enemy_weapons());
    BattleState::new(mission_one_board(), units, weapons, seed)
}
```

Keep Rifleman/Striker/Artillery IDs, their exact stats/weapons, board cells, explosive, hazard, and objective behavior local to Mission 1. Re-export player IDs through `mission_one::ids` to minimize HPA-632 test churn.

- [ ] **Step 5: Rewrite every `restart_mission` caller before deleting the method**

In `src/domain/battle.rs::victory_failure_and_restart_are_clean`:

```rust
battle = mission_one(11);
```

In `src/presentation/interaction.rs::restart_battle`, replace the current call:

```rust
world.resource_mut::<BattleRuntime>().0.restart_mission(seed);
```

with the compile-safe zero-upgrade replacement:

```rust
world.resource_mut::<BattleRuntime>().0 = mission_one(seed);
```

Keep the existing presentation-root/transient cleanup unchanged. Only after both callers compile, remove `BattleState::restart_mission` from `mission_one.rs`.

Task 4 replaces this temporary zero-upgrade restart construction with the active campaign definition/upgrades.

- [ ] **Step 6: Verify this intermediate commit is healthy**

Run:

```bash
cargo test mission::squad
cargo test mission::mission_one
cargo test domain::battle::tests::victory_failure_and_restart_are_clean
cargo test --test presentation_app restart_replaces_presentation_root_and_transient_state
cargo test --all-targets
```

Expected: the tree compiles after `restart_mission` removal; zero-upgrade HPA-632 restart behavior is unchanged.

Commit:

```bash
git add src/mission src/domain/battle.rs src/presentation/interaction.rs tests/campaign_persistence.rs
git commit -m "feat: project upgrades into mission-owned deployment"
```

---

### Task 4: Move battle lifecycle to Bevy States and pin the scheduling migration immediately

**Files:**
- Modify: `src/app.rs`, `src/presentation/mod.rs`, `src/presentation/interaction.rs`, `tests/presentation_app.rs`
- Create: `tests/campaign_flow.rs`

**Interfaces:**
- Produces: `GameScreen`, `CampaignRuntime`, `ActiveMission`, definition-driven `OnEnter(Battle)`, campaign-aware restart.
- Consumes: plain `CampaignSession`, `mission_definition`, `MissionDefinition.build`.

- [ ] **Step 1: Add renderer-free application lifecycle test**

Construct a minimal Bevy `App` with:

```rust
CampaignRuntime(CampaignSession {
    state: Some(CampaignState {
        next_mission: MissionId::One,
        credits: 0,
        upgrades: SquadUpgrades {
            vanguard: UpgradeLevels { hp: 1, ..Default::default() },
            ..Default::default()
        },
    }),
    save: SaveFile::new(temp_save_path("battle-entry")),
    last_completion: None,
})
```

Run the battle-entry system and assert:

```rust
let active = app.world().resource::<ActiveMission>().0;
assert_eq!(active.id, MissionId::One);
let battle = &app.world().resource::<BattleRuntime>().0;
assert_eq!(battle.unit(ids::VANGUARD).unwrap().stats.max_hp, 23);
assert_eq!(battle.round(), 1);
```

- [ ] **Step 2: Temporarily make Battle the default state for this migration checkpoint**

In Task 4 only:

```rust
#[derive(States, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GameScreen {
    Title,
    PreMissionStory,
    Briefing,
    #[default]
    Battle,
    Aftermath,
    Upgrade,
    NextMission,
}

#[derive(Resource)]
pub struct CampaignRuntime(pub CampaignSession);

#[derive(Resource, Clone, Copy)]
pub struct ActiveMission(pub &'static MissionDefinition);
```

Seed a temporary Task-4 runtime so default Battle has an active campaign:

```rust
let mut campaign = CampaignSession::new(SaveFile::platform_default());
campaign.state = Some(CampaignState::new_game());
```

Insert `CampaignRuntime(campaign)`. Task 5 removes this temporary in-memory New Game seed and moves `#[default]` back to Title.

- [ ] **Step 3: Convert eager startup into definition-driven battle entry**

Remove eager `mission_one(...)` construction from plugin build.

`OnEnter(GameScreen::Battle)` resolves the active row once:

```rust
let state = runtime.0.state.as_ref().expect("Battle requires active campaign");
let definition = mission_definition(state.next_mission)
    .expect("current mission must have authored definition");
commands.insert_resource(ActiveMission(definition));
let mut battle = (definition.build)(fresh_seed(), &state.upgrades);
battle.begin_round().expect("authored mission opening must be valid");
commands.insert_resource(BattleRuntime(battle));
```

Reset the same transient interaction/playback/preview/selection resources currently cleared by restart, then spawn the existing battlefield/HUD.

Gate every battle update group—restart/rebuild, telegraph/reaction reconciliation, transforms/highlights, playback, battle input, and HUD—with `.run_if(in_state(GameScreen::Battle))`. Leave asset monitoring and window-position stabilization global.

- [ ] **Step 4: Make restart use `ActiveMission` rather than naming Mission 1**

Replace Task 3's temporary `mission_one(seed)` restart construction with:

```rust
let definition = world.resource::<ActiveMission>().0;
let upgrades = world
    .resource::<CampaignRuntime>()
    .0
    .state
    .as_ref()
    .expect("restart requires active campaign")
    .upgrades
    .clone();
world.resource_mut::<BattleRuntime>().0 = (definition.build)(seed, &upgrades);
```

Preserve existing root/transient cleanup and `RestartRoundPending` behavior.

- [ ] **Step 5: Fix the renderer-free restart fixture with both required resources**

`tests/presentation_app.rs::presentation_fixture_app()` inserts:

```rust
CampaignRuntime(CampaignSession {
    state: Some(CampaignState::new_game()),
    save: SaveFile::new(temp_save_path("presentation-restart")),
    last_completion: None,
}),
ActiveMission(mission_definition(MissionId::One).unwrap()),
```

Keep all existing restart assertions.

- [ ] **Step 6: Verify automated lifecycle coverage**

Run:

```bash
cargo test --test campaign_flow
cargo test --test presentation_app
cargo test --all-targets
```

- [ ] **Step 7: Run the real scheduling checkpoint before moving on**

Run: `cargo run`

Because Battle is temporarily default, verify in the real window before Task 5:

1. 3D board renders with the existing Mission 1 units/telegraphs;
2. HUD/objective/threat text updates;
3. select a mech and complete one normal Move/Action/stance sequence;
4. finish all three player units and resolve one enemy phase;
5. restart a terminal result and verify round/state reset.

If any battle system was incorrectly gated, fix it in Task 4 before committing.

Commit:

```bash
git add src/app.rs src/presentation tests/campaign_flow.rs tests/presentation_app.rs
git commit -m "feat: add campaign-aware battle lifecycle"
```

---

### Task 5: Add Title → VN → Briefing and restore Title as the real default

**Files:**
- Create: `src/presentation/campaign_ui.rs`, four `assets/vn/*.png`
- Modify: `src/presentation/mod.rs`, `src/app.rs`, `tests/campaign_flow.rs`

**Interfaces:**
- Consumes: `CampaignRuntime`, `GameScreen`, plain `start_new_game`/`continue_game`, `mission_definition`.
- Produces: `ScreenRoot`, title actions, shared dialogue renderer, briefing UI.

- [ ] **Step 1: Add copy/expression tests**

Test `briefing_copy(definition)` contains title, primary, optional, `300 credits`, and `+100 credits`. Test pre-story line 0 vs line 2 swaps `control_neutral.png` → `control_alert.png`.

- [ ] **Step 2: Add exactly four checked-in original PNGs**

```text
assets/vn/relay_nine_bg.png        1280×720
assets/vn/control_neutral.png      512×512
assets/vn/control_alert.png        512×512
assets/vn/vanguard_neutral.png     512×512
```

No generation scripts, prompt manifests, catalogs, or art pipeline.

- [ ] **Step 3: Implement one local dialogue renderer used by both story screens**

`campaign_ui.rs` defines `ScreenRoot`, `CampaignStatus`, `DialogueCursor`, `CampaignUiAction`, `DialogueSnapshot`, and:

```rust
fn spawn_dialogue_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    scene: &DialogueScene,
    advance_action: CampaignUiAction,
) {
    // one ScreenRoot containing background ImageNode, portrait ImageNode,
    // speaker Text, dialogue Text, and one advance button
}
```

The helper is private to `campaign_ui.rs`; it is not a dialogue engine. `setup_pre_mission_story` calls it with `definition.pre_mission`. Task 8 reuses the same helper for `ActiveMission.0.aftermath`.

`dialogue_snapshot(scene, cursor)` returns exact speaker/text/portrait values and drives the mutable portrait/speaker/dialogue nodes.

- [ ] **Step 4: Remove Task-4 temporary startup state**

Change `GameScreen` to its final form with `#[default] Title`.

Replace the temporary seeded campaign with:

```rust
CampaignRuntime(CampaignSession::new(SaveFile::platform_default()))
```

No campaign state exists until New Game or Continue succeeds.

- [ ] **Step 5: Route Title actions through the unified `FlowError` API**

`NewGame`:

```rust
match start_new_game(&mut runtime.0) {
    Ok(()) => next_state.set(GameScreen::PreMissionStory),
    Err(error) => status.0 = error.to_string(),
}
```

`Continue`:

```rust
match continue_game(&mut runtime.0) {
    Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
    Ok(MissionId::Two) => next_state.set(GameScreen::Upgrade),
    Err(error) => status.0 = error.to_string(),
}
```

Missing save disables Continue; invalid JSON disables it and shows the same `FlowError` shape.

- [ ] **Step 6: Implement pre-story and briefing from stable `next_mission`**

Before Battle, resolving `runtime.0.state.next_mission` is correct because completion has not advanced it yet.

Pre-story and Briefing resolve `mission_definition(id)` and use that row. Briefing copy:

```text
Mission 1 — Turnabout at Relay Nine
PRIMARY
Eliminate all enemies.
BONUS
Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.
REWARD
300 credits
BONUS +100 credits
```

START MISSION sets `GameScreen::Battle` only; `OnEnter(Battle)` owns battle construction.

- [ ] **Step 7: Verify entry flow**

Run:

```bash
cargo test --test campaign_flow
cargo test --all-targets
cargo run
```

Manual checkpoint: Title → New Game → three-line VN with Control expression swap → Briefing → Start Mission → existing 3D battle.

Commit:

```bash
git add src/presentation/campaign_ui.rs src/presentation/mod.rs src/app.rs assets/vn tests/campaign_flow.rs
git commit -m "feat: add title story and briefing flow"
```

---

### Task 6: Implement pilot rules with one structural Aegis cleanup

**Files:**
- Modify: `src/domain/model.rs`, `src/domain/battle.rs`, `src/domain/combat.rs`, `src/domain/enemy.rs`, `src/mission/mission_one.rs`

**Interfaces:**
- Produces: `PilotSkillState`, `pilot_skills`, `movement_allowance`, `use_aegis`, `use_focus`, `use_overdrive`.
- Consumes: existing `resolve_intent_for_test`, `resolve_enemy_phase`, `resolve_counter`, `reachable_cells`, attack preview/resolution.

- [ ] **Step 1: Add activation/precondition and fresh-state tests**

Cover:

- Aegis requires active Vanguard + living adjacent ally and is once per mission;
- Focus requires active Gunner and sets `focus_pending`/`focus_used`;
- Overdrive requires active Interceptor before Move, changes allowance 4 → 6, and clears active on Finish;
- `mission_one(7).pilot_skills() == PilotSkillState::default()`;
- `mission_one_for_campaign(7, &SquadUpgrades::default()).pilot_skills() == PilotSkillState::default()`.

- [ ] **Step 2: Add public-path Aegis damage test with deterministic fallback**

Use the authored left Rifleman intent because it targets Gunner. Compare `DamageApplied` from `resolve_intent_for_test(ids::RIFLEMAN_LEFT)` with and without Aegis.

First sweep seeds `0..64` until the control intent hits. If no seed hits, set Gunner's fixture evasion to `0` in both otherwise-identical battles and use the next fixed seed; do not leave implementation blocked on finding a magic seed.

Expected successful-hit damage:

```rust
assert_eq!(control_damage, 4); // Service Rifle 5 - Gunner Armor 1
assert_eq!(aegis_damage, 1);   // same attack - one 3-point reduction
```

Also extend the existing incoming-value Guard test to prove Guard + Aegis equals Guard only; this private-helper test is supplemental, not the Aegis gameplay test.

- [ ] **Step 3: Add one full-phase Aegis expiry test**

Finish the player squad, call `resolve_enemy_phase`, then assert:

```rust
assert_eq!(battle.pilot_skills().aegis_target, None);
assert!(battle.pilot_skills().aegis_used);
```

Do not create a second low-HP terminal fixture solely to chase another return site; Step 7 removes duplicated cleanup sites structurally.

- [ ] **Step 4: Add Focus Action and Counter-isolation tests**

Action: Focus makes Gunner preview 100%; validated player Action consumes `focus_pending`; `focus_used` remains true.

Counter: two otherwise identical battles use the same seed and Gunner Counter; only one has Focus pending. Resolve the same Rifleman intent and compare Gunner→Rifleman `AttackRolled` roll/hit/crit fields. They must be identical, and Focus remains pending after Counter.

- [ ] **Step 5: Implement `PilotSkillState` and focused errors**

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

Add `PilotSkillWrongUnit`, `PilotSkillAlreadyUsed`, `InvalidAegisTarget`, and `PilotSkillRequiresMoveAvailable` errors with concise `Display` text.

`BattleState::new` initializes the default state. `movement_allowance` is base +2 only for active Interceptor Overdrive. `reachable_cells` consumes that allowance. `finish_activation` clears only `overdrive_active`.

- [ ] **Step 6: Wire Focus through the single player hit-value seam**

Change:

```rust
fn attack_values(
    attacker: &UnitState,
    weapon: &WeaponSpec,
    defender: &UnitState,
    force_hit: bool,
) -> AttackValues
```

Use `100` only when `force_hit`; otherwise preserve the existing 5–95 formula.

`preview_attack` and player `attack` read Focus. `attack` clears `focus_pending` only after all validation succeeds and the Action is committed. `resolve_counter` always passes `false` and never mutates Focus state.

- [ ] **Step 7: Wire Aegis once and clean it once**

`incoming_attack_values(profile, defender, aegis_guarded)` uses one 3-point reduction when Guard **or** Aegis is present.

`resolve_enemy_profile_against` passes `self.pilot_skills.aegis_target == Some(target)`.

Move the current implementation body into a private helper and wrap it:

```rust
pub fn resolve_enemy_phase(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
    let result = self.resolve_enemy_phase_inner();
    if result.is_ok() {
        self.pilot_skills.aegis_target = None;
    }
    result
}

fn resolve_enemy_phase_inner(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
    // existing phase validation / intent loop / begin_round logic
}
```

Do not add cleanup before each successful early return. Errors keep the target; every successful exit clears it exactly once. Never clear `aegis_used`.

- [ ] **Step 8: Verify and commit**

Run:

```bash
cargo test domain
cargo test mission::mission_one
cargo test --all-targets
```

Commit:

```bash
git add src/domain src/mission/mission_one.rs
git commit -m "feat: add signature pilot skills"
```

---

### Task 7: Wire PILOT and make the HUD consume `ActiveMission`

**Files:**
- Modify: `src/presentation/interaction.rs`, `src/presentation/ui.rs`, `tests/presentation_app.rs`

**Interfaces:**
- Produces: `InteractionMode::AegisTarget`, `CommandAction::PilotSkill`, keyboard `P`, pilot HUD state, unambiguous objective labels.
- Consumes: `ActiveMission` and `HudSnapshot::from_battle(..., definition)`.

- [ ] **Step 1: Add Aegis/Focus/Overdrive interaction tests**

Use the existing renderer-free `route_cell_click`/`execute_command` seams:

- Vanguard PILOT arms `AegisTarget`; clicking adjacent Gunner sets `aegis_target` and returns Inspect.
- Gunner PILOT sets `focus_pending`.
- Interceptor PILOT changes movement allowance 4 → 6.

- [ ] **Step 2: Change `HudSnapshot` to accept the authored row explicitly**

```rust
pub fn from_battle(
    battle: &BattleState,
    selected: Option<UnitId>,
    definition: &MissionDefinition,
) -> Self
```

Use:

```text
PRIMARY  <definition.primary_objective> · <runtime progress>
BONUS    <definition.optional_objective> · Complete|Not yet
```

Do not render `[P]` or `[B]` prefixes for objectives. Update existing HUD tests to pass `mission_definition(MissionId::One).unwrap()` explicitly.

- [ ] **Step 3: Add one unambiguous `[P] PILOT` binding and update the control hint**

Add `CommandAction::PilotSkill` and `InteractionMode::AegisTarget`. Map `KeyCode::KeyP`.

Dynamic button labels:

```text
[P] AEGIS
[P] FOCUS
[P] OVERDRIVE
```

Update the existing battle hint line in the same step to include `[P] PILOT`, for example:

```text
[M] MOVE  [1-3] WEAPONS  [P] PILOT  [C/G/E] STANCE  [F] FINISH  [SPACE] RESOLVE
```

No other HUD element may use `[P]` as a key hint.

- [ ] **Step 4: Make runtime HUD use `ActiveMission`, never `next_mission`**

`update_hud` reads:

```rust
active_mission: Res<ActiveMission>
```

and calls:

```rust
HudSnapshot::from_battle(&battle.0, interaction.selected_unit, active_mission.0)
```

It must not resolve `CampaignRuntime.0.state.next_mission`; that value may already be Mission 2 during the final Battle frame after Continue.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo test --test presentation_app
cargo test --all-targets
```

Run `cargo run` and confirm objectives read PRIMARY/BONUS while `[P]` appears only on the pilot command/hint.

Commit:

```bash
git add src/presentation tests/presentation_app.rs
git commit -m "feat: expose pilot skills and mission HUD"
```

---

### Task 8: Complete Victory Continue, Aftermath, upgrades, and Mission 2 handoff

**Files:**
- Modify: `src/presentation/interaction.rs`, `src/presentation/ui.rs`, `src/presentation/campaign_ui.rs`, `src/app.rs`, `tests/campaign_flow.rs`
- Add unit tests inside `src/presentation/interaction.rs` for the private command path.

**Interfaces:**
- Consumes: `ActiveMission`, plain `complete_current_mission`, `persist_purchase`, terminal `MissionResult`.
- Produces: save-backed `CommandAction::ContinueVictory`, Aftermath, Upgrade, NextMission.

- [ ] **Step 1: Add actual ContinueVictory success/failure command tests**

Use the same private `run_command` path used by button/keyboard adapters.

Success fixture contains:

```rust
GameScreen::Battle
CampaignRuntime with persisted MissionId::One state
ActiveMission(mission_definition(MissionId::One).unwrap())
terminal victorious BattleRuntime
```

Invoke `ContinueVictory`, then assert:

```text
disk and runtime next_mission = MissionId::Two
credits = authored 300 or 400
ActiveMission still points to MissionId::One
NextState<GameScreen> = Aftermath
```

Failure fixture uses a `SaveFile` whose parent path is an ordinary file so `store()` fails. Invoke the same command and assert:

```text
current GameScreen remains Battle
CampaignRuntime state remains MissionId::One with original credits
ActiveMission remains MissionId::One
StatusMessage contains the save error
```

- [ ] **Step 2: Implement result-specific terminal actions using `ActiveMission`**

Defeat: Restart visible/enabled, Continue hidden/disabled.

Victory: Continue visible/enabled, Restart hidden/disabled.

On Continue:

```rust
let result = battle.result().filter(|result| result.victory)
    .ok_or(/* existing terminal/phase error */)?;
complete_current_mission(&mut runtime.0, active_mission.0, result)?;
next_state.set(GameScreen::Aftermath);
```

On error, keep Battle/current campaign state and show the error. Do not look up the mission from `runtime.0.state.next_mission` after completion.

- [ ] **Step 3: Reuse Task-5 dialogue renderer for Aftermath**

`setup_aftermath_screen` calls the same private `spawn_dialogue_screen(...)` helper with:

```rust
active_mission.0.aftermath
```

It also displays `runtime.0.last_completion` exactly as persisted:

```text
MISSION REWARD
Base 300
Turnabout +0|+100
Total 300|400
Credits <credits_after>
```

Do not recalculate/grant reward in UI. Advancing past the final line sets Upgrade.

- [ ] **Step 4: Implement fixed 3×4 upgrade screen without a purchase receipt**

Rows show level/current effect/next effect/cost/MAX from `runtime.0.state` and `UPGRADE_COSTS`.

Purchase buttons call:

```rust
persist_purchase(&mut runtime.0, mech, track)
```

On success, re-read the persisted level/credits from `runtime.0.state` and update the row/status. On error, show `FlowError` and leave the displayed persisted state unchanged. Do not add UI-local optimistic credits/levels or `PurchaseReceipt`.

PROCEED is always enabled and sets NextMission.

- [ ] **Step 5: Implement minimal NextMission handoff**

Display:

```text
MISSION 2 UNLOCKED
Campaign progress saved.
Credits: <current>
Vanguard <four levels>
Gunner <four levels>
Interceptor <four levels>
```

Return to Title changes only `GameScreen`; it does not write the save. No Mission 2 definition/content is added in HPA-635.

- [ ] **Step 6: Register remaining screen lifecycle**

Register `OnEnter`/`OnExit` for Aftermath, Upgrade, NextMission using the shared `ScreenRoot` cleanup. No Bevy state machine code goes under `campaign`.

- [ ] **Step 7: Verify full flow**

Run:

```bash
cargo test --test campaign_flow
cargo test --test presentation_app
cargo test --all-targets
cargo run
```

Manual checkpoint:

`New Game → VN → Briefing → Mission 1 → Victory Continue → Aftermath → buy one upgrade → Mission 2 unlocked`.

Restart process, Continue, verify Upgrade opens with saved state and no duplicate Mission 1 reward.

Commit:

```bash
git add src/app.rs src/presentation tests/campaign_flow.rs
git commit -m "feat: complete Mission 1 campaign loop"
```

---

### Task 9: Close HPA-635 acceptance and CI

**Files:**
- Modify: `README.md`
- Create: `docs/validation/hpa-635.md`
- Modify source/tests/assets only for concrete failures discovered by gates.

- [ ] **Step 1: Update README**

Document:

- Title default flow and New Game/Continue semantics;
- platform save paths;
- no mid-battle resume/migrations;
- Mission 1 300/+100 reward;
- upgrade costs 200/400/600 and all four effects;
- `[P]` PILOT behavior and PRIMARY/BONUS objective labels;
- Mission 2 screen as HPA-637 handoff only.

- [ ] **Step 2: Run all local gates**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Fix only concrete failures, then rerun the complete failed gate.

- [ ] **Step 3: Perform clean release playthrough**

Run `cargo run --release` with a known clean/disposable save and record:

- Title/Continue availability;
- VN background/portrait and Control neutral→alert swap;
- definition-driven briefing objectives/rewards;
- Mission 1 completion;
- one observed Aegis, Focus, Overdrive use and restart reset;
- reward receipt;
- one successful upgrade and persisted state;
- Mission 2 unlocked screen;
- process restart + Continue without duplicate reward.

- [ ] **Step 4: Record failure/no-op evidence**

Record exact tests proving defeat does not progress, invalid Aegis target is no-op, unaffordable/maxed purchases are no-op, failed Continue save leaves Battle/state unchanged, and post-Mission-1 Continue resumes Upgrade.

- [ ] **Step 5: Build acceptance matrix from live HPA-635**

For every Linear checkbox, cite an exact test/source seam/manual observation/CI job. Do not mark unsupported criteria complete.

- [ ] **Step 6: Inspect final scope**

```bash
git status --short
git diff --check main...HEAD
git diff --stat main...HEAD
```

Expected: one application crate, serde/JSON, focused campaign/mission/presentation changes, four VN images, tests/docs; no backend, second UI framework, generic dialogue/status/mission engine, Mission 2 content, save migration, or unrelated refactor.

- [ ] **Step 7: Commit validation and rerun exact-HEAD gates**

```bash
git add README.md docs/validation/hpa-635.md src tests assets Cargo.toml Cargo.lock
git commit -m "docs: validate HPA-635 campaign loop"

cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Push that exact HEAD and require GitHub Actions `Build + lint` plus coverage-backed `Unit test` to succeed before HPA-635 is Done.

---

## Plan Self-Review Checklist

- `campaign` contains no Bevy imports.
- All four session operations return `FlowError`; `SaveError` stays behind the save/session boundary.
- `MissionDefinition` contains `unlocks` **and** `build`; app/restart never name `mission_one_for_campaign`.
- `ActiveMission` stores the row actually used to build Battle; HUD/restart/Continue/Aftermath do not re-derive it from advanced `next_mission`.
- `MissionDefinition` is not compared by function-pointer equality.
- Mission 1 owns `(4,7)/(3,8)/(5,8)` deployment; `build_player_squad` consumes a deployment parameter.
- `CampaignState::complete_mission` and `purchase_upgrade` live in `campaign/progression.rs`.
- There is no `PurchaseReceipt` without a concrete consumer.
- Task 3 rewrites both `restart_mission` callers before deleting the method, so its `cargo test --all-targets` gate can pass.
- Task 4 temporarily defaults to Battle and performs a real `cargo run` scheduling checkpoint; Task 5 restores Title default.
- Aegis cleanup occurs once around `resolve_enemy_phase_inner`, not at three return sites.
- Aegis damage is tested through `resolve_intent_for_test`; one full-phase test pins expiry.
- Focus is proven not to affect/consume Counter.
- Both Mission 1 constructors start with `PilotSkillState::default()`.
- Objectives use PRIMARY/BONUS labels; `[P]` is reserved for PILOT and the control hint includes it.
- PreMissionStory and Aftermath reuse one local dialogue-screen builder.
- Mobility changes evasion only; Overdrive is the sole +movement effect.
- One ticket = one PR; no mission registry, skill framework, save versioning, second UI crate, or Mission 2 content.