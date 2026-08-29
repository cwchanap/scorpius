# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius from the completed Mission 1 campaign loop to three continuously playable missions, adding a three-round Gunner defense, a Courier interception mission with a real Round-4 window, and the Flanker enemy without introducing a generic objective or AI framework.

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
- Mission 3 Courier starts `(0,6)`, extraction is `(8,0)`, deadline is Round 4. The 14-step open path is a locked rule because it guarantees player Round 4 exists after three move-4 later-round passes.
- Optional objectives affect credits only and never gate progression.
- Reuse the existing VN art and glTF; add no asset-generation pipeline.
- Keep save state to `next_mission`, credits, and upgrades; add no migration/version compatibility code.
- All automated tests stay headless.
- Final gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `cargo build --release`.

---

## File Structure

### New files

- `src/mission/enemies.rs` — fixed regular-enemy constructors and four weapon specs.
- `src/mission/mission_two.rs` — Mission 2 board, IDs, opening, rules, dialogue, rewards, lifecycle tests.
- `src/mission/mission_three.rs` — Mission 3 board, IDs, opening, rules, dialogue, rewards, round-clock lifecycle tests.
- `docs/validation/hpa-637.md` — final automated/manual validation evidence.

### Modified files

- `src/domain/model.rs` — closed mission-rule types, generic bonus state/result, Flanker archetype.
- `src/domain/battle.rs` — store rules and evaluate protect/intercept terminal conditions.
- `src/domain/enemy.rs` — consume authored opening rows; add Flanker protect/intercept/fallback movement and target preference.
- `src/mission/mod.rs` — modules, MissionId 1–4, definition dispatch.
- `src/mission/mission_one.rs` — consume shared enemy catalog and authored Mission 1 rules/opening.
- `src/campaign/progression.rs` — generic bonus reward bit.
- `src/presentation/battlefield.rs` — Flanker scene/scale/rings, extraction ring, generic debug name.
- `src/presentation/sync.rs` — preserve archetype-specific scale during unit sync.
- `src/presentation/interaction.rs` — Flanker exhaustive match and generic debug root name if touched.
- `src/presentation/ui.rs` — rule-aware objective progress and generic result/event copy.
- `src/presentation/campaign_ui.rs` — continuous mission routing and generic reward/handoff copy.
- `tests/presentation_app.rs` — renderer-free campaign/battle integration through Mission 3.
- `tests/campaign_flow.rs` / `tests/campaign_persistence.rs` — routing/save enum coverage where the existing seam lives.
- `README.md` — current three-mission player-facing behavior.
- `CLAUDE.md` — current architecture/rules of record.

### Expected untouched unless a failing integration test proves otherwise

- `src/campaign/model.rs` shape.
- `src/campaign/save.rs` implementation.
- `src/campaign/session.rs` implementation.
- `src/app.rs`.
- `assets/models/mission_one.gltf`.
- `assets/vn/*`.
- `Cargo.toml` / `Cargo.lock`.

---

### Task 1: Add closed mission rules and generic bonus semantics

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/campaign/progression.rs`
- Modify: `src/mission/mission_one.rs`
- Modify: `src/presentation/ui.rs`
- Modify: active Rust tests that construct `BattleState` or read `turnabout_complete`

**Interfaces:**
- Produces: `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `MissionRules`.
- Produces: `BattleState::rules(&self) -> MissionRules`.
- Changes: `BattleState::new(board, units, weapons, rules, seed)`.
- Changes: `ObjectiveProgress { optional_complete: bool }`.
- Changes: `MissionResult { victory, optional_complete, rounds }`.
- Preserves: `BattleEvent::OptionalObjectiveCompleted`.

- [ ] **Step 1: Write focused terminal-rule tests before changing the model**

Inside `src/domain/battle.rs` tests, add a plain `objective_fixture(rules)` with two players and two enemies. Keep these direct `check_terminal_state` tests because they pin individual match arms:

