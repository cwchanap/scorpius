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

```rust
/// In EnemyPlanning before begin_round increments, `round` equals the number
/// of complete player/enemy rounds already resolved.
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

Change `BattleState::new` to accept/store `MissionRules`; add `pub const fn rules(&self) -> MissionRules`.

- [ ] **Step 6: Implement `primary_outcome` with explicit protect early-clear behavior**

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

On terminal victory, emit a newly-earned `OptionalObjectiveCompleted` immediately before `MissionCompleted`.

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

Add exact intended occupants and keep the current intent-order assertion:

```rust
assert_eq!(battle.intent_for(ids::RIFLEMAN_LEFT).unwrap().intended_occupant, Some(ids::GUNNER));
assert_eq!(battle.intent_for(ids::RIFLEMAN_RIGHT).unwrap().intended_occupant, Some(ids::INTERCEPTOR));
assert_eq!(battle.intent_for(ids::STRIKER).unwrap().intended_occupant, Some(ids::VANGUARD));
assert_eq!(battle.intent_for(ids::ARTILLERY).unwrap().intended_occupant, Some(ids::VANGUARD));
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

- [ ] **Step 4: Delete Mission-1-specific opening matches from `enemy.rs`**

Round-0 movement looks up `EnemyOpening` by enemy ID; forced opening target resolves the authored target's current living position. Delete the old archetype/x-position opening match and `opening_target()` helper.

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
}
```

Also add a protect-target intent test asserting intended occupant Gunner and a tie fixture asserting more open orthogonal neighbors wins.

- [ ] **Step 6: Implement Flanker movement with one reused attack-band helper**

Extract current Rifleman/Striker logic to `choose_attack_band_destination(battle, id, candidates)`. Use it for Rifleman, Striker, and a Flanker that is neither the protected-target pressure unit nor the interception target.

Protect Flanker sort key: `(band_distance_to_target, manhattan_to_target, Reverse(open_neighbors), y, x)`.

Courier sort key: `(manhattan_to_escape, Reverse(open_neighbors), y, x)`.

Artillery stays unchanged.

- [ ] **Step 7: Add protected-target attack preference without changing committed intents**

For Flanker + protect rule, sort legal attack choices by `misses_protected_target` before the existing threatened-count/player-priority keys. Once committed, footprint/target remain locked through the player phase.

- [ ] **Step 8: Remove the remaining positional initiative hack**

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

Add a private test for those values. Existing Mission 1 intent order remains the regression for equal Rifleman tie-breaking by attacker ID.

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

Pin board 9×9, deployment V `(3,7)`, G `(4,6)`, I `(5,7)`, exact blocking/hazard/explosive cells, enemy IDs 21–24, protect rules, and Gunner existence.

For every opening row: enemy unit exists, target (when present) exists as a player, destination is in bounds and non-blocking. Construct with Gunner HP1 and assert max HP 15.

- [ ] **Step 2: Implement exact Mission 2 roster/opening**

```rust
static MISSION_TWO_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(2,4), target: Some(squad::ids::VANGUARD) },
    EnemyOpening { unit: ids::STRIKER, destination: GridPos::new(4,5), target: Some(squad::ids::GUNNER) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(4,0), target: Some(squad::ids::GUNNER) },
    EnemyOpening { unit: ids::FLANKER, destination: GridPos::new(5,5), target: Some(squad::ids::INTERCEPTOR) },
];
```

Starting positions: Rifleman `(2,2)`, Striker `(4,3)`, Artillery `(4,0)`, Flanker `(8,4)`. After `begin_round()`, assert intended occupants exactly Vanguard/Gunner/Gunner/Interceptor.

- [ ] **Step 3: Add exact Mission 2 definition/VN copy**

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
400 base / 100 bonus / unlock Three
```

Use the approved three pre-mission/two aftermath lines from the spec and existing VN assets only.

- [ ] **Step 4: Add immediate-clear lifecycle coverage**

Start Mission 2, KO all four enemies in Round 1 with `apply_direct_damage`, and assert the final KO emits `MissionCompleted` and `result.victory == true`.

- [ ] **Step 5: Add real three-resolution survival lifecycle coverage with enemies alive**

