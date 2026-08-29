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
- Mission 3 Courier starts `(0,6)`, extraction is `(8,0)`, deadline is Round 4. The 14-step open path is locked because it guarantees player Round 4 exists after three move-4 later-round passes.
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
- `tests/campaign_flow.rs` — saved-ID/Proceed routing coverage.
- `tests/campaign_persistence.rs` — MissionId Four save round-trip/continuity coverage.
- `README.md` — current three-mission player-facing behavior.
- `CLAUDE.md` — current architecture/rules of record.

Expected untouched unless a failing integration test proves otherwise: `src/campaign/model.rs` shape, `src/campaign/save.rs` implementation, `src/campaign/session.rs`, `src/app.rs`, assets, `Cargo.toml`, and `Cargo.lock`.

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

Inside `src/domain/battle.rs` tests, add `objective_fixture(rules)` with player IDs 1/2, escort enemy 8, objective enemy 9, and enough weapon data for `apply_direct_damage` tests. Pin the match arms:

```rust
#[test]
fn protect_rule_ignores_enemy_clear_and_fails_on_target_ko() {
    let mut battle = objective_fixture(PROTECT_RULE);
    for enemy in [UnitId(8), UnitId(9)] {
        battle.apply_direct_damage(enemy, 99, DamageSource::PlayerWeapon(WeaponId(1)));
    }
    assert_eq!(battle.result(), None);

    battle.apply_direct_damage(
        UnitId(2),
        99,
        DamageSource::EnemyWeapon(UnitId(9), WeaponId(9)),
    );
    assert!(battle.result().is_some_and(|result| !result.victory));
}

#[test]
fn protect_rule_requires_enemy_planning_round_boundary() {
    let mut battle = objective_fixture(PROTECT_RULE);
    battle.phase = BattlePhase::EnemyPlanning;
    battle.round = 2;
    assert!(battle.check_terminal_state().is_empty());
    battle.round = 3;
    let events = battle.check_terminal_state();
    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionCompleted { .. })));
}

#[test]
fn intercept_rule_wins_on_target_ko_not_escort_clear() {
    let mut battle = objective_fixture(INTERCEPT_RULE);
    battle.apply_direct_damage(UnitId(8), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None);
    battle.apply_direct_damage(UnitId(9), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert!(battle.result().is_some_and(|result| result.victory));
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

These isolate terminal logic only. Tasks 3/4 add real round-machine lifecycle tests.

- [ ] **Step 2: Run the focused module red**

```bash
cargo test --lib domain::battle:: -- --nocapture
```

Expected: compile failure for the missing rule/result types.

- [ ] **Step 3: Add the exact closed domain types**

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

- [ ] **Step 4: Store rules in `BattleState`**

Change construction to:

```rust
pub(crate) fn new(
    board: BoardState,
    units: impl IntoIterator<Item = UnitState>,
    weapons: impl IntoIterator<Item = WeaponSpec>,
    rules: MissionRules,
    seed: u64,
) -> Self
```

Add private `rules: MissionRules` and `pub const fn rules(&self) -> MissionRules`.

- [ ] **Step 5: Implement `primary_outcome` without fallback victory**

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
        PrimaryObjective::InterceptBeforeEscape { target, escape, deadline_round } => {
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

`check_terminal_state` seals only this outcome; protect/intercept do not get an eliminate-all fallback.

- [ ] **Step 6: Implement one generic bonus bit**

Keep Turnabout's damage-source predicates in `observe_damage_for_objectives`, gated by `OptionalObjective::Turnabout`. Add:

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

On terminal victory, emit a newly-earned `OptionalObjectiveCompleted` immediately before `MissionCompleted`. Defeat never newly grants a terminal-only bonus.

- [ ] **Step 7: Rename active Rust fields and campaign reward usage**

Replace active `turnabout_complete` reads/writes with `optional_complete`. `CampaignState::complete_mission` awards `definition.optional_reward` only when `result.optional_complete`. Historical HPA-632/HPA-635 docs stay unchanged.

- [ ] **Step 8: Run and commit**

```bash
cargo fmt --check
cargo test --lib domain::battle::
cargo test --all-targets

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
- Produces: shared Rifleman/Striker/Artillery/Flanker constructors and four enemy weapon specs.
- Adds: `UnitArchetype::Flanker`.
- Consumes: `BattleState::rules().opening_plan` for round-0 movement/intents.
- Produces: local `choose_attack_band_destination(...)` reused by Rifleman/Striker and non-objective Flanker fallback.
- Preserves: exact Mission 1 round-0 positions/targets/intent order.

