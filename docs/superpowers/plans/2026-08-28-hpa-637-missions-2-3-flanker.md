# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius from the completed Mission 1 campaign loop to three continuously playable missions, adding a three-round Gunner defense, a Courier interception mission, and the Flanker enemy without introducing a generic objective or AI framework.

**Architecture:** `BattleState` gains one closed `MissionRules` row covering only the objective/opening shapes Missions 1–3 use. `mission` remains the typed authoring layer with a small shared regular-enemy catalog and separate Mission 2/3 modules; Flanker stays one explicit deterministic planner branch, while the existing campaign screens, save model, combat mechanics, and single Bevy application crate remain the composition surface.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, existing headless Rust tests, existing checked-in `assets/models/mission_one.gltf` and VN PNGs only.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Global Constraints

- One Linear ticket (`HPA-637`) = one implementation PR; continue implementation on this draft PR.
- Keep dependency direction `presentation -> mission -> domain`; `src/domain/` must not import Bevy or campaign/presentation types.
- Keep one application crate and `bevy = "0.19"`; add no dependency.
- Preserve committed-intent semantics: player movement never retargets an already committed enemy intent.
- Keep mission content typed in Rust; add no RON/JSON/scripting/content framework.
- Add no objective callback/trait/registry, behavior tree, utility-AI framework, pathfinding dependency, stealth, teleportation, new initiative system, status framework, new playable unit, deployment selection, mission select, branching, difficulty, boss, or new hazard type.
- Mission 2 protects the existing Gunner; do not add a neutral faction/objective-unit role.
- Mission 3 uses one Flanker as Courier; escort clear is never a hidden victory condition.
- Optional objectives affect credits only and never gate progression.
- Reuse the existing VN art and glTF; add no asset-generation pipeline.
- Keep save state to `next_mission`, credits, and upgrades; add no migration/version compatibility code.
- All automated tests stay headless.
- Final gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `cargo build --release`.

---

## File Structure

### New files

- `src/mission/enemies.rs` — fixed regular-enemy constructors and four weapon specs.
- `src/mission/mission_two.rs` — Mission 2 board, IDs, opening, rules, dialogue, rewards, tests.
- `src/mission/mission_three.rs` — Mission 3 board, IDs, opening, rules, dialogue, rewards, tests.
- `docs/validation/hpa-637.md` — final automated/manual validation evidence.

### Modified files

- `src/domain/model.rs` — closed mission-rule types, generic bonus state/result, Flanker archetype.
- `src/domain/battle.rs` — store rules and evaluate protect/intercept terminal conditions.
- `src/domain/enemy.rs` — consume authored opening rows and add Flanker movement/target preference.
- `src/mission/mod.rs` — modules, MissionId 1–4, definition dispatch.
- `src/mission/mission_one.rs` — consume shared enemy catalog and authored Mission 1 rules/opening.
- `src/campaign/progression.rs` — generic bonus reward bit.
- `src/presentation/battlefield.rs` — Flanker scene/scale/rings, extraction ring, generic debug name.
- `src/presentation/sync.rs` — preserve archetype-specific scale during unit sync.
- `src/presentation/interaction.rs` — Flanker is never pilot-controllable; generic debug name if touched.
- `src/presentation/ui.rs` — rule-aware objective progress and generic result/event copy.
- `src/presentation/campaign_ui.rs` — continuous mission routing and generic reward/handoff copy.
- `tests/presentation_app.rs` — renderer-free campaign/battle integration through Mission 3.
- `README.md` — current three-mission player-facing behavior.
- `CLAUDE.md` — current architecture/rules of record.

### Expected untouched unless a failing integration test proves otherwise

- `src/campaign/model.rs`
- `src/campaign/save.rs` implementation
- `src/campaign/session.rs`
- `src/app.rs`
- `assets/models/mission_one.gltf`
- `assets/vn/*`
- `Cargo.toml`
- `Cargo.lock`

---

### Task 1: Add closed mission rules and generic bonus semantics

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/campaign/progression.rs`
- Modify: active call sites/tests that construct `BattleState` or read `turnabout_complete`

**Interfaces:**
- Produces: `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `MissionRules`.
- Produces: `BattleState::rules(&self) -> MissionRules`.
- Changes: `BattleState::new(board, units, weapons, rules, seed)`.
- Changes: `ObjectiveProgress { optional_complete: bool }`.
- Changes: `MissionResult { victory, optional_complete, rounds }`.
- Preserves: `BattleEvent::OptionalObjectiveCompleted`.

- [ ] **Step 1: Write terminal-rule tests before changing the model**

Add a plain-Rust fixture inside `src/domain/battle.rs` tests that accepts `MissionRules`, with player `UnitId(1)`, protected player `UnitId(2)`, escort enemy `UnitId(8)`, and objective enemy `UnitId(9)`. Use private child-module access to call `check_terminal_state` directly.

Pin these behaviors:

```rust
#[test]
fn protect_rule_ignores_enemy_clear_and_wins_only_after_required_round() {
    let mut battle = objective_fixture(MissionRules {
        primary: PrimaryObjective::ProtectThroughRound {
            target: UnitId(2),
            round: 3,
        },
        optional: OptionalObjective::ProtectTargetAtHalfHp { target: UnitId(2) },
        opening_plan: &[],
    });

    battle.apply_direct_damage(UnitId(9), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None);

    battle.phase = BattlePhase::EnemyPlanning;
    battle.round = 2;
    assert!(battle.check_terminal_state().is_empty());

    battle.round = 3;
    let events = battle.check_terminal_state();
    assert!(matches!(battle.result(), Some(MissionResult { victory: true, .. })));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionCompleted { .. })));
}

#[test]
fn protect_rule_fails_immediately_when_target_is_knocked_out() {
    let mut battle = objective_fixture(PROTECT_RULE);
    let events = battle.apply_direct_damage(
        UnitId(2),
        99,
        DamageSource::EnemyWeapon(UnitId(9), WeaponId(9)),
    );
    assert!(matches!(battle.result(), Some(MissionResult { victory: false, .. })));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionFailed { .. })));
}

#[test]
fn intercept_rule_wins_on_target_ko_not_escort_clear() {
    let mut battle = intercept_fixture();
    battle.apply_direct_damage(UnitId(8), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None);

    battle.apply_direct_damage(UnitId(9), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert!(matches!(battle.result(), Some(MissionResult { victory: true, .. })));
}

#[test]
fn intercept_rule_fails_on_escape_or_deadline() {
    let mut escaped = intercept_fixture();
    escaped.units.get_mut(&UnitId(9)).unwrap().position = GridPos::new(8, 2);
    assert!(matches!(escaped.check_terminal_state().as_slice(), [BattleEvent::MissionFailed { .. }]));

    let mut timed_out = intercept_fixture();
    timed_out.phase = BattlePhase::EnemyPlanning;
    timed_out.round = 4;
    assert!(matches!(timed_out.check_terminal_state().as_slice(), [BattleEvent::MissionFailed { .. }]));
}
```

- [ ] **Step 2: Run the focused module and confirm the new tests fail/compile-break for missing rule types**

```bash
cargo test --lib domain::battle:: -- --nocapture
```

Expected: red until the new rule/result types and terminal semantics exist.

- [ ] **Step 3: Add the exact closed domain types**

In `src/domain/model.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimaryObjective {
    EliminateAllEnemies,
    ProtectThroughRound { target: UnitId, round: u16 },
    InterceptBeforeEscape {
        target: UnitId,
        escape: GridPos,
        deadline_round: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalObjective {
    Turnabout,
    ProtectTargetAtHalfHp { target: UnitId },
    VictoryByRound { round: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnemyOpening {
    pub unit: UnitId,
    pub destination: GridPos,
    pub target: Option<UnitId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionRules {
    pub primary: PrimaryObjective,
    pub optional: OptionalObjective,
    pub opening_plan: &'static [EnemyOpening],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectiveProgress {
    pub optional_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionResult {
    pub victory: bool,
    pub optional_complete: bool,
    pub rounds: u16,
}
```

- [ ] **Step 4: Store rules in `BattleState` and change construction**

Change the constructor to:

```rust
pub(crate) fn new(
    board: BoardState,
    units: impl IntoIterator<Item = UnitState>,
    weapons: impl IntoIterator<Item = WeaponSpec>,
    rules: MissionRules,
    seed: u64,
) -> Self
```

Add private `rules: MissionRules` plus:

```rust
pub const fn rules(&self) -> MissionRules {
    self.rules
}
```

For existing generic fixtures, use:

```rust
MissionRules {
    primary: PrimaryObjective::EliminateAllEnemies,
    optional: OptionalObjective::Turnabout,
    opening_plan: &[],
}
```

Use the same temporary rules in `mission_one_for_campaign`; Task 2 replaces the empty opening with exact authored rows.

- [ ] **Step 5: Implement objective-aware terminal evaluation without a new round counter**

Add a private `primary_outcome(&self) -> Option<bool>`:

```rust
fn primary_outcome(&self) -> Option<bool> {
    let any_living_player = self
        .units
        .values()
        .any(|unit| unit.faction == Faction::Player && !unit.is_knocked_out());
    if !any_living_player {
        return Some(false);
    }

    match self.rules.primary {
        PrimaryObjective::EliminateAllEnemies => {
            let any_living_enemy = self
                .units
                .values()
                .any(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out());
            (!any_living_enemy).then_some(true)
        }
        PrimaryObjective::ProtectThroughRound { target, round } => {
            let target = self.units.get(&target).expect("authored protected target must exist");
            if target.is_knocked_out() {
                Some(false)
            } else if self.phase == BattlePhase::EnemyPlanning && self.round >= round {
                Some(true)
            } else {
                None
            }
        }
        PrimaryObjective::InterceptBeforeEscape {
            target,
            escape,
            deadline_round,
        } => {
            let target = self.units.get(&target).expect("authored interception target must exist");
            if target.is_knocked_out() {
                Some(true)
            } else if target.position == escape {
                Some(false)
            } else if self.phase == BattlePhase::EnemyPlanning && self.round >= deadline_round {
                Some(false)
            } else {
                None
            }
        }
    }
}
```

`check_terminal_state()` keeps its current `self.result.is_some()` guard/cleanup and seals only the outcome returned above. Do not add eliminate-all as a fallback for protect/intercept missions.

- [ ] **Step 6: Implement one generic bonus bit**

Keep Turnabout's existing damage predicates, but only run them for `OptionalObjective::Turnabout`. Add private helpers:

```rust
fn optional_condition_met(&self) -> bool {
    match self.rules.optional {
        OptionalObjective::Turnabout => self.objectives.optional_complete,
        OptionalObjective::ProtectTargetAtHalfHp { target } => {
            let target = self.units.get(&target).expect("authored bonus target must exist");
            !target.is_knocked_out() && target.hp * 2 >= target.stats.max_hp
        }
        OptionalObjective::VictoryByRound { round } => self.round <= round,
    }
}

fn mark_optional_complete(&mut self) -> Vec<BattleEvent> {
    if self.objectives.optional_complete {
        Vec::new()
    } else {
        self.objectives.optional_complete = true;
        vec![BattleEvent::OptionalObjectiveCompleted]
    }
}
```

On terminal victory, if the terminal bonus condition is true and not already complete, append `OptionalObjectiveCompleted` before `MissionCompleted`. Defeat never newly marks terminal-only bonuses.

- [ ] **Step 7: Rename active code from `turnabout_complete` to `optional_complete`**

Update campaign reward calculation, current presentation reads, and all active Rust tests/fixtures. Do not rewrite historical HPA-632/HPA-635 spec/validation documents.