Build Mission 2 with Gunner HP3/Armor3 upgrades, begin Round 1, and use a local helper that finishes all living players with Guard. Resolve Round 1 → player Round 2/no result; resolve Round 2 → player Round 3/no result; assert at least one enemy remains; resolve Round 3 → victory.

- [ ] **Step 6: Add protect failure and bonus boundaries**

Gunner KO fails immediately. At integer half HP, winning sets `optional_complete`; one HP lower wins without bonus. Newly-earned bonus event immediately precedes `MissionCompleted`.

- [ ] **Step 7: Add MissionId Three/Four and definition-driven routing once**

```rust
pub enum MissionId { One, Two, Three, Four }
```

`mission_definition`: One/Two `Some`, Three/Four `None` in this task. Add `number()`.

Continue routing is immediately final: One→story, Two/Three→Upgrade, Four→NextMission. Upgrade `Proceed` checks whether the next mission is authored; handoff copy becomes `MISSION {number} UNLOCKED`.

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

- [ ] **Step 1: Write exact Mission 3 authoring/reference tests**

Pin board 9×9, player deployment `(4,7)/(3,8)/(5,8)`, blocking `(4,3),(4,4),(4,5)`, hazard `(2,5)`, explosive `(6,3)` HP4, Courier/Rifleman/Striker roster, Courier Flanker archetype/start `(0,6)`, escape `(8,0)`, deadline 5, and Manhattan distance 14.

Validate opening references. Assert escape is in bounds, non-blocking, not hazard, and not a live explosive.

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

Definition copy uses `Round 5`; use the approved VN lines from the spec.

- [ ] **Step 3: Add focused interception outcomes**

Escort clear alone does not win. Courier KO with escort alive wins. Round-2 KO earns bonus; Round-3 KO does not. Courier already on `(8,0)` fails at terminal check. EnemyPlanning round 5 with living Courier not on exit fails from deadline.

- [ ] **Step 4: Add durable timing helpers**

Use HP3/Armor3 on all three player mechs and a `finish_living_players_with_guard` helper in timing tests so seeded damage does not hide clock defects.

- [ ] **Step 5: Prove player Round 4 exists before extraction**

Remove escorts after opening. Resolve Rounds 1–3. After each resolution, assert no result and exact Player rounds 2,3,4. At player Round 4 assert Courier is not `(8,0)`. Do not assert an exact remaining distance.

- [ ] **Step 6: Prove the open route extracts after player Round 4**

From an equivalent fresh fixture, resolve player Round 4 and assert Courier position `(8,0)`, defeat result, and `MissionFailed` event. This is the headline chase contract.

- [ ] **Step 7: Prove blocked extraction reaches the Round-5 deadline backstop**

Place a durable living Interceptor on `(8,0)` via the existing test seam, remove escorts, and drive through Round 4. After resolving Round 4, assert Player Round 5/no result/Courier not on exit. Capture Courier position; resolve player Round 5; assert deadline defeat and unchanged Courier position, proving no fifth later move occurred.

- [ ] **Step 8: Add player-caused push-into-extraction regression**

Place Vanguard `(6,0)`, Courier `(7,0)`, call `resolve_push`, assert Courier moves to `(8,0)`, result is defeat, and events contain `MissionFailed`.

- [ ] **Step 9: Make Mission 3 authored and prove progression/save**

`mission_definition(Three)` becomes authored; Four remains handoff. Complete One/Two/Three without bonuses → Four and 1200 credits. Optional reward changes credits only. Persist/reload Four with credits/upgrades exactly; no migration/schema version.

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
- Modify: `tests/campaign_flow.rs`

**Interfaces:**
- Changes: `MISSION_ONE_SCENE_COUNT` from 10 to 11.
- Changes: `scene_index(UnitArchetype::Flanker) = 10`.
- Produces: rule-aware HUD/result copy and extraction ring.
- Does not introduce `unit_scale`, a Flanker child under-ring, or inverse-scale compensation.

- [ ] **Step 1: Write the glTF authoring test before editing the asset**

