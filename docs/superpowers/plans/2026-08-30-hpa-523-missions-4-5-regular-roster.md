# HPA-523 Missions 4–5 and Regular Enemy Roster Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Missions 4–5, Bulwark, and Controller as one player-visible HPA-523 slice, completing the six-enemy regular roster and advancing the campaign to the Mission 6 handoff.

**Architecture:** Extend the existing closed Rust domain model with one exact target-elimination objective and two explicit enemy archetypes. Keep mission geometry/content in `mission_four.rs` / `mission_five.rs`, reuse current push/environment/intent/campaign/UI seams, and append two scenes to the existing checked-in glTF rather than creating new frameworks or asset paths.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus the existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-30-hpa-523-missions-4-5-regular-roster-design.md`

## Global Constraints

- One HPA-523 ticket = one PR. Continue implementation on this planning branch/PR.
- Seven task commits stay in this PR; do not split the ticket into prerequisite or follow-up PRs.
- No new dependencies, crates, objective framework, AI policy framework, generic statuses, displacement-resistance model, physics, scripting/data format, save migration, VN art, or asset pipeline.
- Add exactly two regular enemies: Bulwark and Controller. The final regular roster is exactly six.
- Add exactly one primary objective shape: `EliminateTarget { target: UnitId }`.
- Bulwark remains pushable through the existing `resolve_push`; there is no resistance system on `main`.
- Controller uses existing one-cell push and never commits a diagonal push target, whether the center came from normal targeting or an authored opening.
- Mission 4 uses only existing blocking, hazard, explosive, collision, and push rules.
- Mission 5 exploits already-locked Artillery footprints; do not special-case friendly fire.
- Mission IDs become One–Six; One–Five are authored and Six is the HPA-523 terminal handoff.
- Reuse `vn/relay_nine_bg.png`, `vn/control_alert.png`, `vn/control_neutral.png`, and `vn/vanguard_neutral.png`; add no VN files.
- Reuse `assets/models/mission_one.gltf`; final counts are 13 scenes, 70 nodes, 13 meshes, 13 materials, 1 buffer.
- CI gates remain `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo llvm-cov --all-targets --lcov --output-path lcov.info`, and `cargo build --release`.

## Risks

- **Mission 5 opening geometry is load-bearing.** Both Artillery `Cross1` footprints must include `(3,7)`, Gunner `(3,8) -> (2,7)` and Vanguard `(4,7) -> (3,5)` must remain legal public movement paths, and Bulwark `(3,6) -> (3,7)` must remain a legal push. Task 4's real `begin_round` + public movement/displacement test is required coverage and must not be replaced with `move_unit_direct_for_test` or treated as an optional authoring assertion.

---

### Task 1: Add the closed target-elimination objective and target HUD

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/presentation/ui.rs`
- Test: `src/domain/battle.rs`
- Test: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: current `PrimaryObjective`, `BattleState::check_terminal_state`, `HudSnapshot::from_battle`.
- Produces: `PrimaryObjective::EliminateTarget { target: UnitId }`; `ObjectiveTrackSnapshot::Target { name, hp, max_hp }`.

- [ ] **Step 1: Write the failing domain tests for target-only victory/failure**

Add focused `battle.rs` tests using a Mission 1 fixture with overridden rules:

```rust
#[test]
fn eliminate_target_wins_when_target_falls_with_escorts_alive() {
    let mut battle = mission_one(7);
    battle.set_rules_for_test(MissionRules {
        primary: PrimaryObjective::EliminateTarget { target: ids::STRIKER },
        optional: OptionalObjective::Turnabout,
        opening_plan: &[],
    });

    battle.apply_direct_damage(
        ids::STRIKER,
        99,
        DamageSource::PlayerWeapon(ids::PILE_LANCE),
    );

    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(!battle.unit(ids::RIFLEMAN_LEFT).unwrap().is_knocked_out());
}

#[test]
fn eliminate_target_loses_when_players_are_wiped_while_target_lives() {
    let mut battle = mission_one(7);
    battle.set_rules_for_test(MissionRules {
        primary: PrimaryObjective::EliminateTarget { target: ids::STRIKER },
        optional: OptionalObjective::Turnabout,
        opening_plan: &[],
    });
    for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
        battle.apply_direct_damage(player, 99, DamageSource::Collision);
    }

    assert!(battle.result().is_some_and(|result| !result.victory));
    assert!(!battle.unit(ids::STRIKER).unwrap().is_knocked_out());
}
```

- [ ] **Step 2: Run the domain tests and confirm the red state**

```bash
cargo test --lib eliminate_target -- --nocapture
```