```rust
#[test]
fn protect_rule_ignores_enemy_clear_and_fails_on_protected_target_ko() {
    let mut battle = objective_fixture(PROTECT_RULE);
    battle.apply_direct_damage(UnitId(8), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    battle.apply_direct_damage(UnitId(9), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None);

    battle.apply_direct_damage(
        UnitId(2),
        99,
        DamageSource::EnemyWeapon(UnitId(9), WeaponId(9)),
    );
    assert!(matches!(battle.result(), Some(MissionResult { victory: false, .. })));
}

#[test]
fn protect_rule_only_wins_at_enemy_planning_round_boundary() {
    let mut battle = objective_fixture(PROTECT_RULE);
    battle.phase = BattlePhase::EnemyPlanning;
    battle.round = 2;
    assert!(battle.check_terminal_state().is_empty());
    battle.round = 3;
    assert!(matches!(
        battle.check_terminal_state().as_slice(),
        [BattleEvent::OptionalObjectiveCompleted, BattleEvent::MissionCompleted { .. }]
            | [BattleEvent::MissionCompleted { .. }]
    ));
}

#[test]
fn intercept_rule_wins_on_target_ko_and_not_escort_clear() {
    let mut battle = objective_fixture(INTERCEPT_RULE);
    battle.apply_direct_damage(UnitId(8), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None);
    battle.apply_direct_damage(UnitId(9), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert!(matches!(battle.result(), Some(MissionResult { victory: true, .. })));
}

#[test]
fn intercept_rule_fails_on_escape_or_deadline() {
    let mut escaped = objective_fixture(INTERCEPT_RULE);
    escaped.units.get_mut(&UnitId(9)).unwrap().position = GridPos::new(8, 0);
    assert!(matches!(escaped.check_terminal_state().as_slice(), [BattleEvent::MissionFailed { .. }]));

    let mut timed_out = objective_fixture(INTERCEPT_RULE);
    timed_out.phase = BattlePhase::EnemyPlanning;
    timed_out.round = 4;
    assert!(matches!(timed_out.check_terminal_state().as_slice(), [BattleEvent::MissionFailed { .. }]));
}
```

The full round-machine behavior is tested against real Mission 2/3 constructors in Tasks 3 and 4; these private tests are not substitutes for those lifecycle tests.

- [ ] **Step 2: Run the focused module red**

```bash
cargo test --lib domain::battle:: -- --nocapture
```

Expected: compile failure for the missing rule/result types.

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

Generic fixtures use eliminate-all + Turnabout + empty opening until Task 2 moves Mission 1 opening data into rules.

- [ ] **Step 5: Implement objective-aware terminal evaluation**

Add private `primary_outcome(&self) -> Option<bool>`:

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

`check_terminal_state` seals only this outcome. Do not add eliminate-all as a fallback for protect/intercept missions.

- [ ] **Step 6: Implement one generic bonus bit**

Keep Turnabout's existing damage-source predicates in `observe_damage_for_objectives`, but only run them for `OptionalObjective::Turnabout`.

Add:

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
```

On terminal victory, mark a newly satisfied terminal bonus and emit `OptionalObjectiveCompleted` immediately before `MissionCompleted`. Defeat never newly marks terminal-only bonuses.

- [ ] **Step 7: Rename active Rust code from `turnabout_complete` to `optional_complete`**

Update campaign reward calculation, current presentation reads, and active Rust tests/fixtures. Do not rewrite historical HPA-632/HPA-635 spec/validation documents.

- [ ] **Step 8: Run focused/full regression tests**

```bash
cargo fmt --check
cargo test --lib domain::battle::
cargo test --all-targets
```

- [ ] **Step 9: Commit**

```bash
git add src/domain/model.rs src/domain/battle.rs src/campaign/progression.rs src/mission/mission_one.rs src/presentation/ui.rs tests

git commit -m "feat: add authored mission objective rules"
```

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
- Produces shared weapon constants/specs through `SKIRMISH_CARBINE` plus existing three enemy weapons.
- Adds: `UnitArchetype::Flanker`.
- Consumes: `BattleState::rules().opening_plan` for round-0 movement/intents.
- Produces: local `choose_attack_band_destination(...)` reused by Rifleman/Striker and non-objective Flanker fallback.
- Preserves: exact Mission 1 round-0 positions/targets/intent order.

- [ ] **Step 1: Strengthen Mission 1 opening characterization before refactoring**

Keep the current exact positions, intent order `[STRIKER, RIFLEMAN_LEFT, RIFLEMAN_RIGHT, ARTILLERY]`, and mortar footprint. Add intended occupants:

```rust
assert_eq!(battle.intent_for(ids::RIFLEMAN_LEFT).unwrap().intended_occupant, Some(ids::GUNNER));
assert_eq!(battle.intent_for(ids::RIFLEMAN_RIGHT).unwrap().intended_occupant, Some(ids::INTERCEPTOR));
assert_eq!(battle.intent_for(ids::STRIKER).unwrap().intended_occupant, Some(ids::VANGUARD));
assert_eq!(battle.intent_for(ids::ARTILLERY).unwrap().intended_occupant, Some(ids::VANGUARD));
```

Run it green before refactor:

```bash
cargo test --lib domain::enemy::tests::authored_opening_places_four_locked_threats -- --exact
```

- [ ] **Step 2: Create `mission::enemies` with exact existing values + Flanker**

Use shared weapon IDs 201–204 and `mission::squad::{stats, unit, weapon}`.

```rust
rifleman:  stats(9, 1, 2, 72, 5, 0), Service Rifle
striker:   stats(12, 2, 2, 78, 10, 0), Shock Claw
artillery: stats(10, 1, 1, 90, 0, 0), Siege Mortar
flanker:   stats(8, 0, 4, 82, 30, 0), Skirmish Carbine
```

Skirmish Carbine:

```rust
weapon(SKIRMISH_CARBINE, "Skirmish Carbine", 1, 2,
       WeaponShape::Single, 4, 5, 10, 0, false, false)