Parse `assets/models/mission_one.gltf` with `serde_json::Value`; assert 11 scenes, scene 10 named `Flanker`, nodes `[49,50,51,52,53,54,55]`, mesh 10 named `Flanker Magenta`, and material 10 named `Flanker Magenta`.

- [ ] **Step 2: Append scene 10 and nodes 49–55**

```text
49 Left Leg      translation [-0.16, 0.18,  0.00] scale [0.12, 0.36, 0.16]
50 Right Leg     translation [ 0.16, 0.18,  0.00] scale [0.12, 0.36, 0.16]
51 Torso         translation [ 0.00, 0.62,  0.00] scale [0.36, 0.42, 0.28]
52 Head          translation [ 0.00, 0.95,  0.00] scale [0.20, 0.20, 0.20]
53 Left Fin      translation [-0.42, 0.67, -0.10] scale [0.42, 0.08, 0.28]
54 Right Fin     translation [ 0.42, 0.67, -0.10] scale [0.42, 0.08, 0.28]
55 Rear Thruster translation [ 0.00, 0.52, -0.34] scale [0.20, 0.16, 0.34]
```

All use mesh 10.

- [ ] **Step 3: Add mesh/material 10 using existing accessors**

Mesh primitive uses POSITION 0, NORMAL 1, indices 2, material 10. Material `Flanker Magenta` uses base color `[0.78,0.08,0.46,1.0]`, metallic `0.25`, roughness `0.62`, emissive `[0.08,0.0,0.04]`. Do not change buffer/accessors.

- [ ] **Step 4: Load/map the new scene without scale special cases**

Set `MISSION_ONE_SCENE_COUNT = 11`; map Flanker to scene 10. Keep the existing root scale `0.72` everywhere. Add a pure scene-index/count test.

- [ ] **Step 5: Write objective-generic UI-copy tests**

Mission 2 HUD: protect-or-clear primary, `Round 1/3`, Gunner HP, Hold Fast state. Mission 3 HUD: `Round 1/5`, cells from extraction, Swift Intercept state. Pin generic result overlay and aftermath `Bonus +...` copy.

- [ ] **Step 6: Implement rule-aware HUD/result/event copy**

Eliminate shows enemy count; protect shows round + target HP; intercept shows round + Manhattan distance. Bonus state is Turnabout Complete/Not yet, half-HP On track/Missed, or victory-by-round Available/Missed. Terminal uses `optional_complete`; playback says `BONUS OBJECTIVE COMPLETE`.

- [ ] **Step 7: Spawn extraction ring from the authored Mission 3 rule**

Use existing `ring_mesh` + `intended_target` under `PresentationRoot` at the rule's escape cell `(8,0)`; add no domain prop type.

- [ ] **Step 8: Remove touched Mission-1-only debug root names**

Use `Mission Presentation`; keep the file name `mission_one.gltf`.

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

- [ ] **Step 1: Add Mission 2 renderer-free entry coverage**

Create state with next mission Two + Gunner HP1; run battle entry; assert ActiveMission Two, protect rule, round1, Gunner max HP15.

- [ ] **Step 2: Add Mission 3 entry/restart coverage**

Enter Three, mutate, restart fixed seed; assert ActiveMission remains Three, Courier HP8, escape `(8,0)`, deadline5. Use existing restarted-round seam; no mission-ID special case.

- [ ] **Step 3: Add saved-ID and Proceed routing assertions**

```text
Continue One   -> PreMissionStory
Continue Two   -> Upgrade
Continue Three -> Upgrade
Continue Four  -> NextMission
Proceed Two    -> PreMissionStory
Proceed Three  -> PreMissionStory
Proceed Four   -> NextMission
```

- [ ] **Step 4: Add save-backed progression continuity**

Start new game, complete M1 no bonus, buy Vanguard HP1 (200), complete M2 no bonus, buy Gunner HP1 (200), complete M3 no bonus, reload; assert Four, 800 credits, both purchases retained.

- [ ] **Step 5: Run and commit only changed files**

```bash
cargo fmt --check
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test presentation_app
cargo test --all-targets

git add tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs
git commit -m "test: cover campaign progression through mission 3"
```