- [ ] **Step 8: Run focused + full regression tests**

```bash
cargo fmt --check
cargo test --lib domain::battle::
cargo test --lib campaign::progression::
cargo test --all-targets
```

Expected: all pass and Mission 1 still behaves as eliminate-all + Turnabout.

- [ ] **Step 9: Commit**

```bash
git add src/domain/model.rs src/domain/battle.rs src/campaign/progression.rs src/mission/mission_one.rs src/presentation/ui.rs

git commit -m "feat: add authored mission objective rules"
```

Stage any additional active test file changed by the rename.

---

### Task 2: Move openings into mission data and add the regular-enemy/Flanker seam

**Files:**
- Create: `src/mission/enemies.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/mission/mission_one.rs`
- Modify: `src/domain/model.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/presentation/interaction.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/battlefield.rs`

**Interfaces:**
- Produces: `mission::enemies::{rifleman, striker, artillery, flanker}`.
- Produces: `mission::enemies::{service_rifle, shock_claw, siege_mortar, skirmish_carbine}`.
- Adds: `UnitArchetype::Flanker`.
- Consumes: `BattleState::rules().opening_plan` for round-0 movement/intents.
- Preserves: exact Mission 1 round-0 positions/targets/intent order.

- [ ] **Step 1: Strengthen the Mission 1 opening characterization test**

Keep the existing exact positions, intent order `[STRIKER, RIFLEMAN_LEFT, RIFLEMAN_RIGHT, ARTILLERY]`, and mortar footprint, and add:

```rust
assert_eq!(battle.intent_for(ids::RIFLEMAN_LEFT).unwrap().intended_occupant, Some(ids::GUNNER));
assert_eq!(battle.intent_for(ids::RIFLEMAN_RIGHT).unwrap().intended_occupant, Some(ids::INTERCEPTOR));
assert_eq!(battle.intent_for(ids::STRIKER).unwrap().intended_occupant, Some(ids::VANGUARD));
assert_eq!(battle.intent_for(ids::ARTILLERY).unwrap().intended_occupant, Some(ids::VANGUARD));
```

- [ ] **Step 2: Run the characterization test green before refactoring**

```bash
cargo test --lib domain::enemy::tests::authored_opening_places_four_locked_threats -- --exact
```

Expected: PASS.

- [ ] **Step 3: Create `mission::enemies` with exact existing values + Flanker**

Use shared weapon IDs 201–204 and `mission::squad::{stats, unit, weapon}`. Factories:

```rust
pub fn rifleman(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(id, name, UnitArchetype::Rifleman, Faction::Enemy,
         stats(9, 1, 2, 72, 5, 0), position, vec![SERVICE_RIFLE])
}

pub fn striker(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(id, name, UnitArchetype::Striker, Faction::Enemy,
         stats(12, 2, 2, 78, 10, 0), position, vec![SHOCK_CLAW])
}

pub fn artillery(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(id, name, UnitArchetype::Artillery, Faction::Enemy,
         stats(10, 1, 1, 90, 0, 0), position, vec![SIEGE_MORTAR])
}

pub fn flanker(id: UnitId, name: &'static str, position: GridPos) -> UnitState {
    unit(id, name, UnitArchetype::Flanker, Faction::Enemy,
         stats(8, 0, 4, 82, 30, 0), position, vec![SKIRMISH_CARBINE])
}
```

Weapon specs:

```rust
weapon(SERVICE_RIFLE, "Service Rifle", 2, 4, WeaponShape::Single, 5, 0, 5, 0, false, false)
weapon(SHOCK_CLAW, "Shock Claw", 1, 1, WeaponShape::Single, 7, 10, 10, 0, false, false)
weapon(SIEGE_MORTAR, "Siege Mortar", 3, 8, WeaponShape::Cross1, 6, 5, 5, 0, false, false)
weapon(SKIRMISH_CARBINE, "Skirmish Carbine", 1, 2, WeaponShape::Single, 4, 5, 10, 0, false, false)
```

Export `pub mod enemies;`.

- [ ] **Step 4: Refactor Mission 1 to shared enemies + exact authored opening**

Keep Mission 1 enemy IDs 11–14. Re-export shared weapon IDs from `mission_one::ids` for current callers/tests. Add:

```rust
static MISSION_ONE_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN_LEFT, destination: GridPos::new(2, 5), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::RIFLEMAN_RIGHT, destination: GridPos::new(6, 5), target: Some(ids::INTERCEPTOR) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4, 6), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4, 0), target: Some(ids::VANGUARD) },
];

const MISSION_ONE_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateAllEnemies,
    optional: OptionalObjective::Turnabout,
    opening_plan: &MISSION_ONE_OPENING,
};
```

- [ ] **Step 5: Delete Mission-1-specific opening logic from `enemy.rs`**

Round-0 movement looks up `EnemyOpening` by enemy ID and uses `destination`; opening forced intent resolves `target` to the current living target position. Delete the old archetype/position opening match and `opening_target()` helper.

Do not change later movement or intent locking here.

- [ ] **Step 6: Add `UnitArchetype::Flanker` and make exhaustive matches explicit**

Flanker must be rejected by pilot-skill paths just like other enemies. Temporarily map `scene_index(Flanker) => 2`; Task 5 adds the full visual profile. Do not add a Flanker initiative value or another combat capability.

- [ ] **Step 7: Write red Flanker planner tests**

Use plain fixtures that deliberately make normal player-priority scoring disagree with the desired objective target.

