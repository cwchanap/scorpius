# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius from the completed Mission 1 campaign loop to three continuously playable missions, adding a Gunner-defense encounter, a real Courier chase with reachable extraction, and a distinct Flanker enemy without introducing a generic objective or AI framework.

**Architecture:** `BattleState` gains one closed `MissionRules` row covering only the objective/opening shapes Missions 1–3 use. `mission` remains the typed authoring layer with a small shared regular-enemy catalog and separate Mission 2/3 modules. Flanker stays explicit in the existing deterministic planner; the existing campaign/save/UI composition remains intact. The checked-in glTF is extended by one authored scene rather than adding runtime scale/marker compensation.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, existing headless Rust tests, existing checked-in `assets/models/mission_one.gltf` and VN PNGs.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Global Constraints

- One Linear ticket (`HPA-637`) = one implementation PR; continue implementation on this draft PR.
- Keep dependency direction `presentation -> mission -> domain`; `src/domain/` must not import Bevy or campaign/presentation types.
- Keep one application crate and `bevy = "0.19"`; add no dependency.
- Preserve committed-intent semantics: player movement never retargets an already committed enemy intent.
- Keep mission content typed in Rust; add no RON/JSON/scripting/content framework.
- Add no objective callback/trait/registry, behavior tree, utility-AI framework, pathfinding dependency, stealth, teleportation, new initiative system, status framework, new playable unit, deployment selection, mission select, branching, difficulty, boss, or new hazard type.
- Mission 2 protects the existing Gunner; do not add a neutral faction/objective-unit role.
- Mission 2 wins when Gunner survives through the real Round-3 enemy-resolution boundary **or** when all attackers are eliminated while Gunner is alive.
- Mission 3 Courier starts `(0,6)`, extraction is `(8,0)`, and the deadline is Round 5. Three move-4 passes cannot extract before player Round 4; the fourth can extract; Round 5 is only the blocked/stalled backstop.
- Optional objectives affect credits only and never gate progression.
- Extend the existing checked-in glTF with one Flanker scene; add no new asset file or generation pipeline.
- Keep save state to `next_mission`, credits, and upgrades; add no migration/version compatibility code.
- All automated tests stay headless.
- Final gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `cargo build --release`.

---

## File Structure

### New files

- `src/mission/enemies.rs` — fixed regular-enemy constructors and four weapon specs.
- `src/mission/mission_two.rs` — Mission 2 board, IDs, opening, rules, dialogue, rewards, authoring/lifecycle tests.
- `src/mission/mission_three.rs` — Mission 3 board, IDs, opening, rules, dialogue, rewards, extraction/deadline lifecycle tests.
- `docs/validation/hpa-637.md` — final automated/manual validation evidence.

### Modified files

- `src/domain/model.rs` — closed mission-rule types, generic bonus state/result, Flanker archetype.
- `src/domain/battle.rs` — store rules, one named round-boundary helper, protect/intercept terminal conditions.
- `src/domain/enemy.rs` — authored opening rows; Flanker protect/intercept/fallback movement; fixed archetype initiative values.
- `src/mission/mod.rs` — modules, MissionId One–Four, definition dispatch.
- `src/mission/mission_one.rs` — shared enemy catalog and authored Mission 1 rules/opening.
- `src/campaign/progression.rs` — generic bonus reward bit.
- `src/presentation/assets.rs` — load 11 glTF scenes.
- `src/presentation/battlefield.rs` — Flanker scene index 10, extraction ring, generic debug name.
- `src/presentation/interaction.rs` — Flanker exhaustive enemy match and generic debug root name if touched.
- `src/presentation/ui.rs` — rule-aware objective progress and generic result/event copy.
- `src/presentation/campaign_ui.rs` — continuous mission routing and generic reward/handoff copy.
- `assets/models/mission_one.gltf` — add Flanker scene 10, mesh/material/nodes.
- `tests/campaign_flow.rs` — saved-ID/Proceed routing and glTF authored-scene checks if convenient.
- `tests/campaign_persistence.rs` — MissionId Four round-trip/continuity coverage.
- `tests/presentation_app.rs` — renderer-free battle entry/restart/HUD integration through Mission 3.
- `README.md` — current three-mission player-facing behavior.
- `CLAUDE.md` — current architecture/rules of record.

Expected untouched unless a failing integration test proves otherwise: `src/presentation/sync.rs`, `src/campaign/model.rs` shape, `src/campaign/save.rs` implementation, `src/campaign/session.rs`, `src/app.rs`, VN assets, `Cargo.toml`, and `Cargo.lock`.

---

### Task 1: Add closed mission rules, generic bonus semantics, and one round-boundary helper

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
- Produces: private `BattleState::completed_enemy_round(&self, round: u16) -> bool`.
- Changes: `BattleState::new(board, units, weapons, rules, seed)`.
- Changes: `ObjectiveProgress { optional_complete: bool }`.
- Changes: `MissionResult { victory, optional_complete, rounds }`.
- Preserves: `BattleEvent::OptionalObjectiveCompleted`.