- [ ] **Step 1: Strengthen Mission 1 opening characterization and run it green**

Add:

```rust
assert_eq!(battle.intent_for(ids::RIFLEMAN_LEFT).unwrap().intended_occupant, Some(ids::GUNNER));
assert_eq!(battle.intent_for(ids::RIFLEMAN_RIGHT).unwrap().intended_occupant, Some(ids::INTERCEPTOR));
assert_eq!(battle.intent_for(ids::STRIKER).unwrap().intended_occupant, Some(ids::VANGUARD));
assert_eq!(battle.intent_for(ids::ARTILLERY).unwrap().intended_occupant, Some(ids::VANGUARD));
```

```bash
cargo test --lib domain::enemy::tests::authored_opening_places_four_locked_threats -- --exact
```

- [ ] **Step 2: Create `mission::enemies` with exact fixed values**

```text
Rifleman:  HP9 Armor1 Move2 Acc72 Eva5  -> Service Rifle
Striker:   HP12 Armor2 Move2 Acc78 Eva10 -> Shock Claw
Artillery: HP10 Armor1 Move1 Acc90 Eva0  -> Siege Mortar
Flanker:   HP8 Armor0 Move4 Acc82 Eva30   -> Skirmish Carbine
```

Skirmish Carbine is `range 1–2`, Single, damage 4, hit +5, crit 10, EN 0, no push, no counter. Existing three enemy weapon values remain bit-identical.

- [ ] **Step 3: Move Mission 1 opening into `MissionRules`**

```rust
static MISSION_ONE_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN_LEFT, destination: GridPos::new(2,5), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::RIFLEMAN_RIGHT, destination: GridPos::new(6,5), target: Some(ids::INTERCEPTOR) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4,6), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4,0), target: Some(ids::VANGUARD) },
];
```

Mission 1 rules are eliminate-all + Turnabout + this opening.

- [ ] **Step 4: Delete the Mission-1-specific opening matches from `enemy.rs`**

Round-0 movement looks up `EnemyOpening` by enemy ID; forced target resolves the authored target's current living position. Delete the old archetype/x-position opening match and `opening_target()` helper.

- [ ] **Step 5: Add Flanker exhaustive matches**

Flanker is enemy-only, rejected by pilot paths, maps to glTF scene index 2, and uses the existing default initiative value. Task 5 adds its visual scale/rings.

- [ ] **Step 6: Write concrete Flanker planner tests**