```rust
#[test]
fn protect_flanker_moves_into_band_of_the_protected_target() {
    let battle = protect_flanker_fixture();
    let destination = choose_enemy_destination(&battle, UnitId(24)).unwrap();
    let gunner = battle.unit(UnitId(2)).unwrap();
    let weapon = battle.weapon(SKIRMISH_CARBINE).unwrap();
    assert_eq!(
        distance_to_band(destination.manhattan(gunner.position), weapon.min_range, weapon.max_range),
        0
    );
}

#[test]
fn protect_flanker_prefers_gunner_when_gunner_is_legally_targetable() {
    let battle = protect_flanker_attack_fixture();
    let intent = build_intent(&battle, UnitId(24), None).unwrap();
    assert_eq!(intent.intended_occupant, Some(UnitId(2)));
}

#[test]
fn courier_flanker_reduces_distance_to_escape_instead_of_chasing_player() {
    let battle = intercept_flanker_fixture();
    let origin = battle.unit(UnitId(31)).unwrap().position;
    let destination = choose_enemy_destination(&battle, UnitId(31)).unwrap();
    assert!(destination.manhattan(GridPos::new(8, 2)) < origin.manhattan(GridPos::new(8, 2)));
}
```

Also pin equal-distance tie-breaking toward the candidate with more open orthogonal neighbors.

- [ ] **Step 8: Run the new Flanker tests red**

```bash
cargo test --lib domain::enemy:: -- --nocapture
```

Expected: new objective-aware assertions fail before planner changes.

- [ ] **Step 9: Implement only the two Flanker destination policies**

Add a local `open_neighbor_count(battle, mover, position)` helper using board bounds/blocking/live-explosive/living-unit occupancy.

For protect rules, score by `(distance_to_band, Manhattan, Reverse(open_neighbors), y, x)`. For the designated interception target, score by `(Manhattan_to_escape, Reverse(open_neighbors), y, x)`.

Extract the current Rifleman/Striker attack-band calculation into a small local helper only if needed for Flanker's non-objective fallback. Do not refactor Artillery or add policy objects.

- [ ] **Step 10: Prefer the protected target only for Flanker targeting**

In `choose_target`, when `attacker.archetype == Flanker` and rules are protect, place `misses_protected_target: bool` before the existing threatened-count/player-priority sort key. Keep all existing footprint/intent locking.

- [ ] **Step 11: Run regressions**

```bash
cargo fmt --check
cargo test --lib domain::enemy::
cargo test --lib mission::mission_one::
cargo test --all-targets
```

Expected: exact Mission 1 opening remains green; Flanker tests are deterministic.

- [ ] **Step 12: Commit**

```bash
git add src/domain/model.rs src/domain/enemy.rs src/mission/mod.rs src/mission/enemies.rs src/mission/mission_one.rs src/presentation/interaction.rs src/presentation/ui.rs src/presentation/battlefield.rs

git commit -m "feat: add authored enemy openings and flanker"
```

---

### Task 3: Author Mission 2 as a three-round Gunner defense

**Files:**
- Create: `src/mission/mission_two.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`

**Interfaces:**
- Produces: `mission_two(seed)`, `mission_two_for_campaign(seed, upgrades)`, `MISSION_TWO_DEFINITION`.
- Adds: `MissionId::Three` and `MissionId::number()` for One/Two/Three.
- Changes: `mission_definition(Two)` becomes authored; Three remains handoff until Task 4.

- [ ] **Step 1: Write Mission 2 authoring tests first**

Pin:

```rust
assert_eq!(battle.board().width(), 9);
assert_eq!(battle.board().height(), 9);
assert_eq!(battle.unit(ids::VANGUARD).unwrap().position, GridPos::new(3, 7));
assert_eq!(battle.unit(ids::GUNNER).unwrap().position, GridPos::new(4, 6));
assert_eq!(battle.unit(ids::INTERCEPTOR).unwrap().position, GridPos::new(5, 7));
assert_eq!(
    battle.rules().primary,
    PrimaryObjective::ProtectThroughRound { target: ids::GUNNER, round: 3 }
);
assert_eq!(
    battle.rules().optional,
    OptionalObjective::ProtectTargetAtHalfHp { target: ids::GUNNER }
);
```

Also assert exact blocking `(3,3),(5,3),(2,6),(6,6)`, hazards `(1,5),(7,5)`, explosive `(6,4)` HP 4, one each Rifleman/Striker/Artillery/Flanker, and nonzero Gunner upgrade projection through `build_player_squad`.

Pin definition:

```text
Mission 2 — Hold Relay Nine
Protect Gunner through the end of Round 3.
Hold Fast: finish with Gunner at or above 50% HP.
400 base / 100 bonus / unlock Three
```

- [ ] **Step 2: Run the new module test target red**

```bash
cargo test --lib mission::mission_two:: -- --nocapture
```

Expected: missing module/definition until implemented.

- [ ] **Step 3: Implement exact Mission 2 board/roster/opening**

IDs:

```rust
pub const RIFLEMAN: UnitId = UnitId(21);
pub const STRIKER: UnitId = UnitId(22);
pub const ARTILLERY: UnitId = UnitId(23);
pub const FLANKER: UnitId = UnitId(24);
```

Player deployment:

```rust
SquadDeployment {
    vanguard: GridPos::new(3, 7),
    gunner: GridPos::new(4, 6),
    interceptor: GridPos::new(5, 7),
}
```

Board:

```rust
BoardState::new(
    9,
    9,
    [GridPos::new(3,3), GridPos::new(5,3), GridPos::new(2,6), GridPos::new(6,6)],
    [GridPos::new(1,5), GridPos::new(7,5)],
    [ExplosiveState { position: GridPos::new(6,4), hp: 4, exploded: false }],
)
```

Enemy starts/openings/forced targets:

```rust
static MISSION_TWO_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(2,4), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4,5), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4,0), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::FLANKER, destination: GridPos::new(5,5), target: Some(ids::INTERCEPTOR) },
];
```

Enemy starting positions are Rifleman `(2,2)`, Striker `(4,3)`, Artillery `(4,0)`, Flanker `(8,4)`.

- [ ] **Step 4: Pin the competing opening threats in `enemy.rs`**

After `begin_round()` on Mission 2, assert intended occupants are exactly:

```text
Rifleman  -> Vanguard
Striker   -> Gunner
Artillery -> Gunner
Flanker   -> Interceptor
```