- [ ] **Step 1: Write focused terminal-rule tests before changing the model**

Inside `src/domain/battle.rs` tests, add `objective_fixture(rules)` with two player units, escort enemy `UnitId(8)`, objective enemy `UnitId(9)`, an open 9×9 board, and enough weapon data for `apply_direct_damage`.

Pin eliminate/protect/intercept semantics separately:

```rust
#[test]
fn protect_rule_wins_when_all_attackers_are_gone() {
    let mut battle = objective_fixture(PROTECT_RULE);
    battle.apply_direct_damage(UnitId(8), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None);
    let events = battle.apply_direct_damage(
        UnitId(9),
        99,
        DamageSource::PlayerWeapon(WeaponId(1)),
    );
    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionCompleted { .. })));
}

#[test]
fn protect_rule_fails_when_target_is_knocked_out() {
    let mut battle = objective_fixture(PROTECT_RULE);
    battle.apply_direct_damage(
        UnitId(2),
        99,
        DamageSource::EnemyWeapon(UnitId(9), WeaponId(9)),
    );
    assert!(battle.result().is_some_and(|result| !result.victory));
}

#[test]
fn intercept_rule_wins_on_target_ko_not_escort_clear() {
    let mut battle = objective_fixture(INTERCEPT_RULE);
    battle.apply_direct_damage(UnitId(8), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert_eq!(battle.result(), None);
    battle.apply_direct_damage(UnitId(9), 99, DamageSource::PlayerWeapon(WeaponId(1)));
    assert!(battle.result().is_some_and(|result| result.victory));
}
```

- [ ] **Step 2: Add a focused test for the round-boundary helper semantics**

After the types exist, the private helper must mean “this many enemy resolutions are complete,” not merely compare a number:

```rust
#[test]
fn completed_enemy_round_requires_enemy_planning_boundary() {
    let mut battle = objective_fixture(PROTECT_RULE);
    battle.round = 3;
    battle.phase = BattlePhase::Player;
    assert!(!battle.completed_enemy_round(3));

    battle.phase = BattlePhase::EnemyPlanning;
    assert!(battle.completed_enemy_round(3));
}
```

- [ ] **Step 3: Run the focused module red**

```bash
cargo test --lib domain::battle:: -- --nocapture
```

Expected: compile failure for the missing rule/result/helper types.

- [ ] **Step 4: Add the exact closed domain types**

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

- [ ] **Step 5: Store rules and add the named boundary helper**

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

Add private `rules: MissionRules`, public `rules()`, and:

```rust
/// In EnemyPlanning before begin_round increments, `round` equals the number
/// of complete player/enemy rounds already resolved.
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

- [ ] **Step 6: Implement `primary_outcome` without a hidden eliminate-all fallback**

```rust
fn primary_outcome(&self) -> Option<bool> {
    let any_living_player = self
        .units
        .values()
        .any(|unit| unit.faction == Faction::Player && !unit.is_knocked_out());
    if !any_living_player {
        return Some(false);
    }

    let any_living_enemy = self
        .units
        .values()
        .any(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out());

    match self.rules.primary {
        PrimaryObjective::EliminateAllEnemies => (!any_living_enemy).then_some(true),
        PrimaryObjective::ProtectThroughRound { target, round } => {
            let target = self.units.get(&target).expect("authored protected target must exist");
            if target.is_knocked_out() {
                Some(false)
            } else if !any_living_enemy || self.completed_enemy_round(round) {
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
            } else if target.position == escape || self.completed_enemy_round(deadline_round) {
                Some(false)
            } else {
                None
            }
        }
    }
}
```

The protect early-clear arm is explicit product behavior and appears in briefing/HUD copy; destroy-all is not a hidden requirement.

- [ ] **Step 7: Implement one generic bonus bit**

Keep Turnabout damage-source predicates in `observe_damage_for_objectives`, gated by `OptionalObjective::Turnabout`.

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

- [ ] **Step 8: Rename active Rust fields and campaign reward usage**

Replace active `turnabout_complete` reads/writes with `optional_complete`. `CampaignState::complete_mission` awards `definition.optional_reward` only when `result.optional_complete`. Historical HPA-632/HPA-635 docs remain unchanged.

- [ ] **Step 9: Run and commit**

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
- Modify: `src/presentation/battlefield.rs`

**Interfaces:**
- Produces: shared Rifleman/Striker/Artillery/Flanker constructors and four enemy weapon specs.
- Adds: `UnitArchetype::Flanker`.
- Consumes: `BattleState::rules().opening_plan` for round-0 movement/intents.
- Produces: local `choose_attack_band_destination(...)` reused by Rifleman/Striker and non-objective Flanker fallback.
- Changes existing initiative constants only; adds no initiative field/system.
- Preserves exact Mission 1 round-0 positions/targets/intent order.

- [ ] **Step 1: Strengthen Mission 1 opening characterization and run it green before refactor**

Add exact intended occupants:

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
Rifleman:  HP9  Armor1 Move2 Acc72 Eva5  -> Service Rifle
Striker:   HP12 Armor2 Move2 Acc78 Eva10 -> Shock Claw
Artillery: HP10 Armor1 Move1 Acc90 Eva0  -> Siege Mortar
Flanker:   HP8  Armor0 Move4 Acc82 Eva30 -> Skirmish Carbine
```