Expected: compile failure because `PrimaryObjective::EliminateTarget` does not exist.

- [ ] **Step 3: Add the minimal objective variant and terminal rule**

In `model.rs` add:

```rust
EliminateTarget { target: UnitId },
```

In `check_terminal_state` add:

```rust
PrimaryObjective::EliminateTarget { target } => {
    let target_alive = self.unit(target).is_some_and(|unit| !unit.is_knocked_out());
    if !target_alive {
        Some(true)
    } else if !any_living_player {
        Some(false)
    } else {
        None
    }
}
```

Do not change `ObjectiveProgress`, persistence, or add an objective abstraction.

- [ ] **Step 4: Add the failing HUD projection test**

Construct an `EliminateTarget` fixture and pin:

```rust
assert_eq!(
    hud.objective_track,
    Some(ObjectiveTrackSnapshot::Target {
        name: "Striker",
        hp: 12,
        max_hp: 12,
    })
);
assert!(!hud.primary.contains("remaining"));
```

Keep an `EliminateAllEnemies` assertion that still includes the remaining-enemy count.

- [ ] **Step 5: Implement target tracking and objective-specific primary copy**

Extend `ObjectiveTrackSnapshot`:

```rust
Target {
    name: &'static str,
    hp: i16,
    max_hp: i16,
},
```

Map `EliminateTarget` to target HP. Change primary copy construction to:

```rust
let primary = match battle.rules().primary {
    PrimaryObjective::EliminateAllEnemies => {
        format!("{} · {remaining} remaining", definition.primary_objective)
    }
    _ => definition.primary_objective.to_owned(),
};
```

`round_cap` remains only Protect/Intercept; `EliminateTarget` has no round cap.

- [ ] **Step 6: Run focused and full tests**

```bash
cargo fmt --check
cargo test --lib eliminate_target
cargo test --test presentation_app
cargo test --all-targets
```

Expected: all pass.

- [ ] **Step 7: Commit the objective slice**

```bash
git add src/domain/model.rs src/domain/battle.rs src/presentation/ui.rs tests/presentation_app.rs
git commit -m "feat: add target elimination objective"
```

---

### Task 2: Complete the regular enemy roster and Controller push planning

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/mission/enemies.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/interaction.rs`
- Test: `src/domain/enemy.rs`
- Test: `src/mission/enemies.rs`

**Interfaces:**
- Consumes: `attack_band_destination`, `distance_to_band`, `choose_target`, `WeaponSpec.push`, `BattleState::resolve_push`, and the existing exhaustive `UnitArchetype` presentation matches.
- Produces: `UnitArchetype::{Bulwark, Controller}`, `enemies::bulwark`, `enemies::controller`, `enemies::bastion_cannon`, `enemies::vector_projector`, compiling temporary scene mappings until Task 5, and a shared enemy-side push-alignment predicate.

- [ ] **Step 1: Write exact factory/weapon tests before adding the variants**

Pin:

```rust
let bulwark = bulwark(UnitId(41), "Gate Bulwark", GridPos::new(4, 5));
assert_eq!(bulwark.stats.max_hp, 16);
assert_eq!(bulwark.stats.armor, 4);
assert_eq!(bulwark.stats.movement, 1);
assert_eq!(bulwark.stats.accuracy, 76);
assert_eq!(bulwark.stats.evasion, 0);
assert_eq!(bulwark.weapons, vec![ids::BASTION_CANNON]);

let controller = controller(UnitId(42), "Controller", GridPos::new(0, 7));
assert_eq!(controller.stats.max_hp, 9);
assert_eq!(controller.stats.armor, 1);
assert_eq!(controller.stats.movement, 2);
assert_eq!(controller.stats.accuracy, 82);
assert_eq!(controller.stats.evasion, 15);
assert_eq!(controller.weapons, vec![ids::VECTOR_PROJECTOR]);
```

Weapon assertions:

```rust
let cannon = bastion_cannon();
assert_eq!(cannon.id, ids::BASTION_CANNON);
assert_eq!((cannon.min_range, cannon.max_range), (1, 3));
assert_eq!(cannon.base_damage, 6);
assert!(!cannon.push);