```rust
#[test]
fn protect_flanker_moves_into_band_of_protected_target() {
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
fn protect_flanker_prefers_protected_target_when_legal() {
    let battle = protect_flanker_attack_fixture();
    let intent = build_intent(&battle, UnitId(24), None).unwrap();
    assert_eq!(intent.intended_occupant, Some(UnitId(2)));
}

#[test]
fn courier_flanker_reduces_distance_to_escape() {
    let battle = intercept_flanker_fixture();
    let origin = battle.unit(UnitId(31)).unwrap().position;
    let destination = choose_enemy_destination(&battle, UnitId(31)).unwrap();
    let escape = GridPos::new(8, 0);
    assert!(destination.manhattan(escape) < origin.manhattan(escape));
}

#[test]
fn non_objective_flanker_uses_attack_band_fallback() {
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

Fixtures must be open deterministic boards where the asserted legal destination exists. Add a separate tie fixture with two equal-distance choices and assert the one with more open orthogonal neighbors is selected.

- [ ] **Step 7: Implement Flanker movement with one reused attack-band helper**

Extract current Rifleman/Striker logic to:

```rust
fn choose_attack_band_destination(
    battle: &BattleState,
    id: UnitId,
    candidates: &[GridPos],
) -> Result<GridPos, BattleError>
```

Use it for Rifleman, Striker, and a Flanker that is neither the protected-target pressure unit nor the interception target.

Protect Flanker sort key: `(band_distance_to_target, manhattan_to_target, Reverse(open_neighbors), y, x)`.

Courier sort key: `(manhattan_to_escape, Reverse(open_neighbors), y, x)`.

Artillery stays unchanged. Do not introduce behavior/policy objects.

- [ ] **Step 8: Add protected-target attack preference without changing committed intents**

For Flanker + protect rule, sort legal attack choices by `misses_protected_target` before the existing threatened-count/player-priority keys. Once committed, footprint/target remain locked through the player phase.

- [ ] **Step 9: Run and commit**

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

- [ ] **Step 1: Write Mission 2 authoring tests**

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

Also construct with a nonzero Gunner HP upgrade and assert projected max HP is above 12.

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

After `begin_round()`, assert intended occupants exactly Vanguard/Gunner/Gunner/Interceptor.

- [ ] **Step 3: Add exact Mission 2 definition/VN copy**

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
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

- [ ] **Step 4: Add the required public Mission 2 lifecycle test**

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
fn enemy_clear_still_requires_three_real_enemy_resolutions() {
    let mut battle = mission_two(7);
    battle.begin_round().unwrap();
    assert_eq!((battle.round(), battle.phase()), (1, BattlePhase::Player));

    for enemy in [ids::RIFLEMAN, ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
        battle.apply_direct_damage(enemy, 99, DamageSource::PlayerWeapon(squad::ids::PILE_LANCE));
    }
    assert_eq!(battle.result(), None);

    for expected_round in [2, 3] {
        finish_living_players_with_guard(&mut battle);
        battle.resolve_enemy_phase().unwrap();
        assert_eq!((battle.round(), battle.phase()), (expected_round, BattlePhase::Player));
        assert_eq!(battle.result(), None);
    }

    finish_living_players_with_guard(&mut battle);
    let events = battle.resolve_enemy_phase().unwrap();
    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionCompleted { .. })));
}
```

This proves empty-intent cycling already supplies the required clock; do not add waves or a wait command.

- [ ] **Step 5: Add protect failure and bonus boundaries**

Use real Mission 2 state to assert Gunner KO fails immediately. For victory bonus, drive the same three-round helper with Gunner set to exactly the integer half-HP threshold (`(max_hp + 1) / 2`) and assert `optional_complete`; repeat at one HP lower and assert victory with `optional_complete == false`. Assert any newly-earned `OptionalObjectiveCompleted` immediately precedes `MissionCompleted`.

- [ ] **Step 6: Add MissionId Three + definition-driven Proceed routing**

During this task `MissionId` is One/Two/Three. `mission_definition`: One/Two `Some`, Three `None`; add `number()`.

Continue: One→story, Two/Three→Upgrade. Upgrade `Proceed` checks `mission_definition(next_mission).is_some()`; authored IDs go to `PreMissionStory`, otherwise `NextMission`. Handoff copy becomes `MISSION {number} UNLOCKED`.

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
- Modify: `tests/campaign_persistence.rs`

**Interfaces:**
- Produces: `mission_three(seed)`, `mission_three_for_campaign(seed, upgrades)`, `MISSION_THREE_DEFINITION`.
- Adds: `MissionId::Four`; One–Three authored, Four handoff.
- Locks: Courier `(0,6)` → extraction `(8,0)` → Round-4 deadline.
- Preserves: same `CampaignState`/save document shape.

- [ ] **Step 1: Write Mission 3 authoring tests**

Pin exact content:

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

Also assert:

```rust
assert_eq!(GridPos::new(0, 6).manhattan(GridPos::new(8, 0)), 14);
```