Skirmish Carbine is range 1–2, Single, damage 4, hit +5, crit 10, EN 0, no push, no counter. Existing three enemy weapon values remain bit-identical.

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

- [ ] **Step 4: Delete Mission-1-specific opening matches from `enemy.rs`**

Round-0 movement looks up `EnemyOpening` by enemy ID and uses `destination`; forced opening target resolves the authored target's current living position. Delete the old archetype/x-position opening match and `opening_target()` helper.

- [ ] **Step 5: Write concrete Flanker planner tests including fallback**

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
    assert!(destination.manhattan(GridPos::new(8, 0)) < origin.manhattan(GridPos::new(8, 0)));
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

Use open deterministic fixtures. Add a tie fixture with two equal-distance candidates and assert the candidate with more open orthogonal neighbors wins.

- [ ] **Step 6: Implement Flanker movement with one reused attack-band helper**

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

- [ ] **Step 7: Add protected-target attack preference without changing committed intents**

For Flanker + protect rule, sort legal attack choices by `misses_protected_target` before the existing threatened-count/player-priority keys. Once committed, footprint/target remain locked through the player phase.

- [ ] **Step 8: Remove the remaining positional initiative hack**

Change only the existing `initiative(unit)` match:

```rust
fn initiative(unit: &UnitState) -> i16 {
    match unit.archetype {
        UnitArchetype::Striker => 30,
        UnitArchetype::Flanker => 25,
        UnitArchetype::Rifleman => 20,
        UnitArchetype::Artillery => 10,
        _ => 0,
    }
}
```

Add a private unit test for the four values. Keep the existing Mission 1 intent-order characterization; equal Rifleman values still sort left then right by attacker ID.

- [ ] **Step 9: Run and commit**

```bash
cargo fmt --check
cargo test --lib domain::enemy::
cargo test --lib mission::mission_one::
cargo test --all-targets

git add src/domain/model.rs src/domain/enemy.rs src/mission/mod.rs src/mission/enemies.rs src/mission/mission_one.rs src/presentation/interaction.rs src/presentation/battlefield.rs
git commit -m "feat: add authored enemy openings and flanker"
```

---

### Task 3: Author Mission 2, validate its references, and add MissionId handoffs once

**Files:**
- Create: `src/mission/mission_two.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/domain/enemy.rs` tests for exact opening occupants

**Interfaces:**
- Produces: `mission_two(seed)`, `mission_two_for_campaign(seed, upgrades)`, `MISSION_TWO_DEFINITION`.
- Adds in one step: `MissionId::{Three, Four}` and `MissionId::number()`.
- Changes: `mission_definition(Two)` becomes authored; Three/Four remain handoffs until Task 4 authors Three.

- [ ] **Step 1: Write exact Mission 2 authoring/reference tests**

Pin board dimensions and content:

```rust
let battle = mission_two(7);
assert_eq!((battle.board().width(), battle.board().height()), (9, 9));
assert_eq!(battle.unit(squad::ids::VANGUARD).unwrap().position, GridPos::new(3, 7));
assert_eq!(battle.unit(squad::ids::GUNNER).unwrap().position, GridPos::new(4, 6));
assert_eq!(battle.unit(squad::ids::INTERCEPTOR).unwrap().position, GridPos::new(5, 7));
assert_eq!(
    battle.rules().primary,
    PrimaryObjective::ProtectThroughRound { target: squad::ids::GUNNER, round: 3 }
);
assert!(battle.unit(squad::ids::GUNNER).is_some());
```

Assert exact blocking `(3,3),(5,3),(2,6),(6,6)`, hazards `(1,5),(7,5)`, explosive `(6,4)` HP 4, and enemy IDs 21–24.

For every `MISSION_TWO_OPENING` row, assert `row.unit` exists/is Enemy, destination is in bounds/non-blocking, and every `Some(target)` exists/is Player.

Construct with Gunner HP level 1 and assert max HP `15` to prove upgrade projection.

- [ ] **Step 2: Implement exact Mission 2 roster/opening**

```rust
static MISSION_TWO_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(2,4), target: Some(squad::ids::VANGUARD) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4,5), target: Some(squad::ids::GUNNER) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4,0), target: Some(squad::ids::GUNNER) },
    EnemyOpening { unit: ids::FLANKER, destination: GridPos::new(5,5), target: Some(squad::ids::INTERCEPTOR) },
];
```

Starting positions: Rifleman `(2,2)`, Striker `(4,3)`, Artillery `(4,0)`, Flanker `(8,4)`.