let projector = vector_projector();
assert_eq!(projector.id, ids::VECTOR_PROJECTOR);
assert_eq!((projector.min_range, projector.max_range), (2, 4));
assert_eq!(projector.base_damage, 3);
assert_eq!(projector.hit_modifier, 10);
assert_eq!(projector.crit_chance, 0);
assert!(projector.push);
```

- [ ] **Step 2: Run factory tests and confirm the red state**

```bash
cargo test --lib mission::enemies::tests -- --nocapture
```

Expected: compile failures for the new archetypes/factories/weapon IDs.

- [ ] **Step 3: Add the archetypes/factories and immediately keep every exhaustive match compiling**

Add to `UnitArchetype`:

```rust
Bulwark,
Controller,
```

Add IDs:

```rust
pub const BASTION_CANNON: WeaponId = WeaponId(205);
pub const VECTOR_PROJECTOR: WeaponId = WeaponId(206);
```

Construct the exact stats/weapons using the existing `unit`, `stats`, and `weapon` helpers. Do not add fields to `UnitStats` or `WeaponSpec`.

In the same step, update the exhaustive matches that otherwise make `--all-targets` fail before Task 5:

`src/presentation/ui.rs`:

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Artillery
| UnitArchetype::Flanker
| UnitArchetype::Bulwark
| UnitArchetype::Controller => false,
```

and the same enemy set returns `"[P] PILOT"` in `pilot_label`.

`src/presentation/interaction.rs`:

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Artillery
| UnitArchetype::Flanker
| UnitArchetype::Bulwark
| UnitArchetype::Controller => {
    return Err(BattleError::PilotSkillWrongUnit(unit_id));
}
```

`src/presentation/battlefield.rs` uses the existing Flanker scene as a temporary compile-safe stand-in until Task 5 appends real scenes:

```rust
UnitArchetype::Flanker
| UnitArchetype::Bulwark
| UnitArchetype::Controller => 10,
```

Do not change the glTF or scene count in Task 2.

- [ ] **Step 4: Write enemy-planning tests for Controller, authored push safety, Bulwark movement, and initiative**

Keep all fixtures inside `src/domain/enemy.rs` tests so private planner functions can be exercised without new public seams.

Pin five behaviors:

```text
A. Controller: aligned lane exists -> choose_enemy_destination picks a reachable aligned range-2..4 cell.
B. Controller: no aligned lane exists -> falls back to attack_band_destination deterministically.
C. Dynamic Controller intent -> a push weapon never chooses a diagonal committed center.
D. Authored/forced Controller intent -> a diagonal forced target is rejected before any AttackIntent is committed.
E. Bulwark -> with a reachable attack-band cell, choose_enemy_destination leaves origin instead of idling.
```

For C, call private `build_intent(&battle, controller_id, None)` in the module test and assert the Single-shape committed center shares `x` or `y` with its intended occupant when one exists.

For D, place Controller at `(0,0)` and a player at `(2,2)`, then call:

```rust
let error = build_intent(&battle, controller_id, Some(GridPos::new(2, 2))).unwrap_err();
assert_eq!(
    error,
    BattleError::PushTargetNotAligned {
        attacker: GridPos::new(0, 0),
        target: GridPos::new(2, 2),
    }
);
assert!(battle.intents().is_empty());
```

For E, use a Bulwark fixture where one Move-1 reachable cell improves distance to its range 1–3 attack band:

```rust
let destination = choose_enemy_destination(&battle, bulwark_id).unwrap();
assert_ne!(destination, battle.unit(bulwark_id).unwrap().position);
```

Pin initiative:

```rust
assert_eq!(initiative(&controller), 35);
assert_eq!(initiative(&striker), 30);
assert_eq!(initiative(&flanker), 25);
assert_eq!(initiative(&rifleman), 20);
assert_eq!(initiative(&bulwark), 15);
assert_eq!(initiative(&artillery), 10);
```

- [ ] **Step 5: Implement the smallest Bulwark/Controller movement branches**

In `choose_enemy_destination`:

```text
Rifleman | Striker | Bulwark -> existing attack-band path
Controller -> controller_destination
Flanker -> existing explicit Flanker branch
Artillery -> existing artillery branch
```

Add `controller_destination` over the existing candidate list. A candidate is preferred when at least one living player is aligned and inside the Vector Projector's range:

```rust
let aligned_in_band = players.iter().any(|player| {
    let distance = position.manhattan(player.position);
    let aligned = position.x == player.position.x || position.y == player.position.y;
    aligned && distance >= weapon.min_range && distance <= weapon.max_range
});
```

If any candidate satisfies it, restrict to those and choose by:

```text
(distance_to_band to nearest player, nearest Manhattan distance, y, x)
```

Otherwise reuse `attack_band_destination`. No policy object or RNG.

- [ ] **Step 6: Enforce push alignment for both dynamic targeting and authored openings**

Add one local helper beside targeting code:

```rust
fn push_target_aligned(attacker: GridPos, target: GridPos) -> bool {
    attacker.x == target.x || attacker.y == target.y
}
```

In `choose_target`, keep the range filter and reject diagonal centers for push weapons:

```rust
let in_range = distance >= weapon.min_range && distance <= weapon.max_range;
in_range && (!weapon.push || push_target_aligned(attacker.position, *target))
```

Then enforce the invariant again in `build_intent` after the final `choice` is selected, which covers authored `forced_target` values that bypass `choose_target`:

```rust
if weapon.push && !push_target_aligned(attacker.position, choice.center) {
    return Err(BattleError::PushTargetNotAligned {
        attacker: attacker.position,
        target: choice.center,
    });
}
```

This copies the existing player-side alignment rule conceptually; do not create a new combat-rule type. A bad authored opening fails before commitment instead of reaching `resolve_push` during enemy resolution.

- [ ] **Step 7: Run roster/planning/presentation compile regressions**

```bash
cargo fmt --check
cargo test --lib mission::enemies::tests
cargo test --lib domain::enemy::tests
cargo test --lib domain::environment::tests
cargo test --all-targets
```

Expected: existing Rifleman/Striker/Artillery/Flanker behavior remains green; the new Bulwark movement and both Controller push-alignment paths pass; `--all-targets` compiles with the temporary scene-10 mapping.

- [ ] **Step 8: Commit the roster slice**

```bash
git add src/domain/model.rs src/domain/enemy.rs src/mission/enemies.rs \
  src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add Bulwark and Controller enemies"