- [ ] **Step 2: Implement exact Mission 3 rules/definition/VN**

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
Primary: Intercept Courier before extraction or the end of Round 4.
Bonus: Swift Intercept: defeat Courier by the end of Round 2.
500 base / 150 bonus / unlock Four
```

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

Reuse existing VN assets only.

- [ ] **Step 3: Add focused interception outcome tests**

Write separate tests with exact assertions:

```rust
let mut battle = mission_three(7);
battle.apply_direct_damage(ids::RIFLEMAN, 99, DamageSource::PlayerWeapon(squad::ids::PILE_LANCE));
battle.apply_direct_damage(ids::STRIKER, 99, DamageSource::PlayerWeapon(squad::ids::PILE_LANCE));
assert_eq!(battle.result(), None);
```

Then a fresh battle where Courier is KO'd while Rifleman is alive must produce victory. For bonus, set test round to 2 before Courier KO and assert `optional_complete == true`; repeat at round 3 and assert victory with `optional_complete == false`.

Exact extraction uses the existing test-only position seam, then public `begin_round()`:

```rust
let mut battle = mission_three(7);
battle.move_unit_direct_for_test(ids::COURIER, GridPos::new(8, 0));
battle.begin_round().unwrap();
assert!(battle.result().is_some_and(|result| !result.victory));
```

- [ ] **Step 4: Add the required real Mission 3 round-clock lifecycle test**

KO only the escorts after opening so Courier geometry is isolated; Guard prevents the Courier's low-damage intent from accidentally ending the squad.

```rust
#[test]
fn player_round_four_exists_before_courier_deadline() {
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
        assert_eq!(battle.result(), None, "must be playable at Round {expected_round}");
        assert_eq!((battle.round(), battle.phase()), (expected_round, BattlePhase::Player));
    }

    let before_deadline = battle.unit(ids::COURIER).unwrap().position;
    assert_ne!(before_deadline, GridPos::new(8, 0));
    assert_eq!(before_deadline.manhattan(GridPos::new(8, 0)), 2);

    finish_living_players_with_guard(&mut battle);
    battle.resolve_enemy_phase().unwrap();
    assert!(battle.result().is_some_and(|result| !result.victory));
    assert_eq!(battle.unit(ids::COURIER).unwrap().position, before_deadline);
}
```

The unchanged position after the final resolve proves the deadline fires at the `round == 4` EnemyPlanning check before a fourth later-round Courier move.

- [ ] **Step 5: Extend MissionId/dispatch to Four**

Final enum: One/Two/Three/Four. `mission_definition` returns Some for One–Three and None for Four; `number()` returns 1–4. Continue routes One→story, Two/Three→Upgrade, Four→NextMission.

- [ ] **Step 6: Prove progression/save through Mission 3**

Complete One, Two, Three with `optional_complete: false`; assert `next_mission == Four` and credits `1200`. A separate completion with `optional_complete: true` must add only the authored optional reward and preserve the same unlock.

Persist/reload `next_mission: Four` with nonzero credits and one nonzero upgrade in `tests/campaign_persistence.rs`; assert exact equality. Add no schema version/migration.

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

- [ ] **Step 1: Write concrete UI-copy tests**

```rust
#[test]
fn mission_two_hud_reports_protect_progress() {
    let mut battle = mission_two(7);
    battle.begin_round().unwrap();
    let hud = HudSnapshot::from_battle(&battle, None, &MISSION_TWO_DEFINITION);
    assert!(hud.primary.contains("Protect Gunner through the end of Round 3."));
    assert!(hud.primary.contains("Round 1/3"));
    assert!(hud.primary.contains("Gunner HP"));
    assert!(hud.optional.contains("Hold Fast"));
    assert!(hud.optional.contains("On track"));
}

#[test]
fn mission_three_hud_reports_intercept_clock_and_distance() {
    let mut battle = mission_three(7);
    battle.begin_round().unwrap();
    let hud = HudSnapshot::from_battle(&battle, None, &MISSION_THREE_DEFINITION);
    assert!(hud.primary.contains("Round 1/4"));
    assert!(hud.primary.contains("cells from extraction"));
    assert!(hud.optional.contains("Swift Intercept"));
    assert!(hud.optional.contains("Available"));
}
```

Pin result overlay for a Mission 3 victory with missed bonus and change aftermath reward expectation from `Turnabout +100` to `Bonus +100`.

- [ ] **Step 2: Implement rule-aware objective progress and generic result/event copy**

Eliminate shows enemy count; protect shows round + target HP; intercept shows round + Manhattan distance. Bonus state is Turnabout Complete/Not yet, half-HP On track/Missed, or victory-by-round Available/Missed. Terminal state uses `optional_complete`. `OptionalObjectiveCompleted` playback becomes `BONUS OBJECTIVE COMPLETE`.

- [ ] **Step 3: Add one `unit_scale` helper and tests**

```rust
pub const fn unit_scale(archetype: UnitArchetype) -> f32 {
    match archetype {
        UnitArchetype::Flanker => 0.62,
        _ => 0.72,
    }
}