After `begin_round()`, assert intended occupants exactly Vanguard/Gunner/Gunner/Interceptor.

- [ ] **Step 3: Add exact Mission 2 definition/VN copy**

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
400 base / 100 bonus / unlock Three
```

Pre-mission:

```text
Control: Counterattack incoming. Gunner is finishing the Relay Nine uplink; the upload needs three full rounds.
Vanguard: Then Gunner stays standing. We hold until the upload finishes — or wipe out everything that can interrupt it.
Control: New contact: a fast Flanker is cutting around the line. Expect it to chase the uplink carrier.
```

Aftermath:

```text
Vanguard: Uplink complete. Relay Nine can finally hand us the enemy route data.
Control: It found a courier breaking for extraction. Resupply now — we only get one chance to cut it off.
```

Reuse existing VN assets only.

- [ ] **Step 4: Add the immediate-clear lifecycle test**

```rust
#[test]
fn clearing_all_attackers_ends_the_defense_immediately() {
    let mut battle = mission_two(7);
    battle.begin_round().unwrap();
    assert_eq!((battle.round(), battle.phase()), (1, BattlePhase::Player));

    let mut final_events = Vec::new();
    for enemy in [ids::RIFLEMAN, ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
        final_events = battle.apply_direct_damage(
            enemy,
            99,
            DamageSource::PlayerWeapon(squad::ids::PILE_LANCE),
        );
    }

    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(final_events.iter().any(|event| matches!(event, BattleEvent::MissionCompleted { .. })));
}
```

- [ ] **Step 5: Add the real three-resolution survival lifecycle test with enemies alive**

Use a durable test-only campaign build so the test measures the round boundary rather than seeded damage variance:

```rust
fn defensive_test_upgrades() -> SquadUpgrades {
    SquadUpgrades {
        gunner: UpgradeLevels {
            hp: 3,
            armor: 3,
            ..Default::default()
        },
        ..Default::default()
    }
}

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
fn gunner_surviving_three_real_enemy_resolutions_wins_with_attackers_alive() {
    let mut battle = mission_two_for_campaign(7, &defensive_test_upgrades());
    battle.begin_round().unwrap();

    for expected_round in [2, 3] {
        finish_living_players_with_guard(&mut battle);
        battle.resolve_enemy_phase().unwrap();
        assert_eq!(battle.result(), None);
        assert_eq!((battle.round(), battle.phase()), (expected_round, BattlePhase::Player));
    }

    assert!(battle.units().any(|u| u.faction == Faction::Enemy && !u.is_knocked_out()));
    finish_living_players_with_guard(&mut battle);
    battle.resolve_enemy_phase().unwrap();
    assert!(battle.result().is_some_and(|result| result.victory));
}
```

- [ ] **Step 6: Add protect failure and bonus boundaries**

Use fresh Mission 2 state to assert Gunner KO fails immediately. For the bonus, use direct test HP adjustment before the terminal condition:

```rust
let half = (gunner.stats.max_hp + 1) / 2;
```

At `half`, victory sets `optional_complete`; at `half - 1`, victory remains valid but the bonus is false. Assert any newly-earned `OptionalObjectiveCompleted` immediately precedes `MissionCompleted`.

- [ ] **Step 7: Add MissionId Three/Four and definition-driven routing once**

Final enum shape is introduced now:

```rust
pub enum MissionId { One, Two, Three, Four }
```

`mission_definition`: One/Two `Some`, Three/Four `None` in this task. Add `number()`.

Continue routing is immediately final: One→story, Two/Three→Upgrade, Four→NextMission.

Upgrade `Proceed` checks `mission_definition(next_mission).is_some()`; authored IDs go to `PreMissionStory`, otherwise `NextMission`. Handoff copy becomes `MISSION {number} UNLOCKED`.

- [ ] **Step 8: Run and commit**

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

### Task 4: Author Mission 3 and prove extraction, deadline, and player-caused escape

**Files:**
- Create: `src/mission/mission_three.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/campaign/progression.rs` tests
- Modify: `tests/campaign_persistence.rs`

**Interfaces:**
- Produces: `mission_three(seed)`, `mission_three_for_campaign(seed, upgrades)`, `MISSION_THREE_DEFINITION`.
- Changes: `mission_definition(Three)` from handoff to authored; Four remains handoff.
- Locks: Courier `(0,6)` → extraction `(8,0)` → deadline Round 5.
- Preserves: same `CampaignState`/save document shape.

- [ ] **Step 1: Write exact Mission 3 authoring/reference tests**

```rust
let battle = mission_three(7);
assert_eq!((battle.board().width(), battle.board().height()), (9, 9));
assert_eq!(battle.unit(ids::COURIER).unwrap().archetype, UnitArchetype::Flanker);
assert_eq!(battle.unit(ids::COURIER).unwrap().position, GridPos::new(0, 6));
assert_eq!(
    battle.rules().primary,
    PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: GridPos::new(8, 0),
        deadline_round: 5,
    }
);
assert_eq!(GridPos::new(0, 6).manhattan(GridPos::new(8, 0)), 14);
```

Assert player deployment `(4,7)/(3,8)/(5,8)`, blocking `(4,3),(4,4),(4,5)`, hazard `(2,5)`, explosive `(6,3)` HP 4, and exactly Courier/Rifleman/Striker enemies.

Validate all opening references. For escape `(8,0)`, assert `board.contains`, `!board.is_blocking`, `!board.is_hazard`, and `!board.has_live_explosive`.

- [ ] **Step 2: Implement exact Mission 3 rules/definition/VN**

```rust
MissionRules {
    primary: PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: GridPos::new(8, 0),
        deadline_round: 5,
    },
    optional: OptionalObjective::VictoryByRound { round: 2 },
    opening_plan: &MISSION_THREE_OPENING,
}
```

Definition:

```text
Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by the end of Round 2.
500 base / 150 bonus / unlock Four
```

Pre-mission:

```text
Control: Courier identified. That Flanker has Relay Nine's route keys and is heading for extraction.
Vanguard: We cut across and stop it. Escorts are secondary — the Courier is the mission.
Control: Extraction is at the east marker. If it gets out, or Round 5 closes, the data is gone.
```

Aftermath:

```text
Vanguard: Courier down. The route keys are intact.
Control: Confirmed. They point to a larger force ahead. Spend the salvage and prepare for the next operation.
```

- [ ] **Step 3: Add focused interception outcome tests**

- KO Rifleman + Striker while Courier lives → `result() == None`.
- KO Courier while at least one escort lives → victory.
- Courier KO at round 2 → `optional_complete == true`.
- Courier KO at round 3 → victory with `optional_complete == false`.
- Test-only move Courier to `(8,0)`, call public `begin_round()` in EnemyPlanning → immediate extraction defeat.
- Set EnemyPlanning round 5 with living Courier not on escape → deadline defeat.

- [ ] **Step 4: Add a shared durable lifecycle helper**

For Mission 3 round-clock tests, use all three player mechs with max HP/armor upgrades so the tests isolate Courier geometry and objective timing:

```rust
fn durable_squad_upgrades() -> SquadUpgrades {
    let durable = UpgradeLevels { hp: 3, armor: 3, ..Default::default() };
    SquadUpgrades {
        vanguard: durable,
        gunner: durable,
        interceptor: durable,
    }
}
```

Reuse `finish_living_players_with_guard` from the Mission 2 test shape inside the Mission 3 test module.

- [ ] **Step 5: Prove player Round 4 exists before extraction**

```rust
#[test]
fn player_round_four_exists_before_courier_can_extract() {
    let mut battle = mission_three_for_campaign(7, &durable_squad_upgrades());
    battle.begin_round().unwrap();
    assert_eq!((battle.round(), battle.phase()), (1, BattlePhase::Player));

    for escort in [ids::RIFLEMAN, ids::STRIKER] {
        battle.apply_direct_damage(escort, 99, DamageSource::PlayerWeapon(squad::ids::PILE_LANCE));
    }

    for expected_round in [2, 3, 4] {
        finish_living_players_with_guard(&mut battle);
        battle.resolve_enemy_phase().unwrap();
        assert_eq!(battle.result(), None, "must remain playable at Round {expected_round}");
        assert_eq!((battle.round(), battle.phase()), (expected_round, BattlePhase::Player));
    }

    assert_ne!(battle.unit(ids::COURIER).unwrap().position, GridPos::new(8, 0));
}
```

Do not assert an exact intermediate remaining distance.

- [ ] **Step 6: Prove the open route extracts after player Round 4**

Continue from a fresh equivalent fixture through player Round 4, then:

```rust
finish_living_players_with_guard(&mut battle);
let events = battle.resolve_enemy_phase().unwrap();
assert!(battle.result().is_some_and(|result| !result.victory));
assert_eq!(battle.unit(ids::COURIER).unwrap().position, GridPos::new(8, 0));
assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionFailed { .. })));
```

This is the headline chase contract: normal path failure is extraction, not the deadline.

- [ ] **Step 7: Prove blocked extraction reaches the Round-5 deadline backstop**

After opening, use the existing direct test position seam to place a durable living Interceptor on `(8,0)` so Courier cannot occupy the exit. Remove escorts and drive through Round 4.

After resolving player Round 4:

```rust
assert_eq!((battle.round(), battle.phase()), (5, BattlePhase::Player));
assert_eq!(battle.result(), None);
assert_ne!(battle.unit(ids::COURIER).unwrap().position, GridPos::new(8, 0));
```

Capture Courier position, finish player Round 5, resolve, then assert deadline defeat and unchanged Courier position. This proves the deadline fires before another later move.

- [ ] **Step 8: Add the player-caused push-into-extraction regression**

`resolve_push` already calls terminal evaluation. Test the accidental/self-inflicted loss explicitly:

```rust
#[test]
fn pushing_courier_onto_extraction_fails_immediately() {
    let mut battle = mission_three(7);
    battle.move_unit_direct_for_test(squad::ids::VANGUARD, GridPos::new(6, 0));
    battle.move_unit_direct_for_test(ids::COURIER, GridPos::new(7, 0));

    let events = battle
        .resolve_push(squad::ids::VANGUARD, ids::COURIER)
        .unwrap();

    assert_eq!(battle.unit(ids::COURIER).unwrap().position, GridPos::new(8, 0));
    assert!(battle.result().is_some_and(|result| !result.victory));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::UnitPushed { unit, .. } if *unit == ids::COURIER)));
    assert!(events.iter().any(|event| matches!(event, BattleEvent::MissionFailed { .. })));
}
```

- [ ] **Step 9: Make Mission 3 authored and prove progression/save**

`mission_definition(Three)` becomes `Some(&MISSION_THREE_DEFINITION)`; Four remains `None`. No enum sweep is needed because Task 3 already added One–Four.

Complete One, Two, Three with `optional_complete: false`; assert `next_mission == Four` and credits `1200`. A separate optional-complete test adds only the authored bonus and preserves unlock behavior.

Persist/reload `next_mission: Four` with nonzero credits and one nonzero upgrade in `tests/campaign_persistence.rs`; assert exact equality. Add no schema version/migration.

- [ ] **Step 10: Run and commit**

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

### Task 5: Add a real Flanker glTF scene and make presentation objective-generic

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/presentation/interaction.rs` only if Mission-1-only debug names remain
- Modify: `tests/presentation_app.rs`
- Modify: `tests/campaign_flow.rs` for glTF JSON integrity if that suite remains the asset-content test home