```

Keep existing enemy weapon values bit-identical.

- [ ] **Step 3: Refactor Mission 1 to exact authored opening data**

```rust
static MISSION_ONE_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN_LEFT, destination: GridPos::new(2,5), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::RIFLEMAN_RIGHT, destination: GridPos::new(6,5), target: Some(ids::INTERCEPTOR) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4,6), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4,0), target: Some(ids::VANGUARD) },
];
```

Mission 1 rules are eliminate-all + Turnabout + this opening slice.

- [ ] **Step 4: Delete Mission-1-specific opening matches from `enemy.rs`**

Round-0 movement looks up `EnemyOpening` by enemy ID and uses `destination`; forced opening target resolves the authored `UnitId` to its current living position. Delete the old archetype/x-position opening match and `opening_target()` helper.

- [ ] **Step 5: Add Flanker exhaustive matches without adding capabilities**

`UnitArchetype::Flanker` is enemy-only. Reject it in pilot-skill paths. Map `scene_index(Flanker) => 2`; Task 5 owns visual scale/rings. Keep initiative on the existing default/wildcard value.

- [ ] **Step 6: Write red Flanker planner tests including the non-objective fallback**

Pin objective behavior:

```rust
#[test]
fn protect_flanker_moves_into_band_of_the_protected_target() { /* fixture -> distance_to_band == 0 */ }

#[test]
fn protect_flanker_prefers_protected_target_when_legal() { /* intended_occupant == Gunner */ }

#[test]
fn courier_flanker_reduces_distance_to_escape() { /* destination distance < origin distance */ }
```

Also add the regression missing from the first draft:

```rust
#[test]
fn non_objective_flanker_uses_normal_attack_band_movement() {
    let battle = eliminate_all_flanker_fixture();
    let flanker = battle.unit(UnitId(24)).unwrap();
    let destination = choose_enemy_destination(&battle, flanker.id).unwrap();
    assert_ne!(destination, flanker.position);

    let weapon = battle.weapon(SKIRMISH_CARBINE).unwrap();
    let nearest = living_players(&battle)
        .iter()
        .map(|player| destination.manhattan(player.position))
        .min()
        .unwrap();
    assert_eq!(distance_to_band(nearest, weapon.min_range, weapon.max_range), 0);
}
```

Use a fixture with an open legal move into range so `assert_ne!` is deterministic.

- [ ] **Step 7: Implement Flanker movement with one reused attack-band helper**

Extract the current Rifleman/Striker calculation into local:

```rust
fn choose_attack_band_destination(
    battle: &BattleState,
    id: UnitId,
    candidates: &[GridPos],
) -> Result<GridPos, BattleError>
```

Use it for Rifleman, Striker, and Flanker when the Flanker is neither a protected-target pressure unit nor the interception target.

For protect Flanker score `(band_distance_to_target, manhattan_to_target, Reverse(open_neighbors), y, x)`.

For designated Courier score `(manhattan_to_escape, Reverse(open_neighbors), y, x)`.

Artillery keeps the current branch. Do not introduce policy structs/traits.

- [ ] **Step 8: Make protected-target preference Flanker-only in `choose_target`**

For Flanker + protect rule, sort legal choices by whether the footprint misses the protected target before the existing threatened-count/player-priority keys. Keep committed footprints immutable after planning.

- [ ] **Step 9: Run regressions and commit**

```bash
cargo fmt --check
cargo test --lib domain::enemy::
cargo test --lib mission::mission_one::
cargo test --all-targets