#[test]
fn flanker_reuses_interceptor_scene_but_has_smaller_scale() {
    assert_eq!(scene_index(UnitArchetype::Flanker), 2);
    assert_eq!(unit_scale(UnitArchetype::Flanker), 0.62);
    assert_eq!(unit_scale(UnitArchetype::Rifleman), 0.72);
}
```

- [ ] **Step 4: Use `unit_scale` at initial unit spawn**

Replace initial `Vec3::splat(0.72)` in `populate_mission_root` with `Vec3::splat(unit_scale(unit.archetype))`. This path covers first paint and restart rebuild.

- [ ] **Step 5: Use `unit_scale` for Flanker child-marker compensation**

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

- [ ] **Step 6: Use `unit_scale` in per-frame sync**

```rust
transform.scale = Vec3::splat(unit_scale(unit.archetype));
```

This is the third scale-sensitive call site; the old hard-coded `0.72` must not remain in unit transform sync.

- [ ] **Step 7: Spawn extraction ring from the authored rule**

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

Mission 3 supplies `(8,0)`. No new prop/domain type.

- [ ] **Step 8: Remove touched Mission-1-only debug root names**

Use `Mission Presentation` for presentation/restart roots. Keep the asset filename `mission_one.gltf` unchanged.

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
- Modify source only if these tests expose a concrete mission-specific assumption

**Interfaces:**
- Consumes: existing `enter_battle`, `ActiveMission`, `definition.build`, `CampaignRuntime`, `persist_purchase`, restart seams.
- Verifies: no new app state/resource/save format is needed for Missions 2–3.

- [ ] **Step 1: Add Mission 2 renderer-free entry coverage**

Create state with `next_mission: Two` and Gunner HP level 1, run existing battle-entry system, then assert:

```rust
assert_eq!(app.world().resource::<ActiveMission>().0.id, MissionId::Two);
let battle = &app.world().resource::<BattleRuntime>().0;
assert_eq!(
    battle.rules().primary,
    PrimaryObjective::ProtectThroughRound { target: squad::ids::GUNNER, round: 3 }
);
assert_eq!(battle.round(), 1);
assert_eq!(battle.unit(squad::ids::GUNNER).unwrap().stats.max_hp, 15);
```

- [ ] **Step 2: Add Mission 3 entry/restart coverage**

Enter with `next_mission: Three`, mutate battle, restart with fixed seed, assert ActiveMission remains Three, Courier HP is 8, and rules are:

```rust
PrimaryObjective::InterceptBeforeEscape {
    target: mission_three::ids::COURIER,
    escape: GridPos::new(8, 0),
    deadline_round: 4,
}
```

Use the existing restarted-round seam; do not special-case mission IDs inside restart.

- [ ] **Step 3: Add saved-ID and Proceed routing assertions**

In `tests/campaign_flow.rs`, pin:

```text
Continue One   -> PreMissionStory
Continue Two   -> Upgrade
Continue Three -> Upgrade
Continue Four  -> NextMission
Proceed Two    -> PreMissionStory
Proceed Three  -> PreMissionStory
Proceed Four   -> NextMission
```

Use the existing `pending(&NextState<GameScreen>)` helper and assert each exact destination.

- [ ] **Step 4: Add save-backed progression continuity**

Start new game, complete M1 without bonus, buy Vanguard HP level 1 (200), complete M2 without bonus, buy Gunner HP level 1 (200), complete M3 without bonus, reload a fresh session. Base credits are 300+400+500=1200; two level-1 purchases cost 400, so assert:

```rust
assert_eq!(resumed.state.as_ref().unwrap().next_mission, MissionId::Four);
assert_eq!(resumed.state.as_ref().unwrap().credits, 800);
assert_eq!(resumed.state.as_ref().unwrap().upgrades.vanguard.hp, 1);
assert_eq!(resumed.state.as_ref().unwrap().upgrades.gunner.hp, 1);
```

No bonus is required.

- [ ] **Step 5: Run and commit**

```bash
cargo fmt --check
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test presentation_app
cargo test --all-targets