This test protects the intended defense lesson: Gunner is pressured, but reactions across the whole squad matter.

- [ ] **Step 5: Add exact Mission 2 VN/definition copy using existing assets**

Pre-mission:

```text
Control: Counterattack incoming. Gunner is finishing the Relay Nine uplink; the upload needs three full rounds.
Vanguard: Then Gunner stays standing. We move around the locks, cover the weak angles, and hold.
Control: New contact: a fast Flanker is cutting around the line. Expect it to chase the uplink carrier.
```

Aftermath:

```text
Vanguard: Uplink complete. Relay Nine can finally hand us the enemy route data.
Control: It found a courier breaking for extraction. Resupply now — we only get one chance to cut it off.
```

Use `vn/relay_nine_bg.png`, `control_neutral.png`, `control_alert.png`, `vanguard_neutral.png` only.

- [ ] **Step 6: Add MissionId Three + generic authored-mission Proceed routing**

During this task:

```rust
pub enum MissionId { One, Two, Three }
```

`mission_definition`: One/Two are `Some`, Three is `None`. Add `number()`.

Continue routing:

```text
One -> PreMissionStory
Two/Three -> Upgrade
```

Change Upgrade `Proceed` to:

```rust
let authored = runtime
    .0
    .state
    .as_ref()
    .and_then(|state| mission_definition(state.next_mission))
    .is_some();
next_state.set(if authored { GameScreen::PreMissionStory } else { GameScreen::NextMission });
```

Make handoff copy derive `MISSION {state.next_mission.number()} UNLOCKED` instead of hard-coded Mission 2.

- [ ] **Step 7: Add Mission 2 terminal integration tests**

Using `mission_two(7)`:

- KO all four enemies before Round 3 → `result() == None`.
- KO Gunner → defeat.
- set round 3 + enter `EnemyPlanning`/call the normal round boundary → victory.
- Gunner exactly half HP → bonus true.
- Gunner below half but alive → victory, bonus false.
- For terminal-bonus success, events order is `OptionalObjectiveCompleted` before `MissionCompleted`.

- [ ] **Step 8: Run Mission 2/regression suites**

```bash
cargo fmt --check
cargo test --lib mission::mission_two::
cargo test --lib domain::battle::
cargo test --lib domain::enemy::
cargo test --all-targets
```

Expected: no hidden destroy-all victory; Mission 1 remains green.

- [ ] **Step 9: Commit**

```bash
git add src/mission/mission_two.rs src/mission/mod.rs src/domain/battle.rs src/domain/enemy.rs src/presentation/campaign_ui.rs

git commit -m "feat: add mission 2 gunner defense"
```

---

### Task 4: Author Mission 3 interception and advance the campaign to Mission 4 handoff

**Files:**
- Create: `src/mission/mission_three.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/campaign/progression.rs` tests
- Modify: existing save tests for enum round-trip

**Interfaces:**
- Produces: `mission_three(seed)`, `mission_three_for_campaign(seed, upgrades)`, `MISSION_THREE_DEFINITION`.
- Adds: `MissionId::Four`; One–Three are authored, Four is handoff.
- Preserves: same `CampaignState` shape and save implementation.

- [ ] **Step 1: Write Mission 3 authoring tests first**

Pin:

```rust
assert_eq!(battle.unit(ids::COURIER).unwrap().archetype, UnitArchetype::Flanker);
assert_eq!(battle.unit(ids::COURIER).unwrap().position, GridPos::new(0, 6));
assert_eq!(
    battle.rules().primary,
    PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: GridPos::new(8, 2),
        deadline_round: 4,
    }
);
assert_eq!(battle.rules().optional, OptionalObjective::VictoryByRound { round: 2 });
```

Also assert player deployment `(4,7)/(3,8)/(5,8)`, blocking `(4,3),(4,4),(4,5)`, hazard `(2,5)`, explosive `(6,3)` HP 4, and exactly Courier/Rifleman/Striker enemies.

- [ ] **Step 2: Write red objective tests**

Pin:

- Courier KO wins while at least one escort remains alive.
- Rifleman+Striker KO while Courier lives does not win.
- Courier at `(8,2)` fails.
- living Courier at `EnemyPlanning`, Round 4 fails.
- Courier KO on Round 2 earns bonus; Round 3 KO wins without bonus.

- [ ] **Step 3: Run the new module red**

```bash
cargo test --lib mission::mission_three:: -- --nocapture
```

Expected: missing implementation.

- [ ] **Step 4: Implement exact Mission 3 content**

IDs:

```rust
pub const COURIER: UnitId = UnitId(31);
pub const RIFLEMAN: UnitId = UnitId(32);
pub const STRIKER: UnitId = UnitId(33);
```

Board/deployment:

```text
Players: V (4,7), G (3,8), I (5,8)
Blocking: (4,3), (4,4), (4,5)
Hazard: (2,5)
Explosive: (6,3), HP 4
Escape: (8,2)
```

Enemy starts/opening:

```rust
static MISSION_THREE_OPENING: [EnemyOpening; 3] = [
    EnemyOpening { unit: ids::COURIER, destination: GridPos::new(0,6), target: None },
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(3,4), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(5,7), target: Some(ids::INTERCEPTOR) },
];
```

Starting positions: Courier `(0,6)`, Rifleman `(3,2)`, Striker `(6,6)`.

Definition:

```text
Mission 3 — Cut the Courier
Intercept Courier before extraction or the end of Round 4.
Swift Intercept: defeat Courier by the end of Round 2.
500 base / 150 bonus / unlock Four
```

- [ ] **Step 5: Add exact Mission 3 VN copy using existing assets**

Pre-mission:

```text
Control: Courier identified. That Flanker has Relay Nine's route keys and is heading for extraction.
Vanguard: We cut across and stop it. Escorts are secondary — the Courier is the mission.
Control: Extraction is at the east marker. If it gets out, or Round 4 closes, the data is gone.
```