If a test forces a source fix, stage that exact file separately; do not pre-stage conditional files.

---

### Task 7: Update docs, playtest encounter feel, and record final validation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-637.md`
- Modify: Mission 2/3 authored content only if playtesting exposes a feel/balance issue that does not contradict locked semantics

- [ ] **Step 1: Update README**

Document three-mission flow, rewards, Continue semantics, M2 protect-or-clear behavior, M3 extraction/Round5 behavior, distinct Flanker, unchanged controls/pilot skills.

- [ ] **Step 2: Update CLAUDE.md**

Record MissionRules, `completed_enemy_round`, shared enemies, explicit Flanker planner/fallback, fixed archetype initiative, M2/M3 semantics, glTF scene10, committed-intent invariant, HPA-637 docs.

- [ ] **Step 3: Run all automated gates before manual play**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

- [ ] **Step 4: Manual Mission 2 validation**

Record competing threats, meaningful reactions/Aegis, Gunner KO fail, immediate enemy-clear win, full Round3 win with attackers alive, bonus achieved/missed.

- [ ] **Step 5: Manual Mission 3 validation**

Record magenta Flanker, extraction ring, player Round4 exists, open-route extraction after Round4, blocked exit reaches Round5 backstop, Courier-only victory, escort-clear non-victory, early bonus.

- [ ] **Step 6: Manual save/continue continuity**

Quit/relaunch after M1/M2; Continue opens Upgrade and Proceed enters next mission. After M3, Continue shows Mission4 unlocked.

- [ ] **Step 7: Write concrete validation ledger**

Include head SHA, gate outcomes/test counts, M2 early-clear + Round3 lifecycle evidence, M3 Round4/extraction/blocked-deadline/push evidence, glTF scene evidence, save/upgrade continuity, short-session verdict. No placeholders.

- [ ] **Step 8: Re-run gates and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release

git add README.md CLAUDE.md docs/validation/hpa-637.md
git commit -m "docs: validate HPA-637 missions 2 and 3"
```

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
- [ ] Mission 3 reaches player Round4 after three later Courier moves.
- [ ] Open route extracts on the fourth later move after player Round4.
- [ ] Blocked extraction reaches player Round5, then deadline fires before another Courier move.
- [ ] Pushing Courier onto extraction fails immediately.
- [ ] Courier KO wins with escorts alive; escort clear alone does not win.
- [ ] Non-objective Flanker uses attack-band movement.
- [ ] Initiative order is Striker30/Flanker25/Rifleman20/Artillery10; no positional Rifleman hack remains.
- [ ] Mission authoring tests validate targets/openings and legal extraction.
- [ ] Flanker uses checked-in glTF scene10; scene count is11.
- [ ] No `unit_scale`, Flanker child under-ring, or inverse-scale compensation.
- [ ] Extraction uses existing white ring material.
- [ ] Briefing/HUD/results show primary + bonus; M3 HUD is Round n/5.
- [ ] Bonuses alter credits only.
- [ ] M1 → M2 → M3 → M4 works with save/Continue/Upgrade.
- [ ] Base-only completion yields 1200 credits.
- [ ] README, CLAUDE.md, validation ledger are current.
- [ ] `cargo fmt --check` passes.
- [ ] strict Clippy passes.
- [ ] all-target tests pass.
- [ ] release build passes.

## Self-review

- **Review corrections:** reachable extraction + Round5 backstop; no empty-round protect busywork; real Flanker scene; named boundary helper; positional initiative hack removed; authoring references tested; push-to-extraction covered; MissionId added once; speculative staging removed.
- **Clock consistency:** deadline 5 everywhere; Round4 exists; open-route extraction is live; blocked route uses Round5 backstop.
- **Protect consistency:** early clear is explicit in rules, briefing, tests, and manual validation.
- **Asset consistency:** scene10, no runtime scale/under-ring workaround.
- **Placeholder scan:** no `TBD`, `TODO`, stub test body, or deferred design decision.
- **Scope:** one bounded implementation PR.
- **Mission consistency:** One→Two→Three→Four, only One–Three authored.
- **Reward consistency:** base 300/400/500; bonuses 100/100/150; 1200 base total.