```

---

### Task 3: Author Mission 4 and make Mission 4/5 real campaign IDs

**Files:**
- Create: `src/mission/mission_four.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Test: `src/mission/mission_four.rs`
- Test: `tests/campaign_flow.rs`
- Test: `tests/campaign_model.rs`

**Interfaces:**
- Consumes: `build_player_squad`, shared enemy factories, `MissionDefinition`, `EliminateTarget`, `Turnabout`.
- Produces: `MISSION_FOUR_DEFINITION`, `mission_four_for_campaign`, `MissionId::{Five, Six}` with Six still a handoff.

- [ ] **Step 1: Add failing Mission 4 authoring tests**

Pin:

```text
Board 9×9
Players V(4,7) G(3,8) I(5,8)
Blocking (2,4),(6,4),(2,5),(6,5)
Hazard (4,3)
Explosives (3,4) HP4, (5,4) HP4
Bulwark41 (4,5)->(4,4), Vanguard
Controller42 (0,7)->(1,7), Vanguard
Rifleman43 (8,6)->(6,6), Interceptor
Primary EliminateTarget Bulwark41
Optional Turnabout
Reward 600+150
Unlock Five
```

For every opening row assert enemy faction, movement reachability, in-bounds/open destination, and a real player target when present. For Controller also assert destination-to-Vanguard is row/column aligned and inside Vector Projector range 2–4.

- [ ] **Step 2: Add failing environmental geometry tests**

Drive the real opening first:

```rust
let mut battle = mission_four(7);
battle.begin_round().unwrap();
```

Pin the explosive line:

```rust
battle.begin_activation(ids::GUNNER).unwrap();
let preview = battle
    .preview_attack(ids::GUNNER, squad::ids::RAIL_RIFLE, GridPos::new(3, 4))
    .unwrap();
assert_eq!(preview.target, GridPos::new(3, 4));
```

Resolve the explosive deterministically or call `damage_explosive` after the legal range preview; assert `ExplosionTriggered.footprint` contains `(4,4)` and Bulwark receives `DamageSource::Explosion`.

In a separate fixture, use public movement then the existing displacement seam:

```rust
battle.begin_activation(ids::VANGUARD).unwrap();
battle.move_unit(ids::VANGUARD, GridPos::new(4, 5)).unwrap();
let events = battle.resolve_push(ids::VANGUARD, ids::BULWARK).unwrap();
assert_eq!(battle.unit(ids::BULWARK).unwrap().position, GridPos::new(4, 3));
assert!(events.iter().any(|event| matches!(event, BattleEvent::HazardTriggered { .. })));
```

Also pin target-only victory with Controller/Rifleman alive and Turnabout completion from qualifying environment/enemy damage.

- [ ] **Step 3: Run Mission 4 tests and confirm red**

```bash
cargo test --lib mission::mission_four -- --nocapture
```

Expected: module/ID/definition symbols are not implemented yet.

- [ ] **Step 4: Grow mission IDs and register Mission 4**

```rust
pub enum MissionId {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
}
```

`Display` maps 1–6. Add `pub mod mission_four;` and:

```rust
MissionId::Four => Some(&mission_four::MISSION_FOUR_DEFINITION),
MissionId::Five | MissionId::Six => None,
```

Five is temporarily un-authored until Task 4; Six is the final HPA-523 handoff.