git add src/domain/model.rs src/domain/enemy.rs src/mission/mod.rs src/mission/enemies.rs src/mission/mission_one.rs src/presentation/interaction.rs src/presentation/ui.rs src/presentation/battlefield.rs
git commit -m "feat: add authored enemy openings and flanker"
```

---

### Task 3: Author Mission 2 and prove the real three-round lifecycle

**Files:**
- Create: `src/mission/mission_two.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/domain/enemy.rs` tests for exact opening occupants

**Interfaces:**
- Produces: `mission_two(seed)`, `mission_two_for_campaign(seed, upgrades)`, `MISSION_TWO_DEFINITION`.
- Adds: `MissionId::Three` and `MissionId::number()` for One/Two/Three.
- Changes: `mission_definition(Two)` becomes authored; Three remains handoff until Task 4.

- [ ] **Step 1: Write Mission 2 authoring tests first**

Pin exact content:

```text
Players: Vanguard (3,7), Gunner (4,6), Interceptor (5,7)
Blocking: (3,3), (5,3), (2,6), (6,6)
Hazards: (1,5), (7,5)
Explosive: (6,4), HP 4
Rules: Protect Gunner through Round 3; >=50% HP bonus
Enemies: Rifleman 21, Striker 22, Artillery 23, Flanker 24
Rewards: 400 + 100; unlock Three
```

Assert a nonzero Gunner upgrade still projects through `build_player_squad` once.

- [ ] **Step 2: Implement exact Mission 2 roster/opening**

```rust
static MISSION_TWO_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(2,4), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4,5), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4,0), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::FLANKER, destination: GridPos::new(5,5), target: Some(ids::INTERCEPTOR) },
];
```

Starting positions: Rifleman `(2,2)`, Striker `(4,3)`, Artillery `(4,0)`, Flanker `(8,4)`.

Pin after `begin_round()`:

```text
Rifleman -> Vanguard
Striker -> Gunner
Artillery -> Gunner
Flanker -> Interceptor
```

- [ ] **Step 3: Add exact Mission 2 definition/VN copy**

Definition:

```text
Mission 2 — Hold Relay Nine
Protect Gunner through the end of Round 3.
Hold Fast: finish with Gunner at or above 50% HP.
400 base / 100 bonus / unlock Three
```

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

Reuse existing VN assets only.

- [ ] **Step 4: Add the required public round-lifecycle test**

Inside `mission_two.rs` tests, drive the actual battle machine; do not set `round`/`phase` for this test.

```rust
fn finish_living_players_with_guard(battle: &mut BattleState) {
    let players: Vec<_> = battle
        .units()
        .filter(|u| u.faction == Faction::Player && !u.is_knocked_out())
        .map(|u| u.id)
        .collect();
    for id in players {
        battle.begin_activation(id).unwrap();
        battle.choose_reaction(id, Reaction::Guard).unwrap();
        battle.finish_activation(id).unwrap();
    }
}

#[test]
fn mission_two_enemy_clear_still_requires_three_real_enemy_resolutions() {
    let mut battle = mission_two(7);
    battle.begin_round().unwrap();
    assert_eq!((battle.round(), battle.phase()), (1, BattlePhase::Player));

    for enemy in [ids::RIFLEMAN, ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
        battle.apply_direct_damage(enemy, 99, DamageSource::PlayerWeapon(squad::ids::PILE_LANCE));
    }
    assert_eq!(battle.result(), None);

    finish_living_players_with_guard(&mut battle);
    battle.resolve_enemy_phase().unwrap();
    assert_eq!((battle.round(), battle.phase()), (2, BattlePhase::Player));
    assert_eq!(battle.result(), None);

    finish_living_players_with_guard(&mut battle);
    battle.resolve_enemy_phase().unwrap();
    assert_eq!((battle.round(), battle.phase()), (3, BattlePhase::Player));
    assert_eq!(battle.result(), None);

    finish_living_players_with_guard(&mut battle);
    let events = battle.resolve_enemy_phase().unwrap();
    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionCompleted { .. })));
}
```

This proves the empty-intent round cycle already supports the protect mission; do not add waves or a wait command.

- [ ] **Step 5: Add protect failure/bonus boundary tests**

- Gunner KO → immediate defeat.
- Gunner exactly 50% HP at real Round-3 victory → bonus true.
- Gunner one HP below the half-HP threshold → victory with bonus false.
- terminal bonus event is immediately before `MissionCompleted`.

Focused round/phase fixture tests from Task 1 remain, but acceptance depends on the public lifecycle test above.

- [ ] **Step 6: Add MissionId Three + definition-driven Proceed routing**

During this task:

```rust
pub enum MissionId { One, Two, Three }
```

`mission_definition`: One/Two `Some`, Three `None`; add `number()`.

Continue: One→story, Two/Three→Upgrade. Upgrade `Proceed` checks whether `mission_definition(next_mission).is_some()`; authored IDs go to `PreMissionStory`, otherwise `NextMission`.

Make handoff copy derive `MISSION {number} UNLOCKED` instead of hard-coded Mission 2.

- [ ] **Step 7: Run and commit**

```bash
cargo fmt --check
cargo test --lib mission::mission_two::
cargo test --lib domain::battle::
cargo test --lib domain::enemy::
cargo test --all-targets