Aftermath:

```text
Vanguard: Courier down. The route keys are intact.
Control: Confirmed. They point to a larger force ahead. Spend the salvage and prepare for the next operation.
```

- [ ] **Step 6: Extend MissionId/dispatch to Four**

Final shape:

```rust
pub enum MissionId { One, Two, Three, Four }
```

`mission_definition`: One/Two/Three `Some`, Four `None`; `number()` returns 1–4.

Continue routing becomes:

```text
One       -> PreMissionStory
Two/Three -> Upgrade
Four      -> NextMission
```

The Task-3 definition-driven Proceed automatically routes Three to story and Four to handoff.

- [ ] **Step 7: Prove base progression through all three missions without bonuses**

In campaign progression tests, start new state and complete One, Two, Three with:

```rust
MissionResult {
    victory: true,
    optional_complete: false,
    rounds: 3,
}
```

Assert:

```rust
assert_eq!(state.next_mission, MissionId::Four);
assert_eq!(state.credits, 300 + 400 + 500);
```

Add one separate test proving `optional_complete: true` adds only `definition.optional_reward` and does not alter unlock behavior.

- [ ] **Step 8: Extend save round-trip coverage to MissionId Four**

Persist/reload a state with `next_mission: Four`, nonzero credits, and a nonzero upgrade. Assert exact equality. Do not add a schema version or migration path.

- [ ] **Step 9: Run Mission 3/campaign/save suites**

```bash
cargo fmt --check
cargo test --lib mission::mission_three::
cargo test --lib campaign::progression::
cargo test --lib campaign::save::
cargo test --all-targets
```

Expected: normal no-bonus completion reaches Four with 1200 credits.

- [ ] **Step 10: Commit**

```bash
git add src/mission/mission_three.rs src/mission/mod.rs src/presentation/campaign_ui.rs src/campaign/progression.rs

git commit -m "feat: add mission 3 courier interception"
```

Stage the existing save-test file only if it changed.

---

### Task 5: Make presentation objective-generic and visibly mark Flanker/extraction