- [ ] **Step 5: Implement Mission 4 exactly as the spec**

IDs:

```rust
pub const BULWARK: UnitId = UnitId(41);
pub const CONTROLLER: UnitId = UnitId(42);
pub const RIFLEMAN: UnitId = UnitId(43);
```

Definition:

```text
Title: Mission 4 — Breach the Gate
Primary: Destroy the Gate Bulwark; escorts may be ignored.
Bonus: Chain Reaction: damage any enemy with enemy fire, collision, hazard, or explosion.
Reward: 600 + 150
Four -> Five
```

Use only existing VN file paths and the exact dialogue from the spec.

- [ ] **Step 6: Move Continue's temporary handoff forward**

```rust
Ok(MissionId::Two | MissionId::Three | MissionId::Four) => {
    next_state.set(GameScreen::Upgrade)
}
Ok(MissionId::Five | MissionId::Six) => next_state.set(GameScreen::NextMission),
```

Update comments that still call Four terminal. Task 4 authors Five and leaves only Six in the handoff branch.

- [ ] **Step 7: Add campaign tests through Mission 4**

Exercise real Mission 3 completion into Four and Mission 4 completion into Five. Pin base rewards through Four:

```text
300 + 400 + 500 + 600 = 1800
```

Keep upgrade projection assertions on Mission 4 construction.

- [ ] **Step 8: Run gates**

```bash
cargo fmt --check
cargo test --lib mission::mission_four
cargo test --test campaign_model
cargo test --test campaign_flow
cargo test --all-targets
```

- [ ] **Step 9: Commit Mission 4**

```bash
git add src/mission/mission_four.rs src/mission/mod.rs src/presentation/campaign_ui.rs \
  tests/campaign_flow.rs tests/campaign_model.rs
git commit -m "feat: add Mission 4 environmental breach"
```

---

### Task 4: Author Mission 5 and pin the load-bearing artillery crossfire geometry

**Files:**
- Create: `src/mission/mission_five.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Test: `src/mission/mission_five.rs`
- Test: `tests/campaign_flow.rs`
- Test: `tests/campaign_model.rs`
- Test: `tests/campaign_persistence.rs`

**Interfaces:**
- Consumes: locked `AttackIntent` footprints, public movement, current `resolve_push`, `EliminateAllEnemies`, `VictoryByRound`.
- Produces: `MISSION_FIVE_DEFINITION`, `mission_five_for_campaign`, Five -> Six handoff, and durable evidence for the `(3,7)` crossfire line.

- [ ] **Step 1: Add failing Mission 5 authoring tests**

Pin:

```text
Board 9×9
Players V(4,7) G(3,8) I(5,8)
Blocking (1,4),(7,4),(1,5),(7,5)
Artillery51 (3,0) stays, target Gunner
Artillery52 (7,2) stays, target Vanguard
Bulwark53 (3,5)->(3,6), target Vanguard
Controller54 (0,7)->(1,7), target Vanguard
Flanker55 (8,7)->(6,7), target Interceptor
Primary EliminateAllEnemies
Optional VictoryByRound 4
Reward 700+200
Unlock Six
```

Validate every opening destination/target and Controller alignment/range.

- [ ] **Step 2: Write the required real-opening/public-movement crossfire regression**

Do not use `move_unit_direct_for_test` in this test.

```rust
let mut battle = mission_five(7);
battle.begin_round().unwrap();

let artillery_a = battle.intent_for(ids::ARTILLERY_A).unwrap();
let artillery_b = battle.intent_for(ids::ARTILLERY_B).unwrap();
assert!(artillery_a.footprint.contains(&GridPos::new(3, 7)));
assert!(artillery_b.footprint.contains(&GridPos::new(3, 7)));
assert_eq!(battle.unit(ids::BULWARK).unwrap().position, GridPos::new(3, 6));
```

Prove both exact-fit player paths on the public movement API:

```rust
battle.begin_activation(ids::GUNNER).unwrap();
battle.move_unit(ids::GUNNER, GridPos::new(2, 7)).unwrap();
battle.choose_reaction(ids::GUNNER, Reaction::Guard).unwrap();
battle.finish_activation(ids::GUNNER).unwrap();