git add tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs src/presentation/campaign_ui.rs src/app.rs src/presentation/interaction.rs
git commit -m "test: cover campaign progression through mission 3"
```

Only stage source files that actually changed.

---

### Task 7: Update docs, playtest authored tuning, and record final validation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-637.md`
- Modify: `src/mission/mission_two.rs` / `src/mission/mission_three.rs` only for playtest tuning that does not contradict the locked clock geometry

**Interfaces:**
- Produces current player/developer docs and reproducible HPA-637 validation evidence.
- Playtest may tune encounter feel; it may not move Mission 3 back to a path <=12 or defer Round-4 correctness to manual observation.

- [ ] **Step 1: Update README to the three-mission flow**

Document Title → M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff; M1 300+100, M2 400+100, M3 500+150; Continue semantics; objective summaries; Flanker characteristics; unchanged controls/pilot skills. Remove the old “Mission 2 not in this build” statement.

- [ ] **Step 2: Bring `CLAUDE.md` architecture current**

Record campaign plain-Rust boundary, MissionDefinition One–Three/Four handoff, closed MissionRules, shared `mission::enemies`, explicit Flanker planner branches/fallback, Gunner defense, Courier interception, committed-intent invariant, and HPA-637 docs.

- [ ] **Step 3: Run all automated gates before manual play**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Record exact command outcomes and test counts in `docs/validation/hpa-637.md`.

- [ ] **Step 4: Manual Mission 2 validation**

Record competing opening locks, meaningful reaction/Aegis choices, enemy-clear not ending early, real Round-3 survival victory, Gunner KO failure, and bonus achieved/missed. If feel becomes empty-round stalling, tune authored Mission 2 positions/stats only; do not add waves.

- [ ] **Step 5: Manual Mission 3 validation**

Record Courier fast silhouette/under-ring, extraction ring at `(8,0)`, player Round 4 visibly reachable, objective-aware movement, locked telegraphs, Courier-only victory, escort-clear non-victory, exact extraction failure, Round-4 deadline failure, early bonus, and reuse of existing combat/environment rules.

The automated Task-4 clock test must already be green; manual play cannot redefine deadline geometry.

- [ ] **Step 6: Manual save/continue/upgrade continuity**

Quit/relaunch after M1 and M2; Continue must reopen Upgrade with persisted credits/levels and Proceed must enter the next authored mission. After M3, relaunch/Continue must show `MISSION 4 UNLOCKED`.

- [ ] **Step 7: Write concrete `docs/validation/hpa-637.md`**

Include branch/head SHA, automated gate outcomes/test counts, M2 lifecycle test name/evidence, M3 Round-4 lifecycle test name/evidence plus manual observation, route/visual/result/deadline/bonus evidence, and save/upgrade continuity. No `TBD`, `TODO`, template marker, or unsupported checked claim.

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
- [ ] Non-objective Flanker uses attack-band movement instead of standing still.
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
- **Placeholder scan:** no `TBD`, `TODO`, stub test body, or deferred design choice remains in this implementation ledger.
- **Scope:** one bounded implementation PR; no independent subproject warrants another ticket/PR.
- **Type consistency:** `MissionRules`, `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `ObjectiveProgress::optional_complete`, and `MissionResult::optional_complete` use the same names throughout.
- **Mission consistency:** One unlocks Two, Two unlocks Three, Three unlocks Four; only One–Three are authored.
- **Reward consistency:** base 300/400/500 and bonuses 100/100/150; base-only completion reaches 1200 credits without grinding.