git add src/mission/mission_two.rs src/mission/mod.rs src/domain/battle.rs src/domain/enemy.rs src/presentation/campaign_ui.rs
git commit -m "feat: add mission 2 gunner defense"
```

---

### Task 4: Author Mission 3 and lock the Courier clock in lifecycle tests

**Files:**
- Create: `src/mission/mission_three.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/campaign/progression.rs` tests
- Modify: `tests/campaign_persistence.rs` for `MissionId::Four` round-trip

**Interfaces:**
- Produces: `mission_three(seed)`, `mission_three_for_campaign(seed, upgrades)`, `MISSION_THREE_DEFINITION`.
- Adds: `MissionId::Four`; One–Three authored, Four handoff.
- Locks: Courier `(0,6)` → extraction `(8,0)` → Round-4 deadline.
- Preserves: same `CampaignState`/save document shape.

- [ ] **Step 1: Write Mission 3 authoring tests first**

Pin:

```text
Players: Vanguard (4,7), Gunner (3,8), Interceptor (5,8)
Blocking: (4,3), (4,4), (4,5)
Hazard: (2,5)
Explosive: (6,3), HP 4
Courier: UnitId(31), Flanker, starts/stays (0,6)
Rifleman: UnitId(32), starts (3,2), opens (3,4) -> Vanguard
Striker: UnitId(33), starts (6,6), opens (5,7) -> Interceptor
Extraction: (8,0)
Deadline: 4
Bonus: victory by Round 2
Rewards: 500 + 150; unlock Four
```

Assert `GridPos::new(0,6).manhattan(GridPos::new(8,0)) == 14` to pin the authored clock assumption beside the mission data.

- [ ] **Step 2: Implement exact Mission 3 rules/content/VN**

```rust
MissionRules {
    primary: PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: GridPos::new(8, 0),
        deadline_round: 4,
    },
    optional: OptionalObjective::VictoryByRound { round: 2 },
    opening_plan: &MISSION_THREE_OPENING,
}
```

Definition:

```text
Mission 3 — Cut the Courier
Intercept Courier before extraction or the end of Round 4.
Swift Intercept: defeat Courier by the end of Round 2.
500 base / 150 bonus / unlock Four
```

Use the approved three pre-mission and two aftermath lines from the design spec; reuse existing VN assets only.

- [ ] **Step 3: Add focused interception outcome tests**

- KO Rifleman/Striker while Courier lives → no result.
- KO Courier while an escort lives → victory.
- Courier KO on Round 2 → bonus true; on Round 3 → bonus false.
- Test-only move Courier to `(8,0)`, then call public `begin_round()` while in EnemyPlanning → immediate extraction defeat.

- [ ] **Step 4: Add the required real Mission 3 round-clock lifecycle test**

Drive actual `begin_round`/`resolve_enemy_phase`. KO the two escorts after opening so the test isolates Courier geometry without deleting the objective target.

```rust
#[test]
fn mission_three_reaches_player_round_four_before_deadline() {
    let mut battle = mission_three(7);
    battle.begin_round().unwrap();
    assert_eq!((battle.round(), battle.phase()), (1, BattlePhase::Player));
    assert_eq!(battle.unit(ids::COURIER).unwrap().position, GridPos::new(0, 6));

    for escort in [ids::RIFLEMAN, ids::STRIKER] {
        battle.apply_direct_damage(escort, 99, DamageSource::PlayerWeapon(squad::ids::PILE_LANCE));
    }
    assert_eq!(battle.result(), None);

    for expected_round in [2, 3, 4] {
        finish_living_players_with_guard(&mut battle);
        battle.resolve_enemy_phase().unwrap();
        assert_eq!(battle.result(), None, "must still be playable at Round {expected_round}");
        assert_eq!((battle.round(), battle.phase()), (expected_round, BattlePhase::Player));
    }

    let before_deadline = battle.unit(ids::COURIER).unwrap().position;
    assert_ne!(before_deadline, GridPos::new(8, 0));
    assert_eq!(before_deadline.manhattan(GridPos::new(8, 0)), 2);

    finish_living_players_with_guard(&mut battle);
    battle.resolve_enemy_phase().unwrap();
    assert!(battle.result().is_some_and(|result| !result.victory));
    assert_eq!(
        battle.unit(ids::COURIER).unwrap().position,
        before_deadline,
        "deadline must fire before a fourth later-round Courier move"
    );
}
```

Use the same local `finish_living_players_with_guard` helper shape as Mission 2. This test is the acceptance lock for the review finding: Round 4 cannot be deferred to manual tuning.

- [ ] **Step 5: Extend MissionId/dispatch to Four**

Final shape:

```rust
pub enum MissionId { One, Two, Three, Four }
```

`mission_definition`: One/Two/Three `Some`, Four `None`; `number()` returns 1–4.

Continue routing: One→story, Two/Three→Upgrade, Four→NextMission. Task-3 definition-driven Proceed routes Three→story and Four→handoff automatically.

- [ ] **Step 6: Prove progression/save through Mission 3**

Campaign progression test: complete One, Two, Three with `optional_complete: false`; assert `next_mission == Four` and credits `300 + 400 + 500 == 1200`. Separate bonus test proves bonus credits do not alter unlocks.

Persist/reload `next_mission: Four` with nonzero credits/upgrades in `tests/campaign_persistence.rs`; assert exact equality. Add no schema version/migration.

- [ ] **Step 7: Run and commit**

```bash
cargo fmt --check
cargo test --lib mission::mission_three::
cargo test --lib campaign::progression::
cargo test --test campaign_persistence
cargo test --all-targets