**Interfaces:**
- Changes: `MISSION_ONE_SCENE_COUNT` from 10 to 11.
- Changes: `scene_index(UnitArchetype::Flanker) = 10`.
- Changes: `result_overlay_copy(result, definition)`.
- Produces: rule-aware primary/bonus progress in `HudSnapshot`.
- Produces: extraction ring using existing runtime mesh/material.
- Does **not** produce `unit_scale` or Flanker child-marker components.

- [ ] **Step 1: Write the glTF authoring test before editing the asset**

Use existing `serde_json` to parse the checked-in file headlessly:

```rust
#[test]
fn checked_in_gltf_contains_distinct_flanker_scene() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/models/mission_one.gltf");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();

    let scenes = value["scenes"].as_array().unwrap();
    let meshes = value["meshes"].as_array().unwrap();
    let materials = value["materials"].as_array().unwrap();
    assert_eq!(scenes.len(), 11);
    assert_eq!(scenes[10]["name"], "Flanker");
    assert_eq!(scenes[10]["nodes"], serde_json::json!([49, 50, 51, 52, 53, 54, 55]));
    assert_eq!(meshes[10]["name"], "Flanker Magenta");
    assert_eq!(materials[10]["name"], "Flanker Magenta");
}
```

- [ ] **Step 2: Append scene 10 and nodes 49–55 to the checked-in glTF**