battle.begin_activation(ids::VANGUARD).unwrap();
battle.move_unit(ids::VANGUARD, GridPos::new(3, 5)).unwrap();
let events = battle.resolve_push(ids::VANGUARD, ids::BULWARK).unwrap();
assert_eq!(battle.unit(ids::BULWARK).unwrap().position, GridPos::new(3, 7));
assert!(events.iter().any(|event| matches!(event, BattleEvent::UnitPushed { .. })));
```

Use existing `resolve_intent_for_test` for Artillery A then B and assert each event list contains:

```rust
BattleEvent::AttackRolled { target, .. } if *target == ids::BULWARK
```

Also assert the Controller's already-committed `(4,7)` footprint has no occupant after Vanguard vacates it. This test is the regression for the risk named above; keep the real `begin_round`, public `move_unit`, and real committed footprints.

- [ ] **Step 3: Add Round-4 bonus boundary tests**

Using the same durable round-step style as Mission 3:

```text
victory at round <= 4 -> optional_complete true
victory at round 5 -> optional_complete false
```

Knock out remaining enemies through the existing damage test seam so `check_terminal_state` evaluates the actual optional rule.

- [ ] **Step 4: Run Mission 5 tests and confirm red**

```bash
cargo test --lib mission::mission_five -- --nocapture
```

Expected: module/definition is missing.

- [ ] **Step 5: Implement Mission 5 exactly as authored**

IDs:

```rust
pub const ARTILLERY_A: UnitId = UnitId(51);
pub const ARTILLERY_B: UnitId = UnitId(52);
pub const BULWARK: UnitId = UnitId(53);
pub const CONTROLLER: UnitId = UnitId(54);
pub const FLANKER: UnitId = UnitId(55);
```

Definition:

```text
Title: Mission 5 — Crossfire Break
Primary: Break the assault and destroy all enemies.
Bonus: Rapid Break: win by the end of Round 4.
Reward: 700 + 200
Five -> Six
```

Use shared enemy factories/weapons, no new hazards/props, no artillery special case, and only existing VN assets/dialogue from the spec.

- [ ] **Step 6: Register Five and make Six the only handoff**

```rust
MissionId::Five => Some(&mission_five::MISSION_FIVE_DEFINITION),
MissionId::Six => None,
```

Continue routing:

```rust
Ok(MissionId::Two | MissionId::Three | MissionId::Four | MissionId::Five) => {
    next_state.set(GameScreen::Upgrade)
}
Ok(MissionId::Six) => next_state.set(GameScreen::NextMission),
```

`Proceed` stays definition-driven.

- [ ] **Step 7: Extend campaign/persistence tests through Six**

Pin:

```text
One -> Two -> Three -> Four -> Five -> Six
Base rewards through Five = 2500
Max optional through Five = 700
Max total through Five = 3200
```

Persist an upgrade before Mission 4, reload, construct Mission 4/5 with that state, finish Mission 5, save/reload Six, and assert upgrade levels plus intended credits survive.

- [ ] **Step 8: Run gates**

```bash
cargo fmt --check
cargo test --lib mission::mission_five
cargo test --test campaign_model
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --all-targets
```

- [ ] **Step 9: Commit Mission 5**

```bash
git add src/mission/mission_five.rs src/mission/mod.rs src/presentation/campaign_ui.rs \
  tests/campaign_flow.rs tests/campaign_model.rs tests/campaign_persistence.rs
git commit -m "feat: add Mission 5 artillery assault"
```

---

### Task 5: Append Bulwark/Controller glTF scenes and replace the temporary scene mappings

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Test: `src/presentation/assets.rs`
- Test: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: Task 2's temporary scene-10 mapping, `MissionAssets`, current single embedded glTF buffer/accessors.
- Produces: scene 11 Bulwark, scene 12 Controller, scene count 13, permanent `scene_index` mappings.

- [ ] **Step 1: Write the glTF structure test first**

Extend the JSON test:

```rust
assert_eq!(scenes.len(), 13);
assert_eq!(scenes[11]["name"], "Bulwark");
assert_eq!(scenes[11]["nodes"], serde_json::json!([56]));
assert_eq!(nodes[56]["scale"], serde_json::json!([0.88, 0.88, 0.88]));
assert_eq!(nodes[56]["children"], serde_json::json!([57, 58, 59, 60, 61, 62]));

assert_eq!(scenes[12]["name"], "Controller");
assert_eq!(scenes[12]["nodes"], serde_json::json!([63]));
assert_eq!(nodes[63]["scale"], serde_json::json!([0.72, 0.72, 0.72]));
assert_eq!(nodes[63]["children"], serde_json::json!([64, 65, 66, 67, 68, 69]));