git add src/mission/mission_three.rs src/mission/mod.rs src/presentation/campaign_ui.rs src/campaign/progression.rs tests/campaign_persistence.rs
git commit -m "feat: add mission 3 courier interception"
```

---

### Task 5: Make presentation objective-generic and cover every Flanker scale path

**Files:**
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/sync.rs`
- Modify: `src/presentation/interaction.rs` only if Mission-1-only debug names remain
- Modify: `tests/presentation_app.rs`

**Interfaces:**
- Changes: `result_overlay_copy(result, definition)`.
- Produces: rule-aware primary/bonus progress in `HudSnapshot`.
- Produces: `unit_scale(archetype)` as the single model-scale rule.
- Produces: Flanker child under-ring and static interception extraction ring using existing rendering assets.

- [ ] **Step 1: Write pure UI-copy tests**

Mission 2 HUD must contain `Round 1/3`, `Gunner HP`, and bonus state `On track`/`Missed`.

Mission 3 HUD must contain `Round 1/4`, Courier distance to extraction, and `Swift Intercept` state.

Pin generic result copy and change aftermath reward expectation from `Turnabout +100` to `Bonus +100`.

- [ ] **Step 2: Implement rule-aware objective progress and generic result/event copy**

- eliminate: `{remaining} enemies remaining`;
- protect: `Round {current}/{required} · {target} HP {hp}/{max}`;
- intercept: `Round {current}/{deadline} · {distance} cells from extraction`;
- Turnabout: Complete/Not yet;
- half-HP: On track/Missed;
- victory-by-round: Available/Missed;
- terminal: Achieved/Missed from `optional_complete`.

`OptionalObjectiveCompleted` playback becomes `BONUS OBJECTIVE COMPLETE`. Result overlay includes mission title + authored primary/bonus. Do not move copy into domain.

- [ ] **Step 3: Add one `unit_scale` helper and pin values**

In `battlefield.rs`:

```rust
pub const fn unit_scale(archetype: UnitArchetype) -> f32 {
    match archetype {
        UnitArchetype::Flanker => 0.62,
        _ => 0.72,
    }
}
```

Test Flanker scene index `2`, Flanker scale `0.62`, and a normal enemy scale `0.72`.

- [ ] **Step 4: Use `unit_scale` at initial unit spawn**

