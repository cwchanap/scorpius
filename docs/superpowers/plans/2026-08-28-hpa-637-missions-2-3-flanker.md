# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius from the completed Mission 1 campaign loop to three continuously playable missions, adding protect/intercept objectives and the Flanker enemy without a generic objective or AI framework.

**Architecture:** `BattleState` gains one closed `MissionRules` row describing only the objective/opening shapes Missions 1–3 consume; `mission` remains the typed authoring layer, with a small shared regular-enemy catalog and separate Mission 2/3 modules. Flanker remains one explicit branch in the deterministic enemy planner, while the existing campaign screens, save model, combat mechanics, and single Bevy application crate continue unchanged in shape.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, existing headless Rust test suites, checked-in `assets/models/mission_one.gltf` and existing VN PNGs only.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Global Constraints

- One Linear ticket (`HPA-637`) = one implementation PR; continue implementation on this draft PR.
- Keep dependency direction `presentation -> mission -> domain`; `src/domain/` must not import Bevy or campaign/presentation types.
- Keep one application crate; add no workspace/crate/plugin suite.
- Keep `bevy = "0.19"`; add no dependency.
- Preserve committed-intent semantics: player movement never retargets an already committed enemy intent.
- Keep mission content typed in Rust; add no RON/JSON/scripting/content framework.
- Add no behavior tree, utility-AI framework, stealth, teleportation, new initiative system, status framework, new playable unit, deployment selection, mission select, branching, difficulty, boss, or new hazard type.
- Mission 2 protects the existing Gunner; do not add a neutral faction/objective-unit role.
- Mission 3 uses one Flanker as Courier; escort clear is never a hidden victory condition.
- Optional objectives affect credits only and never gate progression.
- Reuse the existing VN art and existing glTF; add no art-generation pipeline.
- Keep save state to `next_mission`, credits, and upgrades; add no migration/version compatibility code.
- All automated tests stay headless.
- Final local gates remain `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, and `cargo build --release`.

---

## File Structure

### New files

- `src/mission/enemies.rs` — fixed regular-enemy constructors and their four weapon specs.
- `src/mission/mission_two.rs` — Mission 2 board, IDs, opening plan, rules, dialogue, rewards, tests.
- `src/mission/mission_three.rs` — Mission 3 board, IDs, opening plan, rules, dialogue, rewards, tests.
- `docs/validation/hpa-637.md` — final automated/manual validation evidence.

### Modified files

- `src/domain/model.rs` — closed primary/optional objective types, authored enemy opening row, `MissionRules`, generic optional result/progress bit, Flanker archetype.
- `src/domain/battle.rs` — store rules, objective-aware terminal evaluation, terminal optional completion.
- `src/domain/enemy.rs` — consume authored opening rows and add deterministic Flanker movement/target preference.
- `src/mission/mod.rs` — export new mission/enemy modules; grow `MissionId`/dispatch through Mission 4 handoff.
- `src/mission/mission_one.rs` — define Mission 1 rules/opening data and consume shared regular-enemy factories.
- `src/campaign/progression.rs` — consume generic `MissionResult::optional_complete`.
- `src/presentation/interaction.rs` — Flanker is never pilot-controllable; use generic presentation debug name where touched.
- `src/presentation/battlefield.rs` — Flanker fast silhouette/scale/under-ring and generic presentation root name.
- `src/presentation/ui.rs` — rule-aware objective progress, generic bonus event copy, definition-aware result overlay.
- `src/presentation/campaign_ui.rs` — generic reward/handoff copy and continuous authored-mission routing.
- `tests/presentation_app.rs` — campaign/battle entry/restart/routing integration through Mission 3.
- `README.md` — current three-mission player-facing campaign behavior.
- `CLAUDE.md` — current architecture/rules of record.

### Expected untouched unless a failing test proves otherwise

- `src/campaign/model.rs`
- `src/campaign/save.rs` implementation (tests may expand elsewhere/in-file)
- `src/campaign/session.rs`
- `src/app.rs`
- `assets/models/mission_one.gltf`
- `assets/vn/*`
- `Cargo.toml`
- `Cargo.lock`

---

### Task 1: Add closed mission rules and generic optional-result semantics

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/campaign/progression.rs`
- Modify: current call sites/tests that construct `BattleState` or read `turnabout_complete`

**Interfaces:**
- Produces: `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `MissionRules` in `domain::model`.
- Produces: `BattleState::rules(&self) -> MissionRules`.
- Changes: `BattleState::new(board, units, weapons, rules, seed)`.
- Changes: `ObjectiveProgress { optional_complete: bool }`.
- Changes: `MissionResult { victory, optional_complete, rounds }`.
- Preserves: `BattleEvent::OptionalObjectiveCompleted`.

- [ ] **Step 1: Write focused terminal-rule tests before changing the model**

Add tests in `src/domain/battle.rs` that construct a minimal battle fixture with one/two players and one enemy, then call the private `check_terminal_state()` directly from the child test module. The fixture must accept `MissionRules` so each rule can be tested without Bevy or mission authoring.

Use this exact rule coverage:

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
    assert_eq!(battle.result(), None, "enemy clear is not the primary objective");

    battle.phase = BattlePhase::EnemyPlanning;
    battle.round = 2;
    assert!(battle.check_terminal_state().is_empty());
    assert_eq!(battle.result(), None);

    battle.round = 3;
    let events = battle.check_terminal_state();
    assert!(matches!(battle.result(), Some(MissionResult { victory: true, .. })));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionCompleted { .. })));
}

#[test]
fn protect_rule_fails_immediately_when_target_is_knocked_out() {
    let mut battle = objective_fixture(PROTECT_RULE);
    let events = battle.apply_direct_damage(UnitId(2), 99, DamageSource::EnemyWeapon(UnitId(9), WeaponId(9)));
    assert!(matches!(battle.result(), Some(MissionResult { victory: false, .. })));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionFailed { .. })));
}

#[test]
fn intercept_rule_wins_on_target_ko_not_escort_clear() {
    let mut battle = intercept_fixture();
    battle.apply_direct_damage(UnitId(8), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None, "escort clear cannot win interception");

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

The fixture's protected `UnitId(2)` is a normal `Faction::Player` unit. Do not add a role/faction abstraction to make these tests pass.

- [ ] **Step 2: Run the focused test module and confirm it fails for missing rule types/behavior**

Run:

```bash
cargo test --lib domain::battle:: -- --nocapture
```

Expected: compilation/test failures because `MissionRules`, protect/intercept semantics, and generic result fields do not exist yet.

- [ ] **Step 3: Add the closed rule/value types and generic result field names**

In `src/domain/model.rs`, add exactly:

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

`GridPos` is already imported by `model.rs`; keep these plain Rust/serde-independent battle values.

- [ ] **Step 4: Store `MissionRules` in `BattleState` and change construction**

Add a private `rules: MissionRules` field. Change the constructor signature to:

```rust
pub(crate) fn new(
    board: BoardState,
    units: impl IntoIterator<Item = UnitState>,
    weapons: impl IntoIterator<Item = WeaponSpec>,
    rules: MissionRules,
    seed: u64,
) -> Self
```

Add:

```rust
pub const fn rules(&self) -> MissionRules {
    self.rules
}
```

For `viability_fixture()` and every existing non-mission fixture, pass:

```rust
MissionRules {
    primary: PrimaryObjective::EliminateAllEnemies,
    optional: OptionalObjective::Turnabout,
    opening_plan: &[],
}
```

Update `mission_one_for_campaign` to pass the same temporary rule literal in this task; Task 2 replaces the empty opening plan with authored Mission 1 rows.

- [ ] **Step 5: Implement objective-aware terminal evaluation without another counter**

Split the decision from terminal sealing with private helpers:

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
            let target = self
                .units
                .get(&target)
                .expect("authored protected target must exist");
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
            let target = self
                .units
                .get(&target)
                .expect("authored interception target must exist");
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

`check_terminal_state()` calls `primary_outcome()` and keeps the existing one-time `self.result.is_some()` guard plus terminal cleanup.

Do not make eliminate-all an alternate win condition for protect/intercept missions.

- [ ] **Step 6: Generalize optional completion**

Keep the existing damage observation call sites, but make the current Turnabout observer conditional on `OptionalObjective::Turnabout` and rename only internals as useful. Add:

```rust
fn optional_condition_met(&self) -> bool {
    match self.rules.optional {
        OptionalObjective::Turnabout => self.objectives.optional_complete,
        OptionalObjective::ProtectTargetAtHalfHp { target } => {
            let target = self
                .units
                .get(&target)
                .expect("authored bonus target must exist");
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

On terminal victory, before building `MissionResult`, evaluate `optional_condition_met()` and append `mark_optional_complete()` when true. Then store `optional_complete` in `MissionResult`. Do not newly mark terminal-only bonuses on defeat.

Turnabout's existing `observe_damage_for_objectives` calls `mark_optional_complete()` only when `self.rules.optional == OptionalObjective::Turnabout` and its existing damage/source predicates pass.

- [ ] **Step 7: Rename every active-code/test use of `turnabout_complete` to `optional_complete`**

At minimum update:

- `src/campaign/progression.rs` reward calculation;
- `src/presentation/ui.rs` current HUD/result reads (keep old Turnabout display wording temporarily; Task 5 generalizes copy);
- all Rust tests/fixtures constructing `MissionResult` or reading `ObjectiveProgress`.

Do not rewrite historical HPA-632/HPA-635 spec/validation docs; those document the state at those tickets.

- [ ] **Step 8: Run focused and full tests**

Run:

```bash
cargo fmt --check
cargo test --lib domain::battle::
cargo test --lib campaign::progression::
cargo test --all-targets
```

Expected: all pass; Mission 1 remains behaviorally unchanged under `EliminateAllEnemies`/`Turnabout`.

- [ ] **Step 9: Commit the objective foundation**

```bash
git add src/domain/model.rs src/domain/battle.rs src/campaign/progression.rs src/mission/mission_one.rs src/presentation/ui.rs tests

git commit -m "feat: add authored mission objective rules"
```

Only stage paths actually changed; do not manufacture a `tests` path if all test edits were inline.

---

### Task 2: Move opening plans into mission data and add the regular-enemy/Flanker seam

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
- Consumes: `BattleState::rules().opening_plan` during round-0 movement/intent commitment.
- Preserves: exact Mission 1 round-0 destinations/targets/intent order.

- [ ] **Step 1: Add regression tests that describe the data seam before deleting hardcoding**

In `src/domain/enemy.rs`, retain/extend `authored_opening_places_four_locked_threats` to assert all four Mission 1 positions and intended occupants:

```rust
assert_eq!(battle.intent_for(ids::RIFLEMAN_LEFT).unwrap().intended_occupant, Some(ids::GUNNER));
assert_eq!(battle.intent_for(ids::RIFLEMAN_RIGHT).unwrap().intended_occupant, Some(ids::INTERCEPTOR));
assert_eq!(battle.intent_for(ids::STRIKER).unwrap().intended_occupant, Some(ids::VANGUARD));
assert_eq!(battle.intent_for(ids::ARTILLERY).unwrap().intended_occupant, Some(ids::VANGUARD));
```

Also keep the existing ordered attackers `[STRIKER, RIFLEMAN_LEFT, RIFLEMAN_RIGHT, ARTILLERY]` and exact mortar footprint assertion.

- [ ] **Step 2: Run the Mission 1 enemy tests as a green characterization baseline**

```bash
cargo test --lib domain::enemy::authored_opening_places_four_locked_threats -- --exact
```

Expected: PASS before refactoring.

- [ ] **Step 3: Create the regular-enemy catalog**

Add `src/mission/enemies.rs` with these constants/factories, reusing `mission::squad::{stats, unit, weapon}`:

```rust
pub const SERVICE_RIFLE: WeaponId = WeaponId(201);
pub const SHOCK_CLAW: WeaponId = WeaponId(202);
pub const SIEGE_MORTAR: WeaponId = WeaponId(203);
pub const SKIRMISH_CARBINE: WeaponId = WeaponId(204);

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

Weapon factory values are exactly:

```rust
pub fn service_rifle() -> WeaponSpec {
    weapon(SERVICE_RIFLE, "Service Rifle", 2, 4, WeaponShape::Single, 5, 0, 5, 0, false, false)
}

pub fn shock_claw() -> WeaponSpec {
    weapon(SHOCK_CLAW, "Shock Claw", 1, 1, WeaponShape::Single, 7, 10, 10, 0, false, false)
}

pub fn siege_mortar() -> WeaponSpec {
    weapon(SIEGE_MORTAR, "Siege Mortar", 3, 8, WeaponShape::Cross1, 6, 5, 5, 0, false, false)
}

pub fn skirmish_carbine() -> WeaponSpec {
    weapon(SKIRMISH_CARBINE, "Skirmish Carbine", 1, 2, WeaponShape::Single, 4, 5, 10, 0, false, false)
}
```

Export `pub mod enemies;` from `mission/mod.rs`.

- [ ] **Step 4: Refactor Mission 1 to consume the shared catalog and authored opening rows**

Keep Mission 1 unit IDs exactly 11–14. Re-export the shared weapon IDs from `mission_one::ids` so existing tests/callers remain readable:

```rust
pub use crate::mission::enemies::{SERVICE_RIFLE, SHOCK_CLAW, SIEGE_MORTAR};
```

Replace duplicated enemy stats/weapons with the catalog factories.

Add:

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

Pass `MISSION_ONE_RULES` to `BattleState::new`.

- [ ] **Step 5: Replace Mission-1-specific opening logic in `enemy.rs`**

`apply_authored_opening_movement` becomes data lookup by enemy ID:

```rust
let opening = self
    .rules()
    .opening_plan
    .iter()
    .copied()
    .find(|opening| opening.unit == id);
let destination = opening.map_or(origin, |opening| opening.destination);
```

`commit_enemy_intents(true)` resolves a forced target from the matching row:

```rust
let forced_target = authored_opening
    .then(|| {
        self.rules()
            .opening_plan
            .iter()
            .find(|opening| opening.unit == attacker)
            .and_then(|opening| opening.target)
            .and_then(|target| self.unit(target))
            .filter(|target| !target.is_knocked_out())
            .map(|target| target.position)
    })
    .flatten();
```

Delete the old archetype/position opening destination match and `opening_target()` helper.

- [ ] **Step 6: Add `UnitArchetype::Flanker` and make all exhaustive matches compile without giving it pilot behavior**

In `model.rs`, add `Flanker` after `Artillery` or with the regular enemy variants.

Update presentation matches:

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Artillery
| UnitArchetype::Flanker => false
```

for `can_pilot`, and return `BattleError::PilotSkillWrongUnit` for Flanker in `interaction.rs`.

Temporarily map `scene_index(UnitArchetype::Flanker) => 2`; Task 5 adds scale/marker styling. Do not touch the glTF.

Leave the existing `initiative()` wildcard behavior for Flanker; do not add a new initiative concept/value as a Flanker feature.

- [ ] **Step 7: Write Flanker movement/target-selection tests in `enemy.rs`**

Use plain test fixtures with the rule values from Task 1. Pin both required contexts:

```rust
#[test]
fn flanker_in_protect_mission_prefers_protected_target_and_open_lane() {
    let battle = protect_flanker_fixture();
    let destination = choose_enemy_destination(&battle, UnitId(24)).unwrap();
    let gunner = battle.unit(UnitId(2)).unwrap();
    let weapon = battle.weapon(SKIRMISH_CARBINE).unwrap();

    assert_eq!(
        distance_to_band(destination.manhattan(gunner.position), weapon.min_range, weapon.max_range),
        0
    );
    assert!(open_neighbor_count(&battle, UnitId(24), destination) >= 3);
}

#[test]
fn courier_flanker_moves_toward_escape_instead_of_chasing_player() {
    let battle = intercept_flanker_fixture();
    let origin = battle.unit(UnitId(31)).unwrap().position;
    let destination = choose_enemy_destination(&battle, UnitId(31)).unwrap();
    let escape = GridPos::new(8, 2);

    assert!(destination.manhattan(escape) < origin.manhattan(escape));
}

#[test]
fn protect_flanker_prefers_gunner_when_gunner_is_a_legal_target() {
    let battle = protect_flanker_attack_fixture();
    let intent = build_intent(&battle, UnitId(24), None).unwrap();
    assert_eq!(intent.intended_occupant, Some(UnitId(2)));
}
```

The fixtures should deliberately place another player in an equally/closer generic-priority position so the assertions prove objective preference rather than ordinary `player_priority`.

- [ ] **Step 8: Run tests and confirm they fail before adding Flanker planner logic**

```bash
cargo test --lib domain::enemy::flanker -- --nocapture
cargo test --lib domain::enemy::courier -- --nocapture
```

Expected: new assertions fail because Flanker has no objective-aware destination/target preference yet.

- [ ] **Step 9: Implement only the two Flanker destination policies**

Add an `open_neighbor_count` helper using existing board bounds/blocking/live-explosive/unit occupancy rules. Do not create a pathfinding type.

For `UnitArchetype::Flanker` in `choose_enemy_destination`:

```rust
match battle.rules().primary {
    PrimaryObjective::ProtectThroughRound { target, .. } => {
        let target = battle.unit(target).ok_or(BattleError::UnknownUnit(target))?;
        let weapon = unit
            .weapons
            .first()
            .and_then(|weapon| battle.weapon(*weapon))
            .ok_or(BattleError::InvalidTarget(unit.position))?;
        Ok(*candidates
            .iter()
            .min_by_key(|position| {
                (
                    distance_to_band(
                        position.manhattan(target.position),
                        weapon.min_range,
                        weapon.max_range,
                    ),
                    position.manhattan(target.position),
                    Reverse(open_neighbor_count(battle, id, **position)),
                    position.y,
                    position.x,
                )
            })
            .unwrap())
    }
    PrimaryObjective::InterceptBeforeEscape { target, escape, .. } if target == id => {
        Ok(*candidates
            .iter()
            .min_by_key(|position| {
                (
                    position.manhattan(escape),
                    Reverse(open_neighbor_count(battle, id, **position)),
                    position.y,
                    position.x,
                )
            })
            .unwrap())
    }
    _ => choose_attack_band_destination(battle, id, &candidates, &players)?,
}
```

Extract the existing Rifleman/Striker attack-band scoring into `choose_attack_band_destination` only to avoid duplicating it for Flanker's fallback. Do not refactor Artillery.

- [ ] **Step 10: Prefer the protected unit in Flanker attack footprint ordering**

In `choose_target`, calculate:

```rust
let preferred = (attacker.archetype == UnitArchetype::Flanker)
    .then_some(battle.rules().primary)
    .and_then(|primary| match primary {
        PrimaryObjective::ProtectThroughRound { target, .. } => battle.unit(target).map(|unit| unit.position),
        _ => None,
    });
```

When choices contain player occupants, put a `misses_preferred` boolean before the existing `Reverse(threatened)` key. `false` sorts first:

```rust
let misses_preferred = preferred.is_some_and(|position| !choice.footprint.contains(&position));
(
    misses_preferred,
    Reverse(threatened),
    priority,
    choice.center.y,
    choice.center.x,
)
```

Do not change footprint locking after intent commitment.

- [ ] **Step 11: Re-run Mission 1 and Flanker planner tests**

```bash
cargo fmt --check
cargo test --lib domain::enemy::
cargo test --lib mission::mission_one::
```

Expected: all pass; Mission 1 opening remains byte-for-byte equivalent at the behavioral level and Flanker tests prove objective pressure/determinism.

- [ ] **Step 12: Commit the enemy data/AI seam**

```bash
git add src/domain/model.rs src/domain/enemy.rs src/mission/mod.rs src/mission/enemies.rs src/mission/mission_one.rs src/presentation/interaction.rs src/presentation/ui.rs src/presentation/battlefield.rs

git commit -m "feat: add authored enemy openings and flanker"
```

---

### Task 3: Author Mission 2 as the protect/survive vertical slice

**Files:**
- Create: `src/mission/mission_two.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs` only for the new `MissionId` exhaustiveness/generic handoff routing needed to keep the tree buildable

**Interfaces:**
- Produces: `mission_two(seed)` and `mission_two_for_campaign(seed, upgrades)`.
- Produces: `MISSION_TWO_DEFINITION`.
- Adds: `MissionId::Three` and `MissionId::number()` for One/Two/Three.
- Changes: `mission_definition(MissionId::Two) -> Some(&MISSION_TWO_DEFINITION)`; Three remains `None` until Task 4.

- [ ] **Step 1: Write Mission 2 authoring tests first**

In the new module, write tests that pin the design rather than broad snapshots:

```rust
#[test]
fn mission_two_authors_the_three_round_gunner_defense() {
    let battle = mission_two(7);
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
}

#[test]
fn mission_two_has_one_of_each_regular_enemy() {
    let battle = mission_two(7);
    let enemies: Vec<_> = battle.units().filter(|unit| unit.faction == Faction::Enemy).collect();
    assert_eq!(enemies.len(), 4);
    assert_eq!(enemies.iter().filter(|u| u.archetype == UnitArchetype::Rifleman).count(), 1);
    assert_eq!(enemies.iter().filter(|u| u.archetype == UnitArchetype::Striker).count(), 1);
    assert_eq!(enemies.iter().filter(|u| u.archetype == UnitArchetype::Artillery).count(), 1);
    assert_eq!(enemies.iter().filter(|u| u.archetype == UnitArchetype::Flanker).count(), 1);
}

#[test]
fn mission_two_definition_has_locked_copy_rewards_and_unlock() {
    assert_eq!(MISSION_TWO_DEFINITION.id, MissionId::Two);
    assert_eq!(MISSION_TWO_DEFINITION.unlocks, MissionId::Three);
    assert_eq!(MISSION_TWO_DEFINITION.title, "Mission 2 — Hold Relay Nine");
    assert_eq!(MISSION_TWO_DEFINITION.primary_objective, "Protect Gunner through the end of Round 3.");
    assert_eq!(MISSION_TWO_DEFINITION.optional_objective, "Hold Fast: finish with Gunner at or above 50% HP.");
    assert_eq!(MISSION_TWO_DEFINITION.base_reward, 400);
    assert_eq!(MISSION_TWO_DEFINITION.optional_reward, 100);
}
```

Also test all exact terrain cells from the spec and confirm `build_player_squad` upgrade projection by passing at least one nonzero Gunner upgrade and checking max HP/evasion/weapon damage.

- [ ] **Step 2: Run the new module test target and confirm it does not compile**

```bash
cargo test --lib mission::mission_two:: -- --nocapture
```

Expected: missing module/types/definition.

- [ ] **Step 3: Implement exact Mission 2 data**

Use these IDs inside `mission_two::ids`:

```rust
pub use crate::mission::squad::ids::{
    ANCHOR_CANNON, ARC_BLADE, BURST_MISSILE, GUNNER, INTERCEPTOR, OVERCHARGE_SHOT,
    PILE_LANCE, PULSE_CARBINE, RAIL_RIFLE, REPULSOR_RAM, VANGUARD, VECTOR_PULSE,
};

pub const RIFLEMAN: UnitId = UnitId(21);
pub const STRIKER: UnitId = UnitId(22);
pub const ARTILLERY: UnitId = UnitId(23);
pub const FLANKER: UnitId = UnitId(24);
```

Deployment:

```rust
const MISSION_TWO_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(3, 7),
    gunner: GridPos::new(4, 6),
    interceptor: GridPos::new(5, 7),
};
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

Enemy starting positions/factories:

```text
Rifleman  (2,2)
Striker   (4,3)
Artillery (4,0)
Flanker   (8,4)
```

Opening rows:

```rust
static MISSION_TWO_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(2,4), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4,5), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4,0), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::FLANKER, destination: GridPos::new(5,5), target: Some(ids::GUNNER) },
];
```

Weapons are exactly `service_rifle`, `shock_claw`, `siege_mortar`, `skirmish_carbine` from `mission::enemies`.

- [ ] **Step 4: Add the exact Mission 2 VN/definition constants**

Use the strings/assets from the spec without new PNGs. The definition is:

```rust
pub const MISSION_TWO_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Two,
    unlocks: MissionId::Three,
    build: mission_two_for_campaign,
    title: "Mission 2 — Hold Relay Nine",
    primary_objective: "Protect Gunner through the end of Round 3.",
    optional_objective: "Hold Fast: finish with Gunner at or above 50% HP.",
    base_reward: 400,
    optional_reward: 100,
    pre_mission: DialogueScene { background: "vn/relay_nine_bg.png", lines: &PRE_MISSION_LINES },
    aftermath: DialogueScene { background: "vn/relay_nine_bg.png", lines: &AFTERMATH_LINES },
};
```

Pre-mission lines:

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

Use `control_neutral.png`, `vanguard_neutral.png`, and `control_alert.png` in the same speaker pattern as Mission 1.

- [ ] **Step 5: Extend mission dispatch to Mission 2 and a Mission 3 handoff ID**

In `mission/mod.rs`:

```rust
pub mod mission_two;

pub enum MissionId {
    One,
    Two,
    Three,
}

impl MissionId {
    pub const fn number(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
}

pub fn mission_definition(id: MissionId) -> Option<&'static MissionDefinition> {
    match id {
        MissionId::One => Some(&mission_one::MISSION_ONE_DEFINITION),
        MissionId::Two => Some(&mission_two::MISSION_TWO_DEFINITION),
        MissionId::Three => None,
    }
}
```

- [ ] **Step 6: Make campaign UI routing compile and become authored-definition-driven**

Update `CampaignUiAction::Continue` now:

```rust
match continue_game(&mut runtime.0) {
    Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
    Ok(MissionId::Two | MissionId::Three) => next_state.set(GameScreen::Upgrade),
    Err(error) => status.0 = error.to_string(),
}
```

Change `Proceed` from unconditional `NextMission` to:

```rust
CampaignUiAction::Proceed => {
    let authored = runtime
        .0
        .state
        .as_ref()
        .and_then(|state| mission_definition(state.next_mission))
        .is_some();
    next_state.set(if authored {
        GameScreen::PreMissionStory
    } else {
        GameScreen::NextMission
    });
}
```

Make `next_mission_copy` use `state.next_mission.number()` rather than hard-coded Mission 2. With Mission 3 still unauthored in this task, completing Mission 2 still lands on a correct `MISSION 3 UNLOCKED` handoff.

- [ ] **Step 7: Add Mission 2 objective integration tests**

Use `mission_two(7)` and existing test-only state helpers/private child-module access:

```rust
#[test]
fn enemy_clear_does_not_complete_mission_two() {
    let mut battle = mission_two(7);
    for enemy in [ids::RIFLEMAN, ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
        battle.apply_direct_damage(enemy, 99, DamageSource::PlayerWeapon(ids::RAIL_RIFLE));
    }
    assert_eq!(battle.result(), None);
}

#[test]
fn gunner_ko_fails_mission_two() {
    let mut battle = mission_two(7);
    battle.apply_direct_damage(ids::GUNNER, 99, DamageSource::EnemyWeapon(ids::ARTILLERY, SIEGE_MORTAR));
    assert!(matches!(battle.result(), Some(MissionResult { victory: false, .. })));
}

#[test]
fn round_three_enemy_planning_completes_mission_two_and_scores_hp_bonus() {
    let mut battle = mission_two(7);
    battle.set_round_for_test(3);
    let events = battle.begin_round().unwrap();
    assert_eq!(battle.result().unwrap().victory, true);
    assert_eq!(battle.result().unwrap().optional_complete, true);
    assert!(matches!(events.as_slice(), [BattleEvent::OptionalObjectiveCompleted, BattleEvent::MissionCompleted { .. }]));
}
```

For the missed bonus test, set Gunner HP below half with nonlethal direct damage before the Round-3 terminal check and assert `optional_complete == false`.

- [ ] **Step 8: Run Mission 2 and regression suites**

```bash
cargo fmt --check
cargo test --lib mission::mission_two::
cargo test --lib domain::battle::
cargo test --lib domain::enemy::
cargo test --all-targets
```

Expected: all pass; no hidden destroy-all victory for Mission 2.

- [ ] **Step 9: Commit Mission 2**

```bash
git add src/mission/mission_two.rs src/mission/mod.rs src/presentation/campaign_ui.rs src/domain/battle.rs src/domain/enemy.rs

git commit -m "feat: add mission 2 gunner defense"
```

---

### Task 4: Author Mission 3 interception and complete campaign progression through it

**Files:**
- Create: `src/mission/mission_three.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/campaign/progression.rs` tests
- Modify: save/session tests where MissionId variants are enumerated

**Interfaces:**
- Produces: `mission_three(seed)` / `mission_three_for_campaign(seed, upgrades)`.
- Produces: `MISSION_THREE_DEFINITION`.
- Adds: `MissionId::Four`; `mission_definition(Three)` becomes authored; Four is the next handoff.
- Preserves: same `CampaignState` shape and same save file format structure.

- [ ] **Step 1: Write Mission 3 authoring/objective tests first**

Pin exact authoring:

```rust
#[test]
fn mission_three_authors_the_courier_interception() {
    let battle = mission_three(7);
    assert_eq!(battle.board().width(), 9);
    assert_eq!(battle.board().height(), 9);
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
}
```

Also pin deployment `(4,7)/(3,8)/(5,8)`, blocking `(4,3)/(4,4)/(4,5)`, hazard `(2,5)`, explosive `(6,3)`, and exactly three enemies (Courier/Rifleman/Striker).

- [ ] **Step 2: Add red objective tests for KO/escort-clear/extraction/deadline/bonus**

```rust
#[test]
fn courier_ko_wins_with_escorts_alive() {
    let mut battle = mission_three(7);
    battle.apply_direct_damage(ids::COURIER, 99, DamageSource::PlayerWeapon(ids::RAIL_RIFLE));
    assert_eq!(battle.result().unwrap().victory, true);
    assert!(battle.units().any(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out()));
}

#[test]
fn escort_clear_does_not_win_mission_three() {
    let mut battle = mission_three(7);
    battle.apply_direct_damage(ids::RIFLEMAN, 99, DamageSource::PlayerWeapon(ids::RAIL_RIFLE));
    battle.apply_direct_damage(ids::STRIKER, 99, DamageSource::PlayerWeapon(ids::RAIL_RIFLE));
    assert_eq!(battle.result(), None);
}

#[test]
fn courier_at_extraction_fails_mission_three() {
    let mut battle = mission_three(7);
    battle.move_unit_direct_for_test(ids::COURIER, GridPos::new(8, 2));
    let events = battle.begin_round().unwrap();
    assert!(matches!(battle.result(), Some(MissionResult { victory: false, .. })));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionFailed { .. })));
}

#[test]
fn round_four_deadline_fails_with_living_courier() {
    let mut battle = mission_three(7);
    battle.set_round_for_test(4);
    battle.begin_round().unwrap();
    assert!(matches!(battle.result(), Some(MissionResult { victory: false, .. })));
}

#[test]
fn swift_intercept_bonus_is_round_two_only() {
    let mut early = mission_three(7);
    early.set_round_for_test(2);
    early.apply_direct_damage(ids::COURIER, 99, DamageSource::PlayerWeapon(ids::RAIL_RIFLE));
    assert!(early.result().unwrap().optional_complete);

    let mut late = mission_three(7);
    late.set_round_for_test(3);
    late.apply_direct_damage(ids::COURIER, 99, DamageSource::PlayerWeapon(ids::RAIL_RIFLE));
    assert!(!late.result().unwrap().optional_complete);
}
```

- [ ] **Step 3: Run the Mission 3 test target and confirm missing implementation**

```bash
cargo test --lib mission::mission_three:: -- --nocapture
```

Expected: missing module/definition/ID failures.

- [ ] **Step 4: Implement exact Mission 3 data**

IDs:

```rust
pub const COURIER: UnitId = UnitId(31);
pub const RIFLEMAN: UnitId = UnitId(32);
pub const STRIKER: UnitId = UnitId(33);
```

Deployment:

```rust
const MISSION_THREE_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};
```

Board:

```rust
BoardState::new(
    9,
    9,
    [GridPos::new(4,3), GridPos::new(4,4), GridPos::new(4,5)],
    [GridPos::new(2,5)],
    [ExplosiveState { position: GridPos::new(6,3), hp: 4, exploded: false }],
)
```

Enemies:

```text
Courier/Flanker (0,6)
Rifleman         (3,2)
Striker          (6,6)
```

Opening:

```rust
static MISSION_THREE_OPENING: [EnemyOpening; 3] = [
    EnemyOpening { unit: ids::COURIER, destination: GridPos::new(0,6), target: None },
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(3,4), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(5,7), target: Some(ids::INTERCEPTOR) },
];
```

Rules/rewards/copy exactly match the spec:

```rust
const MISSION_THREE_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: GridPos::new(8,2),
        deadline_round: 4,
    },
    optional: OptionalObjective::VictoryByRound { round: 2 },
    opening_plan: &MISSION_THREE_OPENING,
};
```

Base 500, bonus 150, unlock Four.

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

Use the same `relay_nine_bg.png`/Control/Vanguard portraits; add no files under `assets/`.

- [ ] **Step 6: Extend `MissionId`/dispatch to Four and make Continue's handoff explicit**

```rust
pub enum MissionId {
    One,
    Two,
    Three,
    Four,
}

impl MissionId {
    pub const fn number(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
        }
    }
}

pub fn mission_definition(id: MissionId) -> Option<&'static MissionDefinition> {
    match id {
        MissionId::One => Some(&mission_one::MISSION_ONE_DEFINITION),
        MissionId::Two => Some(&mission_two::MISSION_TWO_DEFINITION),
        MissionId::Three => Some(&mission_three::MISSION_THREE_DEFINITION),
        MissionId::Four => None,
    }
}
```

Continue routing becomes:

```rust
Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
Ok(MissionId::Two | MissionId::Three) => next_state.set(GameScreen::Upgrade),
Ok(MissionId::Four) => next_state.set(GameScreen::NextMission),
```

The Task-3 `Proceed` authored-definition check now routes Three to its pre-story and Four to handoff automatically.

- [ ] **Step 7: Add a campaign progression test that advances all three missions without optional bonuses**

In `campaign/progression.rs` tests:

```rust
#[test]
fn normal_completion_advances_one_two_three_to_four_without_bonus_gating() {
    let mut state = CampaignState::new_game();

    for id in [MissionId::One, MissionId::Two, MissionId::Three] {
        let definition = mission_definition(id).unwrap();
        let before = state.credits;
        let receipt = state
            .complete_mission(
                definition,
                MissionResult {
                    victory: true,
                    optional_complete: false,
                    rounds: 3,
                },
            )
            .unwrap();
        assert_eq!(receipt.optional_reward, 0);
        assert_eq!(state.credits, before + definition.base_reward);
    }

    assert_eq!(state.next_mission, MissionId::Four);
    assert_eq!(state.credits, 300 + 400 + 500);
}
```

Add one separate case proving a true optional bit only adds that definition's optional reward and does not change the unlocked mission.

- [ ] **Step 8: Extend save serialization coverage through MissionId Four**

Use the existing `SaveFile` temp-path test style. Persist a state with `next_mission: MissionId::Four`, nonzero credits, and at least one nonzero upgrade; reload and assert exact equality.

Do not add a schema version/migration branch.

- [ ] **Step 9: Run Mission 3/campaign/save suites**

```bash
cargo fmt --check
cargo test --lib mission::mission_three::
cargo test --lib campaign::progression::
cargo test --lib campaign::save::
cargo test --all-targets
```

Expected: all pass; normal base rewards total 1200 credits through Mission 3 even with every bonus missed.

- [ ] **Step 10: Commit Mission 3 and campaign advancement**

```bash
git add src/mission/mission_three.rs src/mission/mod.rs src/presentation/campaign_ui.rs src/campaign/progression.rs src/campaign/save.rs

git commit -m "feat: add mission 3 courier interception"
```

Only stage `src/campaign/save.rs` if its inline tests changed.

---

### Task 5: Make battle/campaign presentation objective-generic and visibly mark Flankers

**Files:**
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/interaction.rs` if the debug root name remains Mission-1-specific

**Interfaces:**
- Changes: `result_overlay_copy(result, definition)` consumes authored objective text.
- Produces: rule-aware primary/bonus progress strings inside `HudSnapshot::from_battle`.
- Preserves: `MissionDefinition` as the only authored human-readable objective source.
- Produces: Flanker uses scene 2, scale 0.62, and persistent under-ring.

- [ ] **Step 1: Write pure presentation-copy tests first**

Add/adjust `ui.rs` tests for all rule shapes. Use real mission builders/definitions where convenient.

Mission 2 expected HUD substrings after `begin_round()`:

```rust
assert!(hud.primary.contains("Protect Gunner through the end of Round 3."));
assert!(hud.primary.contains("Round 1/3"));
assert!(hud.primary.contains("Gunner HP 12/12"));
assert!(hud.optional.contains("Hold Fast"));
assert!(hud.optional.contains("On track"));
```

Mission 3:

```rust
assert!(hud.primary.contains("Intercept Courier"));
assert!(hud.primary.contains("Round 1/4"));
assert!(hud.primary.contains("cells from extraction"));
assert!(hud.optional.contains("Swift Intercept"));
assert!(hud.optional.contains("Available"));
```

Result overlay:

```rust
assert_eq!(
    result_overlay_copy(
        MissionResult { victory: true, optional_complete: false, rounds: 3 },
        &MISSION_THREE_DEFINITION,
    ),
    "MISSION COMPLETE\nMission 3 — Cut the Courier\nPRIMARY  Intercept Courier before extraction or the end of Round 4. · Complete\nBONUS    Swift Intercept: defeat Courier by the end of Round 2. · Missed"
);
```

Aftermath reward test changes expected label from `Turnabout +100` to `Bonus +100`.

- [ ] **Step 2: Run the pure UI tests and confirm expected failures**

```bash
cargo test --lib presentation::ui:: -- --nocapture
cargo test --lib presentation::campaign_ui:: -- --nocapture
```

Expected: failures on hard-coded enemy-count/Turnabout/Relay Nine/Mission 2 strings.

- [ ] **Step 3: Implement rule-aware objective progress inside `HudSnapshot::from_battle`**

Keep all human-readable base copy from `definition.primary_objective` / `optional_objective`.

Primary suffixes:

```rust
let primary_progress = match battle.rules().primary {
    PrimaryObjective::EliminateAllEnemies => {
        let remaining = battle.units()
            .filter(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out())
            .count();
        format!("{remaining} enemies remaining")
    }
    PrimaryObjective::ProtectThroughRound { target, round } => {
        let target = battle.unit(target).expect("authored protected target must exist");
        format!(
            "Round {}/{} · {} HP {}/{}",
            battle.round().min(round),
            round,
            target.name,
            target.hp,
            target.stats.max_hp,
        )
    }
    PrimaryObjective::InterceptBeforeEscape { target, escape, deadline_round } => {
        let target = battle.unit(target).expect("authored interception target must exist");
        format!(
            "Round {}/{} · {} cells from extraction",
            battle.round().min(deadline_round),
            deadline_round,
            target.position.manhattan(escape),
        )
    }
};
```

Set `primary = format!("{} · {primary_progress}", definition.primary_objective)`.

Bonus status:

```rust
let optional_status = if battle.result().is_some() {
    if battle.objectives().optional_complete { "Achieved" } else { "Missed" }
} else {
    match battle.rules().optional {
        OptionalObjective::Turnabout => {
            if battle.objectives().optional_complete { "Complete" } else { "Not yet" }
        }
        OptionalObjective::ProtectTargetAtHalfHp { target } => {
            let target = battle.unit(target).expect("authored bonus target must exist");
            if target.hp * 2 >= target.stats.max_hp { "On track" } else { "Missed" }
        }
        OptionalObjective::VictoryByRound { round } => {
            if battle.round() <= round { "Available" } else { "Missed" }
        }
    }
};
```

- [ ] **Step 4: Make terminal/event copy generic**

Change `result_overlay_copy` to:

```rust
pub fn result_overlay_copy(result: MissionResult, definition: &MissionDefinition) -> String {
    format!(
        "{}\n{}\nPRIMARY  {} · {}\nBONUS    {} · {}",
        if result.victory { "MISSION COMPLETE" } else { "MISSION FAILED" },
        definition.title,
        definition.primary_objective,
        if result.victory { "Complete" } else { "Failed" },
        definition.optional_objective,
        if result.optional_complete { "Achieved" } else { "Missed" },
    )
}
```

`update_hud` passes `active_mission.0` alongside the result.

Change:

```rust
BattleEvent::OptionalObjectiveCompleted => "BONUS OBJECTIVE COMPLETE".to_owned(),
```

Change `aftermath_reward_copy` to render `Bonus +{optional_reward}` rather than `Turnabout +...`.

- [ ] **Step 5: Add pure visual-profile assertions for Flanker**

Keep `scene_index(UnitArchetype::Flanker) == 2`. Add a tiny pure helper only if needed to avoid duplicated scale literals:

```rust
pub const fn unit_scale(archetype: UnitArchetype) -> f32 {
    match archetype {
        UnitArchetype::Flanker => 0.62,
        _ => 0.72,
    }
}
```

Test:

```rust
assert_eq!(scene_index(UnitArchetype::Flanker), 2);
assert_eq!(unit_scale(UnitArchetype::Flanker), 0.62);
assert_eq!(unit_scale(UnitArchetype::Striker), 0.72);
```

- [ ] **Step 6: Render the persistent Flanker marker with existing visual assets**

When spawning each unit in `populate_mission_root`, retain the existing `WorldAssetRoot` entity. For `UnitArchetype::Flanker`, spawn a sibling under `PresentationRoot` at the same grid position:

```rust
commands.spawn((
    Name::new(format!("{} Flanker Marker", unit.name)),
    Mesh3d(visual_assets.ring_mesh.clone()),
    MeshMaterial3d(visual_assets.telegraph_edge.clone()),
    Transform::from_translation(grid_to_world(unit.position) + Vec3::Y * 0.03),
    Pickable::IGNORE,
    ChildOf(root),
));
```

Use `unit_scale(unit.archetype)` for the unit model transform. Do not add a new material or asset file.

Because the scene is rebuilt at battle entry/restart and unit transforms can move during battle, ensure the Flanker marker follows movement. The smallest correct approach is to give it a marker component keyed by `UnitId` (for example `FlankerVisual(UnitId)`) and update it alongside `UnitVisual` in the existing transform sync path. Do not create a general effect/attachment framework.

- [ ] **Step 7: Remove Mission-1-only debug entity names where touched**

Use `Name::new("Mission Presentation")` in `spawn_presentation_root` and restart rebuild roots instead of `Mission 1 Presentation`.

Do not rename `mission_one.gltf` or its constants in this ticket.

- [ ] **Step 8: Run presentation tests**

```bash
cargo fmt --check
cargo test --lib presentation::ui::
cargo test --lib presentation::campaign_ui::
cargo test --test presentation_app
```

Expected: all pass; no renderer/window is required.

- [ ] **Step 9: Commit generic objective presentation and Flanker visual**

```bash
git add src/presentation/ui.rs src/presentation/campaign_ui.rs src/presentation/battlefield.rs src/presentation/interaction.rs

git commit -m "feat: present mission objectives and flanker pressure"
```

---

### Task 6: Prove continuous M1 → M2 → M3 campaign entry, restart, and save/upgrade routing

**Files:**
- Modify: `tests/presentation_app.rs`
- Modify: `src/presentation/campaign_ui.rs` only if tests expose a routing defect
- Modify: `src/app.rs` only if tests expose a mission-generic battle-entry defect
- Modify: `src/presentation/interaction.rs` only if restart is not mission-generic

**Interfaces:**
- Consumes: existing `enter_battle`, `ActiveMission`, `definition.build`, `CampaignRuntime`, `persist_purchase`.
- Verifies: no new application state/resource is necessary for Missions 2–3.

- [ ] **Step 1: Add a renderer-free battle-entry test for Mission 2**

Follow the existing bare-App fixture pattern in `tests/presentation_app.rs`. Set campaign state to `next_mission: MissionId::Two` with a nonzero upgrade, call the current battle-entry path, then assert:

```rust
assert_eq!(app.world().resource::<ActiveMission>().0.id, MissionId::Two);
let battle = &app.world().resource::<BattleRuntime>().0;
assert_eq!(battle.rules().primary, PrimaryObjective::ProtectThroughRound { target: squad::ids::GUNNER, round: 3 });
assert_eq!(battle.round(), 1);
assert!(battle.unit(squad::ids::GUNNER).unwrap().stats.max_hp > 12);
```

Use a Gunner HP upgrade to prove the persisted upgrade is projected into the Mission 2 builder.

- [ ] **Step 2: Add the corresponding Mission 3 entry/restart test**

Start at `MissionId::Three`, enter battle, capture the current `ActiveMission`, mutate battle state enough to differ from a fresh build, call the existing restart path with a fixed seed, then assert:

```rust
assert_eq!(active.id, MissionId::Three);
assert_eq!(battle.rules().primary, PrimaryObjective::InterceptBeforeEscape {
    target: mission_three::ids::COURIER,
    escape: GridPos::new(8, 2),
    deadline_round: 4,
});
assert_eq!(battle.round(), 0); // before restarted-round system, matching current restart contract
assert_eq!(battle.unit(mission_three::ids::COURIER).unwrap().hp, 8);
```

Then run the existing `begin_restarted_round` seam/system if that is how current integration tests finish restart, and assert Round 1 plus committed intents.

Do not special-case Mission 2/3 in `restart_battle`; it must continue to call `ActiveMission.0.build`.

- [ ] **Step 3: Add pure campaign-action routing tests for every saved mission ID**

Drive `apply_campaign_action` with a temp-save-backed `CampaignRuntime` and `NextState<GameScreen>`:

```text
Continue One   -> PreMissionStory
Continue Two   -> Upgrade
Continue Three -> Upgrade
Continue Four  -> NextMission
Proceed Two    -> PreMissionStory
Proceed Three  -> PreMissionStory
Proceed Four   -> NextMission
```

The Proceed cases use the already-loaded runtime state and do not write the save.

- [ ] **Step 4: Add a complete progression/session persistence test through Mission 3**

Using the existing temp save/session helpers:

1. `start_new_game`.
2. complete Mission 1 with `optional_complete = false`.
3. persist one 200-credit upgrade; assert credits become 100.
4. complete Mission 2 with `optional_complete = false`; assert credits become 500.
5. persist one 400-credit second-level upgrade on the same track (or two 200 first-level upgrades if current level/cost setup makes that clearer); assert the saved state exactly matches expected credits/upgrades.
6. complete Mission 3 with `optional_complete = false`.
7. reload with a fresh session and assert `next_mission == MissionId::Four` and all purchased upgrades remain.

Do not require Mission 2/3 bonuses for any purchase/progression assertion.

- [ ] **Step 5: Run integration and full tests**

```bash
cargo fmt --check
cargo test --test presentation_app
cargo test --all-targets
```

Expected: all pass without touching `app.rs` or session/save implementation. If a test proves a current generic seam has a real Mission-2/3 assumption, make the smallest local fix and keep the same interfaces.

- [ ] **Step 6: Commit campaign integration coverage**

```bash
git add tests/presentation_app.rs src/presentation/campaign_ui.rs src/app.rs src/presentation/interaction.rs

git commit -m "test: cover campaign progression through mission 3"
```

Only stage source files if the new tests required a real correction.

---

### Task 7: Update player/developer docs, perform manual tuning, and record final validation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-637.md`
- Modify: `src/mission/mission_two.rs` / `mission_three.rs` only if the playtest shows authored tuning problems

**Interfaces:**
- Produces: current project documentation and a reproducible HPA-637 validation ledger.
- Does not change architecture unless an automated acceptance test is missing; playtest corrections should prefer authored numbers/positions over new systems.

- [ ] **Step 1: Update README campaign description and objective sections**

Replace the HPA-635 one-mission summary with:

```text
Title → pre-mission VN → briefing → Mission 1 → aftermath/upgrades → Mission 2 → aftermath/upgrades → Mission 3 → Mission 4 unlocked handoff.
```

Document:

- Mission 1: eliminate all, Turnabout, 300 + 100.
- Mission 2: protect Gunner through Round 3, half-HP bonus, 400 + 100.
- Mission 3: stop Courier before extraction/end Round 4, Round-2 bonus, 500 + 150.
- Continue routing: active first mission story, inter-mission saves resume at Upgrade, post-Mission-3 save shows Mission 4 handoff.
- Flanker: high movement/evasion, low durability, objective-seeking movement.
- The combat controls/pilot skills are unchanged.
- Validation links include `docs/validation/hpa-637.md`.

Delete the statement that Mission 2 content is not in the build.

- [ ] **Step 2: Bring `CLAUDE.md` current**

Correct its stale claim that the game boots straight into Mission 1 with no campaign/save. State the actual architecture:

- Title/VN/Briefing/Battle/Aftermath/Upgrade/NextMission flow exists.
- `campaign` is plain Rust save/progression/session state.
- `mission_definition` dispatches Missions 1–3; Four is handoff.
- `BattleState` owns `MissionRules` with the three closed primary objective shapes and generic optional bit.
- `mission::enemies` owns four regular enemy archetypes; Flanker has one explicit objective-aware planner branch.
- Mission 2 protects Gunner; Mission 3 intercepts Courier.
- Reference the HPA-637 design/plan/validation docs alongside HPA-632/HPA-635.

Keep the existing command/testing/determinism/committed-intent guidance.

- [ ] **Step 3: Run all automated gates before manual play**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Record exact command results/summary counts in `docs/validation/hpa-637.md`.

Expected: all four commands exit 0.

- [ ] **Step 4: Manual playtest Mission 2 success/failure/bonus behavior**

Run:

```bash
cargo run
```

Record evidence for all of these concrete cases:

```text
M2-A: enter from M1 aftermath -> Upgrade -> Proceed -> Mission 2 VN/briefing.
M2-B: first round visibly shows Rifleman/Striker/Artillery/Flanker locked pressure on Gunner.
M2-C: move/Guard/Evade/Aegis can alter that pressure without any new tutorial mechanic.
M2-D: KO all enemies before Round 3; battle does not immediately win.
M2-E: keep Gunner alive through the third enemy resolution; battle wins even if at least one enemy remains.
M2-F: KO Gunner before completion; battle fails.
M2-G: finish with Gunner >= 50% max HP; bonus is achieved and reward +100.
M2-H: finish with Gunner < 50% max HP; mission still advances, bonus reward is 0.
```

If M2 requires repeatedly passing empty rounds in ordinary play, tune enemy HP/positions/opening cells in `mission_two.rs`; do not add reinforcements/waves in this ticket.

- [ ] **Step 5: Manual playtest Mission 3 interception/Flanker behavior**

Record:

```text
M3-A: Proceed from the post-M2 Upgrade enters Mission 3 VN/briefing.
M3-B: Courier is visibly the Flanker silhouette/under-ring and moves toward extraction/open lanes.
M3-C: Courier movement does not retarget already committed footprints during the player phase.
M3-D: defeating Courier wins while Rifleman/Striker can remain alive.
M3-E: defeating only escorts does not win.
M3-F: Courier reaching (8,2) fails the mission.
M3-G: body-block/delay through the end of Round 4 still fails on deadline.
M3-H: Courier defeat by end Round 2 grants +150; Round 3+ victory still advances with +0 bonus.
M3-I: push/collision/hazard/explosive/reactions remain available and useful; no new subsystem is needed.
```

If the Courier reaches extraction before the player has a meaningful interception opportunity, tune only Mission 3 positions/terrain or Flanker authored stats inside the locked design envelope. Keep movement high relative to Rifleman/Striker and durability lower.

- [ ] **Step 6: Manual save/continue/upgrade continuity**

At least once after Mission 1 and once after Mission 2:

1. return/quit after the reward is persisted;
2. relaunch;
3. Continue;
4. verify Upgrade is shown with exact saved credits/levels;
5. Proceed and verify the next mission receives those upgrades.

After Mission 3, relaunch/Continue and verify the generic handoff says `MISSION 4 UNLOCKED`.

- [ ] **Step 7: Complete `docs/validation/hpa-637.md`**

Use this structure with concrete evidence, not unchecked boxes:

```markdown
# HPA-637 Validation

## Baseline
- Branch / head SHA:
- Design:
- Plan:

## Automated gates
- `cargo fmt --check`: PASS (...)
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS (...)
- `cargo test --all-targets`: PASS (... test count ...)
- `cargo build --release`: PASS (...)

## Mission 2
### Opening pressure
<observed positions/intents>
### Protect success
<round/result evidence>
### Protect failure
<Gunner KO evidence>
### Bonus achieved/missed
<HP/reward evidence>

## Mission 3
### Courier movement/visual
<observed route/marker>
### Interception success
<escort-alive evidence>
### Extraction/deadline failures
<observed result evidence>
### Bonus achieved/missed
<round/reward evidence>

## Campaign/save/upgrades
<Continue/Upgrade/Proceed and Mission 4 handoff evidence>

## Short-session verdict
<round counts and whether any low-pressure stalling was required>
```

Replace every angle-bracket line with the actual observed value before committing; do not leave template markers in the final file.

- [ ] **Step 8: Re-run all gates after any playtest tuning/doc edits**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Expected: all exit 0 after the final authored tuning.

- [ ] **Step 9: Commit final docs/validation/tuning**

```bash
git add README.md CLAUDE.md docs/validation/hpa-637.md src/mission/mission_two.rs src/mission/mission_three.rs

git commit -m "docs: validate HPA-637 missions 2 and 3"
```

Only stage mission files if manual validation actually changed their authored tuning.

---

## Final PR Gate

Before marking the draft ready for review, verify all of the following from the actual branch diff and test output:

- [ ] Only HPA-637 scope is present; there is one PR for the ticket.
- [ ] `Cargo.toml`/`Cargo.lock` have no new dependency.
- [ ] No objective callback/trait/registry or behavior-tree/policy framework was introduced.
- [ ] No neutral faction/objective role exists; Mission 2 protects Gunner.
- [ ] Mission 1 exact opening intent regression passes.
- [ ] Mission 2 cannot win from enemy clear and cannot continue after Gunner KO.
- [ ] Mission 2 succeeds after the full third enemy resolution with Gunner alive.
- [ ] Mission 3 wins on Courier KO with escorts alive.
- [ ] Mission 3 loses on extraction and Round-4 deadline.
- [ ] Flanker planner is deterministic and objective-aware, with no RNG call added to movement/target selection.
- [ ] Flanker uses existing scene 2 + scale/under-ring; no new asset file/pipeline.
- [ ] Briefing, HUD, and result overlay each show primary and bonus objectives for Missions 2/3.
- [ ] Bonus completion changes credits only.
- [ ] M1 → M2 → M3 → M4 handoff works with save/Continue/Upgrade.
- [ ] `README.md`, `CLAUDE.md`, and `docs/validation/hpa-637.md` describe the shipped state.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --all-targets` passes.
- [ ] `cargo build --release` passes.

## Self-review performed while writing this plan

- Spec coverage: every HPA-637 acceptance item maps to Tasks 1–7 and the Final PR Gate.
- Scope: one bounded PR; no separate objective/AI/content subprojects are needed because each seam exists solely to make the two authored missions work.
- Placeholder scan: implementation steps use concrete types, values, coordinates, copy, commands, and expected assertions; final validation explicitly forbids leaving template markers.
- Type consistency: `MissionRules`, `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `ObjectiveProgress::optional_complete`, and `MissionResult::optional_complete` use the same names throughout all tasks.
- Campaign consistency: Mission 1 unlocks Two, Mission 2 unlocks Three, Mission 3 unlocks Four; only One–Three have definitions after Task 4.
- Reward consistency: base rewards are 300/400/500; optional rewards 100/100/150; normal no-bonus completion reaches 1200 credits and never requires grinding.