Scene:

```json
{
  "name": "Flanker",
  "nodes": [49, 50, 51, 52, 53, 54, 55]
}
```

All seven nodes use mesh `10`:

```text
49 Left Leg      translation [-0.16, 0.18,  0.00] scale [0.12, 0.36, 0.16]
50 Right Leg     translation [ 0.16, 0.18,  0.00] scale [0.12, 0.36, 0.16]
51 Torso         translation [ 0.00, 0.62,  0.00] scale [0.36, 0.42, 0.28]
52 Head          translation [ 0.00, 0.95,  0.00] scale [0.20, 0.20, 0.20]
53 Left Fin      translation [-0.42, 0.67, -0.10] scale [0.42, 0.08, 0.28]
54 Right Fin     translation [ 0.42, 0.67, -0.10] scale [0.42, 0.08, 0.28]
55 Rear Thruster translation [ 0.00, 0.52, -0.34] scale [0.20, 0.16, 0.34]
```

- [ ] **Step 3: Add mesh/material 10 using the existing shared cuboid accessors**

Mesh:

```json
{
  "name": "Flanker Magenta",
  "primitives": [{
    "attributes": { "POSITION": 0, "NORMAL": 1 },
    "indices": 2,
    "material": 10
  }]
}
```

Material:

```json
{
  "name": "Flanker Magenta",
  "pbrMetallicRoughness": {
    "baseColorFactor": [0.78, 0.08, 0.46, 1.0],
    "metallicFactor": 0.25,
    "roughnessFactor": 0.62
  },
  "emissiveFactor": [0.08, 0.0, 0.04]
}
```

Do not change the base64 buffer/accessors.

- [ ] **Step 4: Load/map the new scene without scale special cases**