Replace the hard-coded initial `Vec3::splat(0.72)` in `populate_mission_root` with:

```rust
.with_scale(Vec3::splat(unit_scale(unit.archetype)))
```

This is required for first paint and restart rebuilds, because both go through `populate_mission_root`.

- [ ] **Step 5: Use `unit_scale` when spawning the Flanker child under-ring**

Because the ring is a child of the scaled unit, compensate the child local transform using the same helper:

```rust
let parent_scale = unit_scale(unit.archetype);
commands.spawn((
    Name::new("Flanker Marker"),
    Mesh3d(visual_assets.ring_mesh.clone()),
    MeshMaterial3d(visual_assets.telegraph_edge.clone()),
    Transform::from_xyz(0.0, -0.17 / parent_scale, 0.0)
        .with_scale(Vec3::splat(0.9 / parent_scale)),
    Pickable::IGNORE,
    ChildOf(unit_entity),
));
```

This keeps the world marker about `0.9` scale and `-0.17` Y offset while still following unit movement automatically.

- [ ] **Step 6: Use `unit_scale` in per-frame sync**

In `apply_unit_transforms`:

```rust
transform.scale = Vec3::splat(unit_scale(unit.archetype));
```

This is the third required scale-sensitive call site; do not leave the existing hard-coded `0.72`.

- [ ] **Step 7: Spawn extraction ring at the authored rule cell**

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

The Mission 3 rule supplies `(8,0)`; no new prop/domain type is introduced.

- [ ] **Step 8: Remove touched Mission-1-only debug names**

Use `Mission Presentation` for presentation/restart roots. Keep the existing `mission_one.gltf` filename/constants unchanged.

- [ ] **Step 9: Run and commit**

```bash
cargo fmt --check
cargo test --lib presentation::ui::
cargo test --test presentation_app
cargo test --all-targets

git add src/presentation/ui.rs src/presentation/campaign_ui.rs src/presentation/battlefield.rs src/presentation/sync.rs src/presentation/interaction.rs tests/presentation_app.rs
git commit -m "feat: present mission objectives and flanker pressure"
```

Only stage interaction if it changed.

---

### Task 6: Prove campaign entry/restart/save/upgrade routing through Mission 3

**Files:**
- Modify: `tests/presentation_app.rs`
- Modify: `tests/campaign_flow.rs`
- Modify: `tests/campaign_persistence.rs`
- Modify source only if these tests expose an actual mission-specific assumption

**Interfaces:**
- Consumes: existing `enter_battle`, `ActiveMission`, `definition.build`, `CampaignRuntime`, `persist_purchase`, restart seams.
- Verifies: no new app state/resource/save format is needed for Missions 2–3.

- [ ] **Step 1: Add renderer-free Mission 2 battle-entry coverage**

Start with `next_mission: Two` plus a Gunner HP upgrade; invoke existing battle entry; assert ActiveMission Two, protect rules, Round 1, and upgraded Gunner HP > 12.

- [ ] **Step 2: Add Mission 3 entry/restart coverage**

Enter with `next_mission: Three`, mutate battle, restart with fixed seed, assert ActiveMission remains Three and fresh rules are:

```rust
PrimaryObjective::InterceptBeforeEscape {
    target: mission_three::ids::COURIER,
    escape: GridPos::new(8, 0),
    deadline_round: 4,
}
```

Assert Courier HP 8. Use the existing restarted-round seam; do not special-case mission IDs in restart.

- [ ] **Step 3: Add campaign-action routing coverage for every saved ID**

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

Use existing temp-save/session helpers:

1. start new game;
2. complete M1 without bonus;
3. buy one affordable 200-credit upgrade;
4. complete M2 without bonus;
5. buy another affordable upgrade allowed by current credits/costs;
6. complete M3 without bonus;
7. load with a fresh session;
8. assert `next_mission == Four`, exact remaining credits, and exact purchased levels.

No bonus may be required to reach Mission 4 or buy a useful upgrade.

- [ ] **Step 5: Run integration/full tests and commit**

```bash
cargo fmt --check
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test presentation_app
cargo test --all-targets

git add tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs src/presentation/campaign_ui.rs src/app.rs src/presentation/interaction.rs
git commit -m "test: cover campaign progression through mission 3"
```

Only stage source files if a failing test proved a real hard-coded assumption.

---