assert_eq!(nodes.len(), 70);
assert_eq!(meshes.len(), 13);
assert_eq!(materials.len(), 13);
assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
```

Assert nodes 57–62 use mesh 11 and 64–69 use mesh 12; both meshes reuse POSITION accessor 0 and NORMAL accessor 1. Pin materials:

```text
11 “Bulwark Ochre” [0.78,0.38,0.08,1.0]
12 “Controller Cyan” [0.08,0.72,0.86,1.0]
```

- [ ] **Step 2: Run asset test and confirm red**

```bash
cargo test --lib presentation::assets::tests -- --nocapture
```

Expected: current scene/node/mesh/material counts are still 11/56/11/11.

- [ ] **Step 3: Append the two scenes without another buffer/file**

```text
Bulwark root 56 + children 57–62 -> mesh 11
Controller root 63 + children 64–69 -> mesh 12
```

Copy the exact Flanker child transforms 50–55, changing only mesh indices. Add meshes/materials 11 and 12 using the same cube accessors. Keep the existing single embedded buffer and accessor arrays.

- [ ] **Step 4: Replace only the temporary visual mappings**

Change:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 13;
```

Replace Task 2's temporary combined scene-10 arm with:

```rust
UnitArchetype::Flanker => 10,
UnitArchetype::Bulwark => 11,
UnitArchetype::Controller => 12,
```

The `ui.rs` and `interaction.rs` exhaustive enemy arms were already completed in Task 2 and are not Task 5 work. Do not add a per-archetype scale table.

- [ ] **Step 5: Add presentation assertions**

```rust
assert_eq!(scene_index(UnitArchetype::Bulwark), 11);
assert_eq!(scene_index(UnitArchetype::Controller), 12);
```

For Mission 4, assert Target tracker displays Gate Bulwark HP. For Mission 5, assert both Artillery intents appear in the threat list and remaining-enemy count appears because the primary is `EliminateAllEnemies`.

- [ ] **Step 6: Run gates**

```bash
cargo fmt --check
cargo test --lib presentation::assets::tests
cargo test --test presentation_app
cargo test --all-targets
```

- [ ] **Step 7: Commit visuals**

```bash
git add assets/models/mission_one.gltf src/presentation/assets.rs \
  src/presentation/battlefield.rs tests/presentation_app.rs
git commit -m "feat: present Bulwark and Controller"
```

---

### Task 6: Prove Mission 4–5 entry, restart, save, and upgrade continuity

**Files:**
- Test: `tests/campaign_flow.rs`
- Test: `tests/campaign_model.rs`
- Test: `tests/campaign_persistence.rs`
- Test: `tests/presentation_app.rs`
- Modify only for a concrete exhaustive-routing failure: `src/app.rs`
- Modify only for a concrete definition-handoff failure: `src/presentation/mod.rs`

**Interfaces:**
- Consumes: definition-driven battle construction/restart, `CampaignSession`, `ActiveMission`, MissionId One–Six.
- Produces: explicit integration evidence that Four/Five require no new app/session abstraction.

- [ ] **Step 1: Add integration tests**

Cover this exact sequence:

```text
1. persisted next_mission Four with a non-zero upgrade
2. Continue -> Upgrade
3. Proceed -> PreMissionStory because Four is authored
4. Start Mission -> M4 with upgrade projected
5. restart -> same M4 definition, campaign upgrade unchanged
6. M4 completion -> Five persisted
7. Continue/Proceed -> M5 with same upgrade
8. M5 completion -> Six persisted
9. Continue Six -> NextMission
```

Also assert base rewards through Five are exactly 2500 before optional rewards/purchases.

- [ ] **Step 2: Run the integration tests**

```bash
cargo test --test campaign_flow -- --nocapture
cargo test --test campaign_persistence -- --nocapture
cargo test --test presentation_app -- --nocapture
```

Expected: definition-driven code should already handle Four/Five. If compilation exposes a remaining exhaustive MissionId match or a literal Four handoff, update only that concrete branch; do not introduce a router/registry abstraction.

- [ ] **Step 3: Apply only the bounded correction named by a failing test**

Allowed production corrections are limited to:

```text
- an exhaustive MissionId match that lacks Five/Six;
- stale literal Four-terminal handoff logic/copy;
- a hard-coded mission lookup where mission_definition(next_mission) should already be used.
```

Do not add a save version, migration, mission router, or state-machine framework.

- [ ] **Step 4: Run the complete suite**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

- [ ] **Step 5: Commit integration evidence**

Stage the four test files plus only any production file actually corrected:

```bash
git add tests/campaign_flow.rs tests/campaign_model.rs tests/campaign_persistence.rs tests/presentation_app.rs
git add src/app.rs src/presentation/mod.rs 2>/dev/null || true
git commit -m "test: cover Mission 4-5 campaign continuity"
```

Keep the commit even when production code needs no correction; the integration tests themselves are the Task 6 deliverable.

---