`src/presentation/assets.rs`:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 11;
```

`battlefield.rs`:

```rust
UnitArchetype::Flanker => 10,
```

Keep existing `Vec3::splat(0.72)` at unit spawn and sync; there is no `unit_scale`, no Flanker child under-ring, and no inverse-scale math.

Add:

```rust
#[test]
fn flanker_uses_distinct_authored_scene() {
    assert_eq!(scene_index(UnitArchetype::Flanker), 10);
    assert_eq!(MISSION_ONE_SCENE_COUNT, 11);
}
```

- [ ] **Step 5: Write concrete objective-generic UI-copy tests**

Mission 2:

```rust
let mut battle = mission_two(7);
battle.begin_round().unwrap();
let hud = HudSnapshot::from_battle(&battle, None, &MISSION_TWO_DEFINITION);
assert!(hud.primary.contains("Protect Gunner through the end of Round 3, or eliminate all attackers."));
assert!(hud.primary.contains("Round 1/3"));
assert!(hud.primary.contains("Gunner HP"));
assert!(hud.optional.contains("Hold Fast"));
```

Mission 3:

```rust
let mut battle = mission_three(7);
battle.begin_round().unwrap();
let hud = HudSnapshot::from_battle(&battle, None, &MISSION_THREE_DEFINITION);
assert!(hud.primary.contains("Round 1/5"));
assert!(hud.primary.contains("cells from extraction"));
assert!(hud.optional.contains("Swift Intercept"));
```

Pin result overlay for a Mission 3 victory with missed bonus and change aftermath reward expectation from `Turnabout +100` to `Bonus +100`.

- [ ] **Step 6: Implement rule-aware HUD/result/event copy**

Eliminate shows enemy count; protect shows round + target HP; intercept shows round + Manhattan distance. Bonus state is Turnabout Complete/Not yet, half-HP On track/Missed, or victory-by-round Available/Missed. Terminal state uses `optional_complete`.

`OptionalObjectiveCompleted` playback becomes `BONUS OBJECTIVE COMPLETE`. Result overlay includes mission title + authored primary/bonus.

- [ ] **Step 7: Spawn extraction ring from the authored Mission 3 rule**

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

Mission 3 supplies `(8,0)`. No new prop/domain type.

- [ ] **Step 8: Remove touched Mission-1-only debug root names**

Use `Mission Presentation` for presentation/restart roots. Keep the asset filename `mission_one.gltf` unchanged.

- [ ] **Step 9: Run and commit**

```bash
cargo fmt --check
cargo test --lib presentation::ui::
cargo test --test campaign_flow
cargo test --test presentation_app
cargo test --all-targets

git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/campaign_ui.rs tests/campaign_flow.rs tests/presentation_app.rs
git commit -m "feat: add flanker presentation and mission objective UI"
```

Stage `src/presentation/interaction.rs` separately only if it actually changed.

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
    deadline_round: 5,
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

- [ ] **Step 5: Run and commit only the files actually changed**

```bash
cargo fmt --check
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test presentation_app
cargo test --all-targets

git add tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs
git commit -m "test: cover campaign progression through mission 3"
```

If a test forces a small source fix, stage that exact source file in a separate `git add <path>` before the commit; do not pre-stage `src/app.rs` or `src/presentation/interaction.rs` speculatively.

---

### Task 7: Update docs, playtest encounter feel, and record final validation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-637.md`
- Modify: Mission 2/3 authored content only if playtesting exposes a feel/balance issue that does not contradict the locked objective semantics

**Interfaces:**
- Produces current player/developer docs and reproducible HPA-637 validation evidence.
- Playtest may tune encounter feel; it may not redefine Mission 2 early-clear behavior or Mission 3 extraction/deadline timing.

- [ ] **Step 1: Update README to the three-mission flow**

Document Title → M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff; M1 300+100, M2 400+100, M3 500+150; Continue semantics; objective summaries; Flanker characteristics; unchanged controls/pilot skills.

Mission 2 copy must state that clearing all attackers ends the defense immediately. Mission 3 copy must state extraction/`Round 5` rather than Round 4.

- [ ] **Step 2: Bring `CLAUDE.md` architecture current**

Record campaign plain-Rust boundary, MissionDefinition One–Three/Four handoff, closed MissionRules, `completed_enemy_round`, shared `mission::enemies`, explicit Flanker planner branches/fallback, fixed archetype initiative order, Gunner defense, Courier interception, glTF scene 10, committed-intent invariant, and HPA-637 docs.

- [ ] **Step 3: Run all automated gates before manual play**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Record exact command outcomes and test counts in `docs/validation/hpa-637.md`.

- [ ] **Step 4: Manual Mission 2 validation**

Record competing opening locks, meaningful reaction/Aegis choices, Gunner KO failure, one run that clears all attackers and wins immediately, one run that reaches the full Round-3 survival victory with attackers alive, and bonus achieved/missed.

If feel/balance needs tuning, adjust authored Mission 2 positions/stats only; do not add waves or a wait command.

- [ ] **Step 5: Manual Mission 3 validation**

Record distinct magenta Flanker/Courier scene, extraction ring at `(8,0)`, player Round 4 visibly exists, objective-aware movement, locked telegraphs, Courier-only victory, escort-clear non-victory, open-route extraction after Round 4, blocked-exit Round-5 deadline, early bonus, and reuse of existing combat/environment rules.