### Task 7: Update docs, playtest authored tuning, and record final validation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-637.md`
- Modify: `src/mission/mission_two.rs` / `src/mission/mission_three.rs` only for playtest tuning that does not contradict the locked clock geometry

**Interfaces:**
- Produces current player/developer docs and reproducible HPA-637 validation evidence.
- Playtest may tune encounter feel; it may not move the Mission 3 geometry back to a path <=12 or defer Round-4 correctness to manual observation.

- [ ] **Step 1: Update README to the three-mission flow**

Document:

```text
Title -> M1 -> upgrades -> M2 -> upgrades -> M3 -> M4 unlocked handoff
```

Include M1 300+100, M2 400+100, M3 500+150, save/Continue behavior, objective summaries, Flanker characteristics, and unchanged controls/pilot skills. Remove the old “Mission 2 not in this build” text.

- [ ] **Step 2: Bring `CLAUDE.md` architecture current**

Record campaign plain-Rust boundary, MissionDefinition One–Three/Four handoff, closed MissionRules, shared `mission::enemies`, explicit Flanker planner branches/fallback, Gunner defense, Courier interception, committed-intent invariant, and HPA-637 docs.

- [ ] **Step 3: Run all automated gates before manual play**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Record exact outcomes/test counts in `docs/validation/hpa-637.md`.

- [ ] **Step 4: Manual Mission 2 validation**

Record competing opening locks, meaningful reaction/Aegis choices, enemy-clear not ending early, real Round-3 survival victory, Gunner KO failure, and bonus achieved/missed. If feel becomes empty-round stalling, tune authored M2 positions/stats only; do not add waves.

- [ ] **Step 5: Manual Mission 3 validation**

Record Courier fast silhouette/under-ring, extraction ring at `(8,0)`, Round 4 visibly reachable, objective-aware movement, locked telegraphs, Courier-only victory, escort-clear non-victory, exact extraction failure, Round-4 deadline failure, early bonus, and reuse of push/collision/hazard/explosive/reactions.

The automated Task-4 clock test must already be green; manual play is not allowed to redefine the deadline geometry.

- [ ] **Step 6: Manual save/continue/upgrade continuity**

Quit/relaunch after M1 and M2; Continue must reopen Upgrade with persisted credits/levels and Proceed must enter the next authored mission. After M3, relaunch/Continue must show `MISSION 4 UNLOCKED`.

- [ ] **Step 7: Write concrete `docs/validation/hpa-637.md`**

Include branch/head SHA, automated gate outcomes/test counts, M2 lifecycle evidence, M3 Round-4 lifecycle-test name plus manual observation, route/visual/result/deadline/bonus evidence, and save/upgrade continuity. No `TBD`, TODO, template marker, or unsupported checked claim.

- [ ] **Step 8: Re-run all gates after tuning/docs and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release

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
- [ ] Mission 2 enemy-clear does not win; Gunner KO fails; three real enemy resolutions produce victory.
- [ ] Mission 2 opening is Rifleman→Vanguard, Striker→Gunner, Artillery→Gunner, Flanker→Interceptor.
- [ ] Mission 3 start `(0,6)` / extraction `(8,0)` is a 14-step open path.
- [ ] Mission 3 lifecycle test reaches player Round 4 after three later Courier moves with result still `None`.
- [ ] Mission 3 Round-4 resolve triggers deadline before a fourth later Courier move.
- [ ] Exact extraction `(8,0)` still fails immediately.
- [ ] Courier KO wins with escorts alive; escort clear alone does not win.
- [ ] Non-objective Flanker uses normal attack-band movement instead of standing still.
- [ ] Flanker planner is deterministic and adds no RNG call.
- [ ] `unit_scale` is used by initial spawn, child marker compensation, and per-frame sync.
- [ ] Flanker uses existing scene 2 + 0.62 model scale + child under-ring; extraction uses existing white ring material.
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
- **Review corrections:** Mission 3 geometry is 14 steps and lifecycle-tested; non-objective Flanker has attack-band fallback; `unit_scale` covers all three scale-sensitive render paths.
- **Scope:** one bounded implementation PR; no independent subproject warrants another ticket/PR.
- **Type consistency:** `MissionRules`, `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `ObjectiveProgress::optional_complete`, and `MissionResult::optional_complete` use the same names throughout.
- **Mission consistency:** One unlocks Two, Two unlocks Three, Three unlocks Four; only One–Three are authored.
- **Reward consistency:** base 300/400/500 and bonuses 100/100/150; base-only completion reaches 1200 credits without grinding.