### Task 7: Update docs, manually validate both encounters, and close the evidence ledger

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-523.md`

**Interfaces:**
- Consumes: final implementation SHA, automated output, manual playthrough observations.
- Produces: HPA-523 acceptance evidence with concrete values/results.

- [ ] **Step 1: Update docs to current campaign state**

README/CLAUDE must state:

```text
- campaign authored through Mission 5 with Mission 6 handoff
- final roster: Rifleman/Striker/Artillery/Flanker/Bulwark/Controller
- Mission 4: target breach/environment manipulation
- Mission 5: locked-artillery crossfire exploitation
- Bulwark has no displacement immunity
- Controller is push-only, no status system
- save remains local JSON at stable campaign transitions
```

Remove stale Mission-4-handoff wording.

- [ ] **Step 2: Run CI-equivalent gates and capture exact results**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
cargo test --all-targets
```

Record final test count and implementation SHA in `docs/validation/hpa-523.md`.

- [ ] **Step 3: Manually validate Mission 4**

```bash
cargo run
```

Record:

```text
- Bulwark reads visually heavier than regular enemies.
- Controller push telegraph is legible.
- Gunner can use explosive (3,4) to splash Bulwark.
- Vanguard can push Bulwark (4,4)->(4,3) onto hazard.
- Bulwark KO wins with escorts alive.
- Chain Reaction reward appears for qualifying enemy/environment damage.
- Encounter remains a short tactical session.
```

- [ ] **Step 4: Manually validate Mission 5**

Record:

```text
- both Artillery Cross1 footprints include (3,7)
- Gunner (3,8)->(2,7) is legal
- Vanguard (4,7)->(3,5) is legal
- Bulwark (3,6)->(3,7) push is legal
- both already-committed Artillery attacks can damage Bulwark at (3,7)
- Controller's vacated (4,7) hit is harmless
- Rapid Break rewards <= Round 4 but is not a failure deadline
- mixed telegraphs remain readable and encounter stays short
```

- [ ] **Step 5: Manually validate campaign continuity**

```text
M3 results -> upgrade -> M4 story/briefing/battle -> results -> upgrade
-> M5 story/briefing/battle -> results -> upgrade -> M6 handoff
```

Confirm Continue/restart preserve credits and upgrades.

- [ ] **Step 6: Write the validation ledger**

`docs/validation/hpa-523.md` contains only observed final values:

```text
- final implementation commit SHA
- exact commands and pass/fail result
- test count / coverage command result
- Mission 4 automated geometry + manual evidence
- Mission 5 real begin_round `(3,7)` crossfire + public movement/displacement evidence
- glTF 13/70/13/13/1 evidence
- campaign One->Six / 2500 base credits / save-upgrade evidence
- accepted tuning changes with final authored values
```

- [ ] **Step 7: Run final gates after docs**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Expected: green at PR head.

- [ ] **Step 8: Commit closeout**

```bash
git add README.md CLAUDE.md docs/validation/hpa-523.md
git commit -m "docs: validate HPA-523 missions 4-5"
```

---

## Final Review Gate

Before marking the PR ready for review:

- [ ] HPA-523 is one PR with seven task commits.
- [ ] Only one new primary objective exists: `EliminateTarget`.
- [ ] No displacement-resistance/status/AI/objective framework was added.
- [ ] Bulwark is HP16 / Armor4 / Move1, pushable, initiative15, and has a tested later-round attack-band movement path.
- [ ] Controller is HP9 / Armor1 / Move2 with range2–4 damage3 Push1, initiative35.
- [ ] Dynamic and authored Controller intents cannot commit a diagonal push target.
- [ ] Task 2's `cargo test --all-targets` is green before the real Bulwark/Controller glTF scenes exist; temporary scene 10 mapping is replaced in Task 5.
- [ ] Mission 4 preserves both authored environmental solutions and wins on Bulwark KO with escorts alive.
- [ ] Mission 5's real opening preserves the shared `(3,7)` dual-Artillery footprint, both exact-fit public movement paths, and the Bulwark displacement line.
- [ ] Mission 5's Round-4 condition is optional pressure, not a primary deadline.
- [ ] Regular roster totals exactly six archetypes.
- [ ] glTF final structure is 13 scenes / 70 nodes / 13 meshes / 13 materials / 1 buffer.
- [ ] One–Five are authored; Six is the only terminal handoff.
- [ ] Base rewards through Mission 5 total exactly 2500.
- [ ] Save, upgrades, Continue, restart, VN, briefing, results, and upgrade flow remain continuous through Mission 5.
- [ ] README/CLAUDE/validation ledger match shipped behavior.
- [ ] fmt, strict Clippy, all-target tests/coverage, and release build are green.