Also manually confirm that pushing/displacing Courier toward extraction can be dangerous; automated coverage already pins push-to-extraction failure.

- [ ] **Step 6: Manual save/continue/upgrade continuity**

Quit/relaunch after M1 and M2; Continue must reopen Upgrade with persisted credits/levels and Proceed must enter the next authored mission. After M3, relaunch/Continue must show `MISSION 4 UNLOCKED`.

- [ ] **Step 7: Write concrete `docs/validation/hpa-637.md`**

Include branch/head SHA, automated gate outcomes/test counts, Mission 2 early-clear + Round-3 lifecycle test names/evidence, Mission 3 Round-4/extraction/blocked-deadline/push test names/evidence plus manual observations, Flanker glTF scene evidence, save/upgrade continuity, and short-session verdict. No `TBD`, `TODO`, template marker, or unsupported checked claim.

- [ ] **Step 8: Re-run all gates after tuning/docs and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release

git add README.md CLAUDE.md docs/validation/hpa-637.md
git commit -m "docs: validate HPA-637 missions 2 and 3"
```

If manual tuning changed a mission source file, stage that exact file separately.

---

## Final PR Gate

- [ ] One PR contains only HPA-637 scope.
- [ ] No new dependency or runtime asset-generation pipeline.
- [ ] No objective framework, neutral objective role, or behavior-tree/policy framework.
- [ ] Mission 1 exact opening regression remains green.
- [ ] `completed_enemy_round` is the only round-boundary predicate used by protect/deadline objective arms.
- [ ] Mission 2 Gunner KO fails.
- [ ] Mission 2 clearing all attackers wins immediately with Gunner alive.
- [ ] Mission 2 also wins after three real enemy resolutions when attackers remain.
- [ ] Mission 2 opening is Rifleman→Vanguard, Striker→Gunner, Artillery→Gunner, Flanker→Interceptor.
- [ ] Mission 3 start `(0,6)` / extraction `(8,0)` is a 14-step open path.
- [ ] Mission 3 lifecycle reaches player Round 4 after three later Courier moves with result still `None`.
- [ ] Open-route Mission 3 extraction occurs on the fourth later move after player Round 4.
- [ ] Blocked extraction reaches player Round 5, then the deadline fires before another Courier move.
- [ ] Pushing Courier onto `(8,0)` fails immediately.
- [ ] Courier KO wins with escorts alive; escort clear alone does not win.
- [ ] Non-objective Flanker uses attack-band movement instead of standing still.
- [ ] Initiative order is Striker 30, Flanker 25, Rifleman 20, Artillery 10; no positional Rifleman hack remains.
- [ ] Mission 2/3 authoring tests validate rule targets/opening references; Mission 3 validates legal extraction.
- [ ] Flanker uses checked-in glTF scene 10 and `MISSION_ONE_SCENE_COUNT == 11`.
- [ ] No `unit_scale`, Flanker child under-ring, or inverse-scale compensation was added.
- [ ] Mission 3 extraction uses the existing white ring material.
- [ ] Briefing, HUD, and results show primary + bonus objective for Missions 2/3; Mission 3 HUD is Round n/5.
- [ ] Bonuses alter credits only.
- [ ] M1 → M2 → M3 → M4 handoff works with save/Continue/Upgrade.
- [ ] Base-only completion yields 1200 total credits through Mission 3.
- [ ] README, CLAUDE.md, and `docs/validation/hpa-637.md` describe the final state.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --all-targets` passes.
- [ ] `cargo build --release` passes.

## Self-review

- **Spec coverage:** every HPA-637 acceptance item and every accepted review correction maps to Tasks 1–7 and the Final PR Gate.
- **Clock consistency:** Mission 3 deadline is 5 everywhere; Round 4 is guaranteed before extraction, open-route extraction is live on move 4, and Round 5 is the blocked/stalled backstop.
- **Protect consistency:** enemy clear is an explicit early victory path in rules, briefing copy, tests, and manual validation.
- **Asset consistency:** Flanker is scene 10 in the existing glTF; no runtime scale/under-ring workaround remains.
- **Initiative consistency:** the positional Rifleman hack is removed without adding a new initiative field/system.
- **Authoring consistency:** both mission modules pin 9×9 dimensions and authored references; Mission 3 pins legal extraction.
- **Placeholder scan:** no `TBD`, `TODO`, stub test body, or deferred design decision remains in this plan.
- **Scope:** one bounded implementation PR; no independent subproject warrants another ticket/PR.
- **Type consistency:** `MissionRules`, `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `ObjectiveProgress::optional_complete`, and `MissionResult::optional_complete` use the same names throughout.
- **Mission consistency:** One unlocks Two, Two unlocks Three, Three unlocks Four; only One–Three are authored.
- **Reward consistency:** base 300/400/500 and bonuses 100/100/150; base-only completion reaches 1200 credits without grinding.