**Files:**
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/sync.rs`
- Modify: `src/presentation/interaction.rs` only if Mission-1-specific debug names remain there

**Interfaces:**
- Changes: `result_overlay_copy(result, definition)`.
- Produces: rule-aware primary/bonus progress in `HudSnapshot`.
- Produces: `unit_scale(archetype)` shared by spawn/sync.
- Produces: Flanker child under-ring and static interception extraction ring, both using existing rendering assets.

- [ ] **Step 1: Write pure UI-copy tests first**

For Mission 2 after round start:

```rust
assert!(hud.primary.contains("Protect Gunner through the end of Round 3."));
assert!(hud.primary.contains("Round 1/3"));
assert!(hud.primary.contains("Gunner HP"));
assert!(hud.optional.contains("Hold Fast"));
assert!(hud.optional.contains("On track"));
```

For Mission 3:

```rust
assert!(hud.primary.contains("Intercept Courier"));
assert!(hud.primary.contains("Round 1/4"));
assert!(hud.primary.contains("cells from extraction"));
assert!(hud.optional.contains("Swift Intercept"));
assert!(hud.optional.contains("Available"));
```

Pin result copy:

```rust
assert_eq!(
    result_overlay_copy(
        MissionResult { victory: true, optional_complete: false, rounds: 3 },
        &MISSION_THREE_DEFINITION,
    ),
    "MISSION COMPLETE\nMission 3 — Cut the Courier\nPRIMARY  Intercept Courier before extraction or the end of Round 4. · Complete\nBONUS    Swift Intercept: defeat Courier by the end of Round 2. · Missed"
);
```

Change aftermath reward expectation from `Turnabout +100` to `Bonus +100`.

- [ ] **Step 2: Run UI tests red**

```bash
cargo test --lib presentation::ui:: -- --nocapture
cargo test --lib presentation::campaign_ui:: -- --nocapture
```

Expected: fail on current hard-coded enemy-count/Turnabout/Relay Nine/Mission 2 copy.

- [ ] **Step 3: Implement rule-aware objective progress**

Append to `MissionDefinition` copy:

- eliminate: `{remaining} enemies remaining`;
- protect: `Round {current}/{required} · {target} HP {hp}/{max}`;
- intercept: `Round {current}/{deadline} · {distance} cells from extraction`.

Bonus state:

- Turnabout: `Complete` / `Not yet`;
- half-HP: `On track` / `Missed`;
- victory-by-round: `Available` / `Missed`;
- terminal result: `Achieved` / `Missed` from `optional_complete`.

Do not move authored objective strings into domain.

- [ ] **Step 4: Make terminal/event/reward copy generic**

`result_overlay_copy(result, definition)` renders mission title + authored primary/bonus + Complete/Failed/Achieved/Missed. Change `OptionalObjectiveCompleted` playback to `BONUS OBJECTIVE COMPLETE`. Change aftermath receipt label to `Bonus +...`.

- [ ] **Step 5: Add and test `unit_scale`**

In `battlefield.rs`:

```rust
pub const fn unit_scale(archetype: UnitArchetype) -> f32 {
    match archetype {
        UnitArchetype::Flanker => 0.62,
        _ => 0.72,
    }
}
```

Test Flanker scene index 2, scale 0.62, and normal enemy scale 0.72.

- [ ] **Step 6: Spawn Flanker under-ring as a child of its unit visual**

When spawning a Flanker model, retain the spawned unit entity and add a child:

```rust
commands.spawn((
    Name::new("Flanker Marker"),
    Mesh3d(visual_assets.ring_mesh.clone()),
    MeshMaterial3d(visual_assets.telegraph_edge.clone()),
    Transform::from_xyz(0.0, -0.17, 0.0).with_scale(Vec3::splat(0.9)),
    Pickable::IGNORE,
    ChildOf(unit_entity),
));
```

Use a local Y offset that leaves the ring just above the board after visual inspection; keep it a child so movement requires no new sync component/framework.

- [ ] **Step 7: Preserve Flanker scale in `sync.rs`**

Replace hard-coded:

```rust
transform.scale = Vec3::splat(0.72);
```

with:

```rust
transform.scale = Vec3::splat(unit_scale(unit.archetype));
```

Import the helper from `battlefield`. This prevents per-frame sync from erasing the fast-silhouette scale.

- [ ] **Step 8: Spawn an extraction ring for interception rules**

During mission-root population:

```rust
if let PrimaryObjective::InterceptBeforeEscape { escape, .. } = battle.0.rules().primary {
    commands.spawn((
        Name::new("Extraction Objective"),
        Mesh3d(visual_assets.ring_mesh.clone()),
        MeshMaterial3d(visual_assets.intended_target.clone()),
        Transform::from_translation(grid_to_world(escape) + Vec3::Y * 0.03)
            .with_scale(Vec3::splat(1.08)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
}
```

No new domain prop or asset is introduced.

- [ ] **Step 9: Remove touched Mission-1-only debug names**

Use `Name::new("Mission Presentation")` instead of `Mission 1 Presentation` in battlefield/restart roots. Keep the asset filename/constants `mission_one.gltf` unchanged.

- [ ] **Step 10: Run presentation tests**

```bash
cargo fmt --check
cargo test --lib presentation::ui::
cargo test --lib presentation::campaign_ui::
cargo test --test presentation_app
```

Expected: all headless tests pass.

- [ ] **Step 11: Commit**

```bash
git add src/presentation/ui.rs src/presentation/campaign_ui.rs src/presentation/battlefield.rs src/presentation/sync.rs src/presentation/interaction.rs

git commit -m "feat: present mission objectives and flanker pressure"
```

Only stage interaction if changed.

---

### Task 6: Prove continuous campaign entry/restart/save/upgrade routing through Mission 3

**Files:**
- Modify: `tests/presentation_app.rs`
- Modify: `src/presentation/campaign_ui.rs` only if tests expose a routing defect
- Modify: `src/app.rs` only if tests expose a mission-generic battle-entry defect
- Modify: `src/presentation/interaction.rs` only if restart is not mission-generic

**Interfaces:**
- Consumes: existing `enter_battle`, `ActiveMission`, `definition.build`, `CampaignRuntime`, `persist_purchase`, restart seams.
- Verifies: no new app state/resource is needed for Missions 2–3.

- [ ] **Step 1: Add renderer-free Mission 2 battle-entry coverage**

Create campaign state with `next_mission: Two` and a nonzero Gunner HP upgrade, invoke the existing battle-entry path, assert:

```rust
assert_eq!(app.world().resource::<ActiveMission>().0.id, MissionId::Two);
let battle = &app.world().resource::<BattleRuntime>().0;
assert_eq!(battle.rules().primary, PrimaryObjective::ProtectThroughRound {
    target: squad::ids::GUNNER,
    round: 3,
});
assert_eq!(battle.round(), 1);
assert!(battle.unit(squad::ids::GUNNER).unwrap().stats.max_hp > 12);
```

- [ ] **Step 2: Add Mission 3 entry/restart coverage**

Enter with `next_mission: Three`, mutate battle, call existing restart with a fixed seed, assert ActiveMission remains Three and fresh rules are:

```rust
PrimaryObjective::InterceptBeforeEscape {
    target: mission_three::ids::COURIER,
    escape: GridPos::new(8, 2),
    deadline_round: 4,
}
```

Assert fresh Courier HP 8. Respect the current restart contract: if restart leaves round 0 until `begin_restarted_round`, assert that first, run the existing restarted-round seam, then assert Round 1/intents.

Do not special-case mission IDs inside restart.

- [ ] **Step 3: Add pure campaign-action routing coverage for every saved ID**

Pin:

```text
Continue One   -> PreMissionStory
Continue Two   -> Upgrade
Continue Three -> Upgrade
Continue Four  -> NextMission
Proceed Two    -> PreMissionStory
Proceed Three  -> PreMissionStory
Proceed Four   -> NextMission
```

- [ ] **Step 4: Add save-backed progression continuity through Mission 3**

Use existing temp-save/session test helpers:

1. start new game;
2. complete M1 without bonus;
3. buy one affordable 200-credit upgrade;
4. complete M2 without bonus;
5. buy another affordable upgrade using current persisted costs/state;
6. complete M3 without bonus;
7. load with a fresh session;
8. assert `next_mission == Four`, exact remaining credits, and exact purchased upgrades.

Do not require any bonus for progression/purchase viability.

- [ ] **Step 5: Run integration/full tests**

```bash
cargo fmt --check
cargo test --test presentation_app
cargo test --all-targets
```

Expected: green without changes to `app.rs` or save/session implementation. If a test exposes a real hard-coded Mission-1 assumption in those generic seams, make the smallest local correction only.

- [ ] **Step 6: Commit**

```bash
git add tests/presentation_app.rs src/presentation/campaign_ui.rs src/app.rs src/presentation/interaction.rs

git commit -m "test: cover campaign progression through mission 3"
```

Only stage source files that genuinely changed.

---

### Task 7: Update docs, playtest authored tuning, and record final validation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-637.md`
- Modify: `src/mission/mission_two.rs` / `src/mission/mission_three.rs` only if playtesting shows authored tuning problems

**Interfaces:**
- Produces: current player/developer docs and reproducible HPA-637 validation evidence.
- Playtest corrections should prefer authored positions/stats/rewards over new systems.

- [ ] **Step 1: Update README to the three-mission shipped flow**

Document:

```text
Title -> story/briefing -> M1 -> aftermath/upgrades -> M2 -> aftermath/upgrades -> M3 -> M4 unlocked handoff
```

Include:

- M1 eliminate + Turnabout, 300 + 100.
- M2 Gunner defense Round 3 + half-HP bonus, 400 + 100.
- M3 Courier interception before extraction/Round 4 + Round-2 bonus, 500 + 150.
- Continue resumes active first mission story, inter-mission saves at Upgrade, post-M3 at Mission 4 handoff.
- Flanker is fast/evasive/fragile and objective-seeking.
- Existing controls/pilot skills remain unchanged.

Remove the statement that Mission 2 is not in the build.

- [ ] **Step 2: Bring `CLAUDE.md` architecture current**

Correct the stale “boots straight into Mission 1 / no campaign” section. Record current boundaries: campaign plain Rust, MissionDefinition One–Three/Four handoff, closed MissionRules in BattleState, shared `mission::enemies`, explicit Flanker planner branch, Gunner defense, Courier interception, existing committed-intent invariant, and HPA-637 spec/plan/validation references.

- [ ] **Step 3: Run all automated gates before manual play**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Record exact pass/test-count/build evidence in `docs/validation/hpa-637.md`.

- [ ] **Step 4: Manual Mission 2 validation**

With `cargo run`, record concrete evidence that:

- post-M1 Upgrade → Proceed enters M2 VN/briefing;
- opening locks are Rifleman→Vanguard, Striker→Gunner, Artillery→Gunner, Flanker→Interceptor;
- Guard/Evade/Counter/Aegis/movement meaningfully respond to those threats;
- clearing enemies before Round 3 does not instantly win;
- surviving the third enemy resolution with Gunner alive wins, even with an enemy alive;
- Gunner KO fails;
- Gunner ≥50% grants +100; below 50% still advances with +0.

If ordinary play becomes empty-round stalling, tune authored Mission 2 positions/stats; do not add reinforcements/waves.

- [ ] **Step 5: Manual Mission 3 validation**

Record:

- post-M2 Upgrade → Proceed enters M3 VN/briefing;
- Courier fast silhouette + under-ring and extraction white ring are visible;
- Courier later movement heads toward extraction/open lanes;
- committed enemy telegraphs remain locked during the player phase;
- Courier KO wins with escort(s) alive;
- escort clear alone does not win;
- reaching `(8,2)` fails;
- living Courier at Round-4 deadline fails even if blocked from exact extraction;
- Courier KO by end Round 2 grants +150; later KO still wins with +0;
- existing push/collision/hazard/explosive/reaction rules remain useful.

If the Courier gives no meaningful interception window, tune only Mission 3 authored positions/terrain or Flanker stats within the design envelope; keep it faster/more evasive/less durable than front-line enemies.

- [ ] **Step 6: Manual save/continue/upgrade continuity**

After M1 and M2 at least once: quit/relaunch, Continue, verify persisted Upgrade credits/levels, Proceed, and confirm the next mission receives those upgrades. After M3, relaunch/Continue and verify `MISSION 4 UNLOCKED`.

- [ ] **Step 7: Write concrete `docs/validation/hpa-637.md`**

The final document must contain actual branch/head SHA, automated command outcomes/test counts, observed M2 opening/result/bonus evidence, M3 route/visual/result/deadline/bonus evidence, save/upgrade continuity, and a short-session verdict. Do not leave `TBD`, TODOs, template markers, or unchecked claims.

- [ ] **Step 8: Re-run all gates after any tuning/doc edits**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Expected: all exit 0.

- [ ] **Step 9: Commit**

```bash
git add README.md CLAUDE.md docs/validation/hpa-637.md src/mission/mission_two.rs src/mission/mission_three.rs

git commit -m "docs: validate HPA-637 missions 2 and 3"
```

Only stage mission files if manual tuning changed them.

---

## Final PR Gate

Before marking the draft ready for review:

- [ ] One PR contains only HPA-637 scope.
- [ ] No new dependency or asset-generation pipeline.
- [ ] No objective framework, neutral objective role, or behavior-tree/policy framework.
- [ ] Mission 1 exact opening regression remains green.
- [ ] Mission 2 enemy-clear does not win; Gunner KO fails; full Round-3 survive wins.
- [ ] Mission 2 opening creates competing readable threats across all three players.
- [ ] Mission 3 Courier KO wins with escorts alive; extraction and Round-4 deadline fail.
- [ ] Flanker planner is deterministic and objective-aware with no new RNG call.
- [ ] Flanker uses existing scene 2 + 0.62 scale + child under-ring; extraction uses existing white ring material.
- [ ] `apply_unit_transforms` preserves Flanker scale.
- [ ] Briefing, HUD, and results show primary + bonus objective for Missions 2/3.
- [ ] Bonuses alter credits only.
- [ ] M1 → M2 → M3 → M4 handoff works with save/Continue/Upgrade.
- [ ] Base-only completion yields 1200 total credits through Mission 3.
- [ ] README, CLAUDE.md, and `docs/validation/hpa-637.md` describe the final state.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --all-targets` passes.
- [ ] `cargo build --release` passes.

## Self-review

- **Spec coverage:** every HPA-637 acceptance item maps to Tasks 1–7 and the Final PR Gate.
- **Scope:** one bounded implementation PR; no independent subproject warrants another ticket/PR.
- **Placeholder scan:** implementation values, coordinates, copy, test expectations, and commands are explicit; final validation forbids placeholder evidence.
- **Type consistency:** `MissionRules`, `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `ObjectiveProgress::optional_complete`, and `MissionResult::optional_complete` use the same names throughout.
- **Mission consistency:** One unlocks Two, Two unlocks Three, Three unlocks Four; only One–Three are authored.
- **Encounter consistency:** Mission 2 opening is Rifleman→Vanguard, Striker→Gunner, Artillery→Gunner, Flanker→Interceptor; later Flanker pressure follows Gunner. Mission 3 escape is `(8,2)` and is visibly marked.
- **Reward consistency:** base 300/400/500 and bonuses 100/100/150; base-only completion reaches 1200 credits without grinding.
