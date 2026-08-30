# HPA-523 Missions 4–5 and Regular Enemy Roster Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Missions 4–5, Bulwark, and Controller as one player-visible HPA-523 slice, completing the six-enemy regular roster and advancing the campaign to the Mission 6 handoff.

**Architecture:** Extend the existing closed Rust domain model with one exact target-elimination objective and two explicit enemy archetypes. Keep mission geometry/content in `mission_four.rs` / `mission_five.rs`, reuse current push/environment/intent/campaign/UI seams, and append two scenes to the existing checked-in glTF rather than creating new frameworks or asset paths.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus the existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-30-hpa-523-missions-4-5-regular-roster-design.md`

## Global Constraints

- One HPA-523 ticket = one PR. Continue implementation on this planning branch/PR.
- No new dependencies, crates, objective framework, AI policy framework, generic statuses, displacement-resistance model, physics, scripting/data format, save migration, VN art, or asset pipeline.
- Add exactly two regular enemies: Bulwark and Controller. The final regular roster is exactly six.
- Add exactly one primary objective shape: `EliminateTarget { target: UnitId }`.
- Bulwark remains pushable through the existing `resolve_push`; there is no resistance system on `main`.
- Controller uses existing one-cell push and never commits a diagonal push target.
- Mission 4 uses only existing blocking, hazard, explosive, collision, and push rules.
- Mission 5 exploits already-locked Artillery footprints; do not special-case friendly fire.
- Mission IDs become One–Six; One–Five are authored and Six is the HPA-523 terminal handoff.
- Reuse `vn/relay_nine_bg.png`, `vn/control_alert.png`, `vn/control_neutral.png`, and `vn/vanguard_neutral.png`; add no VN files.
- Reuse `assets/models/mission_one.gltf`; final counts are 13 scenes, 70 nodes, 13 meshes, 13 materials, 1 buffer.
- CI gates remain `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo llvm-cov --all-targets --lcov --output-path lcov.info`, and `cargo build --release`.

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
        opening_plan: battle.rules().opening_plan,
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
        opening_plan: battle.rules().opening_plan,
    });
    for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
        battle.apply_direct_damage(player, 99, DamageSource::EnemyWeapon(ids::ARTILLERY, ids::SIEGE_MORTAR));
    }

    assert!(battle.result().is_some_and(|result| !result.victory));
    assert!(!battle.unit(ids::STRIKER).unwrap().is_knocked_out());
}
```

- [ ] **Step 2: Run the domain tests and confirm the red state**

Run:

```bash
cargo test --lib eliminate_target -- --nocapture
```

Expected: compile failure because `PrimaryObjective::EliminateTarget` does not exist.

- [ ] **Step 3: Add the minimal objective variant and terminal rule**

In `model.rs` add:

```rust
EliminateTarget { target: UnitId },
```

In `check_terminal_state` add the direct closed match arm:

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

Do not change `ObjectiveProgress` or serialization.

- [ ] **Step 4: Add the failing HUD projection test**

In presentation coverage, construct an `EliminateTarget` fixture and pin:

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

Keep an existing elimination mission assertion that still contains the remaining-enemy count.

- [ ] **Step 5: Implement target tracking and objective-specific primary copy**

Extend `ObjectiveTrackSnapshot`:

```rust
Target {
    name: &'static str,
    hp: i16,
    max_hp: i16,
},
```

Map `EliminateTarget` to the target unit HP. Build `HudSnapshot.primary` as:

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
- Test: `src/domain/enemy.rs`
- Test: `src/mission/enemies.rs`

**Interfaces:**
- Consumes: `attack_band_destination`, `distance_to_band`, `choose_target`, `WeaponSpec.push`, `BattleState::resolve_push`.
- Produces: `UnitArchetype::{Bulwark, Controller}`, `enemies::bulwark`, `enemies::controller`, `enemies::bastion_cannon`, `enemies::vector_projector`.

- [ ] **Step 1: Write exact factory/weapon tests before adding the variants**

Pin the values from the spec:

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
assert_eq!(bastion_cannon().id, ids::BASTION_CANNON);
assert_eq!((bastion_cannon().min_range, bastion_cannon().max_range), (1, 3));
assert_eq!(bastion_cannon().base_damage, 6);
assert!(!bastion_cannon().push);

assert_eq!(vector_projector().id, ids::VECTOR_PROJECTOR);
assert_eq!((vector_projector().min_range, vector_projector().max_range), (2, 4));
assert_eq!(vector_projector().base_damage, 3);
assert_eq!(vector_projector().hit_modifier, 10);
assert_eq!(vector_projector().crit_chance, 0);
assert!(vector_projector().push);
```

- [ ] **Step 2: Run factory tests and confirm the red state**

```bash
cargo test --lib mission::enemies::tests -- --nocapture
```

Expected: compile failures for the new archetypes/factories/weapon IDs.

- [ ] **Step 3: Add the explicit archetypes, IDs, factories, and weapons**

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

Construct exactly the stats and weapon values pinned above using the existing `unit`, `stats`, and `weapon` helpers. Do not add fields to `UnitStats` or `WeaponSpec`.

- [ ] **Step 4: Write Controller planning tests**

Add deterministic fixtures that prove three behaviors:

```text
A. aligned lane exists -> Controller chooses a reachable aligned range-2..4 cell
B. no aligned lane reachable -> Controller falls back to attack-band movement
C. push weapon target selection never commits a diagonal intended center
```

For C, place Controller diagonally from the nearest player with another legal row/column center inside range, call `begin_round`, and assert every Controller intent center/footprint is aligned with the intended player's current cell when `intended_occupant` is present.

Also pin initiative values:

```rust
assert_eq!(initiative(&controller), 35);
assert_eq!(initiative(&striker), 30);
assert_eq!(initiative(&flanker), 25);
assert_eq!(initiative(&rifleman), 20);
assert_eq!(initiative(&bulwark), 15);
assert_eq!(initiative(&artillery), 10);
```

- [ ] **Step 5: Implement the smallest Controller movement branch**

Add a `controller_destination` helper that receives the existing candidate list, living players, and weapon. Its key is:

```rust
let aligned_in_band = players.iter().any(|player| {
    let distance = position.manhattan(player.position);
    let aligned = position.x == player.position.x || position.y == player.position.y;
    aligned && distance >= weapon.min_range && distance <= weapon.max_range
});
```

If any candidate satisfies `aligned_in_band`, restrict to those and choose by:

```text
(distance_to_band to nearest player, nearest Manhattan distance, y, x)
```

Otherwise call the existing `attack_band_destination`.

In `choose_enemy_destination`:

```text
Bulwark -> existing attack-band path
Controller -> controller_destination
```

- [ ] **Step 6: Prevent diagonal target centers for enemy push weapons**

In the candidate filter inside `choose_target`, keep the current range check and add:

```rust
let aligned = attacker.position.x == target.x || attacker.position.y == target.y;
in_range && (!weapon.push || aligned)
```

Do not alter player attack validation; it already rejects diagonal push targeting.

- [ ] **Step 7: Run roster/planning regressions**

```bash
cargo fmt --check
cargo test --lib mission::enemies::tests
cargo test --lib domain::enemy::tests
cargo test --lib domain::environment::tests
cargo test --all-targets
```

Expected: existing Rifleman/Striker/Artillery/Flanker behavior remains green; new tests pass.

- [ ] **Step 8: Commit the roster slice**

```bash
git add src/domain/model.rs src/domain/enemy.rs src/mission/enemies.rs
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

Create the module with tests first and pin:

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

For every opening row assert:

```rust
assert_eq!(unit.faction, Faction::Enemy);
assert!(opening.destination.manhattan(unit.position) <= unit.stats.movement);
assert!(battle.board().contains(opening.destination));
assert!(!battle.board().is_blocking(opening.destination));
```

For Controller specifically also assert the opening destination is row/column aligned with Vanguard and the distance is inside Vector Projector range 2–4.

- [ ] **Step 2: Add failing environmental geometry tests**

Pin the two concrete solutions:

```rust
let mut battle = mission_four(7);
battle.begin_round().unwrap();

// Gunner can target the left explosive and its explosion footprint reaches Bulwark.
battle.begin_activation(ids::GUNNER).unwrap();
let preview = battle.preview_attack(ids::GUNNER, squad::ids::RAIL_RIFLE, GridPos::new(3, 4)).unwrap();
assert_eq!(preview.target, GridPos::new(3, 4));

// In a separate fixture, Vanguard can occupy (4,5) and push Bulwark to hazard (4,3).
```

For the push fixture use public movement then `resolve_push` directly to remove RNG from the geometry assertion:

```rust
battle.begin_activation(ids::VANGUARD).unwrap();
battle.move_unit(ids::VANGUARD, GridPos::new(4, 5)).unwrap();
let events = battle.resolve_push(ids::VANGUARD, ids::BULWARK).unwrap();
assert_eq!(battle.unit(ids::BULWARK).unwrap().position, GridPos::new(4, 3));
assert!(events.iter().any(|event| matches!(event, BattleEvent::HazardTriggered { .. })));
```

Add a target-victory test that damages only Bulwark to KO and verifies Controller/Rifleman remain alive when victory is emitted. Add a Turnabout assertion using hazard/explosion damage.

- [ ] **Step 3: Run Mission 4 tests and confirm the red state**

```bash
cargo test --lib mission::mission_four -- --nocapture
```

Expected: module/ID/definition symbols are not implemented yet.

- [ ] **Step 4: Grow the mission IDs and registry once**

Change the enum to:

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

`Display` maps them to 1–6. Add `pub mod mission_four;` and register Four:

```rust
MissionId::Four => Some(&mission_four::MISSION_FOUR_DEFINITION),
MissionId::Five | MissionId::Six => None,
```

Five is temporarily un-authored until Task 4; Six remains the final handoff.

- [ ] **Step 5: Implement Mission 4 content exactly as the spec**

Use IDs:

```rust
pub const BULWARK: UnitId = UnitId(41);
pub const CONTROLLER: UnitId = UnitId(42);
pub const RIFLEMAN: UnitId = UnitId(43);
```

Build the exact board, opening rows, enemy roster/weapons, rules, rewards, and dialogue from the spec. Reuse only existing VN file paths.

Mission definition:

```text
Title: Mission 4 — Breach the Gate
Primary copy: Destroy the Gate Bulwark; escorts may be ignored.
Optional copy: Chain Reaction: damage any enemy with enemy fire, collision, hazard, or explosion.
600 + 150
Four -> Five
```

- [ ] **Step 6: Update Continue routing for authored Mission 4**

In `apply_campaign_action` change Continue routing to:

```rust
Ok(MissionId::Two | MissionId::Three | MissionId::Four) => {
    next_state.set(GameScreen::Upgrade)
}
Ok(MissionId::Five | MissionId::Six) => next_state.set(GameScreen::NextMission),
```

Task 4 will make Five authored and move only Six to the handoff branch.

Update comments that still describe Four as terminal.

- [ ] **Step 7: Add campaign tests for Three -> Four -> Five**

Exercise the real Mission 3 completion receipt into Mission 4, then Mission 4 completion into Five. Pin base-credit progression through Mission 4:

```text
300 + 400 + 500 + 600 = 1800 base credits
```

Keep upgrade projection assertions on Mission 4 construction.

- [ ] **Step 8: Run focused/full gates**

```bash
cargo fmt --check
cargo test --lib mission::mission_four
cargo test --test campaign_model
cargo test --test campaign_flow
cargo test --all-targets
```

- [ ] **Step 9: Commit Mission 4**

```bash
git add src/mission/mission_four.rs src/mission/mod.rs src/presentation/campaign_ui.rs tests/campaign_flow.rs tests/campaign_model.rs
git commit -m "feat: add Mission 4 environmental breach"
```

---

### Task 4: Author Mission 5 and the locked artillery crossfire manipulation

**Files:**
- Create: `src/mission/mission_five.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Test: `src/mission/mission_five.rs`
- Test: `tests/campaign_flow.rs`
- Test: `tests/campaign_model.rs`
- Test: `tests/campaign_persistence.rs`

**Interfaces:**
- Consumes: locked `AttackIntent` footprints, current `resolve_push`, `EliminateAllEnemies`, `VictoryByRound`.
- Produces: `MISSION_FIVE_DEFINITION`, `mission_five_for_campaign`, Five -> Six handoff.

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

Validate every opening destination/target, plus Controller alignment/range.

- [ ] **Step 2: Write the failing first-turn artillery-footprint test**

Drive the real opening:

```rust
let mut battle = mission_five(7);
battle.begin_round().unwrap();

let artillery_a = battle.intent_for(ids::ARTILLERY_A).unwrap();
let artillery_b = battle.intent_for(ids::ARTILLERY_B).unwrap();
assert!(artillery_a.footprint.contains(&GridPos::new(3, 7)));
assert!(artillery_b.footprint.contains(&GridPos::new(3, 7)));
assert_eq!(battle.unit(ids::BULWARK).unwrap().position, GridPos::new(3, 6));
```

Then execute the deterministic setup:

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

Use the existing test-only intent resolver to resolve Artillery A then B, and assert each event list contains `AttackRolled { target: BULWARK, .. }`. The test proves reuse of committed-footprint semantics, not a new friendly-fire rule.

- [ ] **Step 3: Add Round-4 bonus boundary tests**

Use the same durable round helper style as Mission 3. Pin:

```text
victory at round <= 4 -> optional_complete true
victory at round 5 -> optional_complete false
```

Do not set the result directly; knock out the remaining enemies through the battle test seam so `check_terminal_state` evaluates the real optional rule.

- [ ] **Step 4: Run Mission 5 tests and confirm the red state**

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

Use shared enemy factories/weapons. Do not add hazards, new props, or a mission-specific artillery behavior branch.

Definition:

```text
Title: Mission 5 — Crossfire Break
Primary copy: Break the assault and destroy all enemies.
Optional copy: Rapid Break: win by the end of Round 4.
700 + 200
Five -> Six
```

Use only the existing VN assets and exact dialogue in the spec.

- [ ] **Step 6: Register Mission 5 and make Six the only handoff**

In `mission_definition`:

```rust
MissionId::Five => Some(&mission_five::MISSION_FIVE_DEFINITION),
MissionId::Six => None,
```

Continue routing becomes:

```rust
Ok(MissionId::Two | MissionId::Three | MissionId::Four | MissionId::Five) => {
    next_state.set(GameScreen::Upgrade)
}
Ok(MissionId::Six) => next_state.set(GameScreen::NextMission),
```

`Proceed` remains definition-driven and needs no special Mission 5 branch.

- [ ] **Step 7: Extend campaign progression/persistence tests through Six**

Pin:

```text
One -> Two -> Three -> Four -> Five -> Six
Base rewards through Five = 2500
Max optional through Five = 700
Max total through Five = 3200
```

Persist an upgrade before Mission 4, reload, construct Mission 4/5 with that state, finish Mission 5, save/reload Six, and assert the upgrade levels/remaining credits are unchanged except for intended rewards/purchases.

- [ ] **Step 8: Run focused/full gates**

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
git add src/mission/mission_five.rs src/mission/mod.rs src/presentation/campaign_ui.rs tests/campaign_flow.rs tests/campaign_model.rs tests/campaign_persistence.rs
git commit -m "feat: add Mission 5 artillery assault"
```

---

### Task 5: Add Bulwark/Controller visuals to the existing glTF and presentation matches

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Test: `src/presentation/assets.rs`
- Test: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: `MissionAssets`, `scene_index`, current single embedded glTF buffer/accessors.
- Produces: scene 11 Bulwark, scene 12 Controller, scene count 13.

- [ ] **Step 1: Write the glTF structure test first**

Extend the JSON test to pin:

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

Also assert child nodes 57–62 use mesh 11 and 64–69 use mesh 12; both primitives reuse POSITION accessor 0 and NORMAL accessor 1.

Pin material names/colors:

```text
11 “Bulwark Ochre” [0.78,0.38,0.08,1.0]
12 “Controller Cyan” [0.08,0.72,0.86,1.0]
```

- [ ] **Step 2: Run the asset test and confirm red**

```bash
cargo test --lib presentation::assets::tests -- --nocapture
```

Expected: scene/node/mesh/material count assertions fail at the current 11/56/11/11 values.

- [ ] **Step 3: Append the two glTF scenes without creating a new buffer**

Append:

```text
Bulwark root node 56 + children 57–62 -> mesh 11
Controller root node 63 + children 64–69 -> mesh 12
```

For child transform data, copy the exact translation/rotation/scale values currently authored on Flanker children 50–55, changing only the mesh index to 11 or 12. Add mesh/material 11 and 12 using the same cube POSITION/NORMAL accessors already used by mesh 10. Keep the existing URI/buffer/accessor arrays unchanged.

- [ ] **Step 4: Wire presentation scene indices and exhaustive enemy matches**

Change:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 13;
```

Extend `scene_index`:

```rust
UnitArchetype::Bulwark => 11,
UnitArchetype::Controller => 12,
```

Extend UI enemy-only match arms so both new archetypes behave like all other non-player archetypes for pilot controls/labels. Do not add a scale table or Controller-specific HUD.

- [ ] **Step 5: Add presentation assertions**

Pin:

```rust
assert_eq!(scene_index(UnitArchetype::Bulwark), 11);
assert_eq!(scene_index(UnitArchetype::Controller), 12);
```

For Mission 4 `HudSnapshot`, assert Target tracker displays Gate Bulwark HP. For Mission 5, assert both Artillery intents appear in the threat list and the objective copy uses remaining-enemy count only because Mission 5 is `EliminateAllEnemies`.

- [ ] **Step 6: Run presentation/asset gates**

```bash
cargo fmt --check
cargo test --lib presentation::assets::tests
cargo test --test presentation_app
cargo test --all-targets
```

- [ ] **Step 7: Commit visuals/presentation**

```bash
git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs src/presentation/ui.rs tests/presentation_app.rs
git commit -m "feat: present Bulwark and Controller"
```

---

### Task 6: Prove renderer-free Mission 4–5 entry, restart, save, and upgrade continuity

**Files:**
- Modify if required by failing tests: `src/app.rs`
- Modify if required by failing tests: `src/presentation/mod.rs`
- Test: `tests/campaign_flow.rs`
- Test: `tests/campaign_model.rs`
- Test: `tests/campaign_persistence.rs`
- Test: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: definition-driven battle construction/restart, `CampaignSession`, `ActiveMission`, `MissionId` One–Six.
- Produces: evidence that no mission-specific app/session branch is required for Four/Five.

- [ ] **Step 1: Add integration coverage before changing app code**

Add tests that:

```text
1. start from persisted next_mission Four with a non-zero upgrade;
2. Continue routes to Upgrade;
3. Proceed routes to PreMissionStory because Four is authored;
4. Start Mission builds M4 with the upgrade projected;
5. restart rebuilds M4 through ActiveMission definition and keeps campaign upgrades;
6. M4 completion persists Five;
7. Continue/Proceed builds M5 with the same upgrade;
8. M5 completion persists Six;
9. Continue Six routes to NextMission.
```

Add a base-reward assertion for exactly 2500 credits before optional rewards/purchases.

- [ ] **Step 2: Run the integration tests**

```bash
cargo test --test campaign_flow -- --nocapture
cargo test --test campaign_persistence -- --nocapture
cargo test --test presentation_app -- --nocapture
```

Expected: if current definition-driven code is sufficient, these pass without production changes. If a test exposes an exhaustive MissionId match or hard-coded Four handoff, change only that concrete match.

- [ ] **Step 3: Apply only test-forced app/session corrections**

Allowed corrections are bounded to:

```text
- exhaustive MissionId routing now that Four/Five are authored;
- stale comments/copy calling Four the terminal handoff;
- mission-definition lookup where a hard-coded Three/Four branch remains.
```

Do not introduce a mission router, state-machine abstraction, campaign registry beyond the existing `mission_definition` match, or a new save version.

- [ ] **Step 4: Run the complete automated suite**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Expected: green.

- [ ] **Step 5: Commit only if Task 6 changed production/tests**

```bash
git add src/app.rs src/presentation/mod.rs tests/campaign_flow.rs tests/campaign_model.rs tests/campaign_persistence.rs tests/presentation_app.rs
git commit -m "test: cover Mission 4-5 campaign continuity"
```

Stage only files that actually changed.

---

### Task 7: Update docs, perform manual encounter validation, and close the evidence ledger

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Create: `docs/validation/hpa-523.md`

**Interfaces:**
- Consumes: final implementation SHA, automated test output, manual playthrough observations.
- Produces: reviewable HPA-523 acceptance evidence with no placeholders.

- [ ] **Step 1: Update product/developer docs to the current campaign**

README/CLAUDE must state:

```text
- campaign is authored through Mission 5 with Mission 6 handoff;
- final regular roster is Rifleman/Striker/Artillery/Flanker/Bulwark/Controller;
- Mission 4 is target-breach/environment manipulation;
- Mission 5 is locked-artillery crossfire exploitation;
- Bulwark has no displacement immunity;
- Controller is push-only, no status system;
- current save remains local JSON at stable campaign transitions.
```

Remove stale wording that says Mission 4 is the terminal handoff.

- [ ] **Step 2: Run the exact CI-equivalent gates and record outputs**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Also run:

```bash
cargo test --all-targets
```

Record the final test count and commit SHA in `docs/validation/hpa-523.md` after the implementation commit exists.

- [ ] **Step 3: Manually validate Mission 4**

Run:

```bash
cargo run
```

Verify and record:

```text
- Bulwark visually reads heavier than other regular enemies.
- Controller push telegraph remains legible.
- Gunner can use the (3,4) explosive to splash Bulwark.
- Vanguard can push Bulwark (4,4)->(4,3) onto the hazard.
- Bulwark KO ends the mission while escorts may remain.
- Turnabout/Chain Reaction reward appears when qualifying environmental/enemy damage occurred.
- Encounter remains a short tactical session.
```

- [ ] **Step 4: Manually validate Mission 5**

Verify and record:

```text
- both Artillery locked Cross1 footprints are readable;
- both include the shared (3,7) cell on the authored opening;
- Gunner can vacate (3,8) and Vanguard can vacate (4,7);
- Vanguard can push Bulwark (3,6)->(3,7);
- both already-committed Artillery attacks can damage Bulwark at (3,7);
- Controller's vacated (4,7) hit lands harmlessly;
- Rapid Break rewards a <= Round 4 win but is not a hard failure deadline;
- mixed telegraphs remain readable and the encounter stays short.
```

- [ ] **Step 5: Manually validate campaign continuity**

From a Mission 3 completion save, verify:

```text
M3 results -> upgrade -> M4 story/briefing/battle -> results -> upgrade
-> M5 story/briefing/battle -> results -> upgrade -> M6 handoff
```

Confirm Continue/restart preserve credits and upgrades across the new missions.

- [ ] **Step 6: Write the validation ledger with concrete evidence**

`docs/validation/hpa-523.md` must contain:

```text
- final implementation commit SHA;
- exact commands and pass/fail result;
- test count / coverage command result;
- Mission 4 automated geometry + manual evidence;
- Mission 5 dual-artillery footprint + displacement evidence;
- glTF scene/node/mesh/material/buffer count evidence;
- campaign One->Six / 2500-base-credit / save-upgrade evidence;
- any accepted tuning changes with their final authored values.
```

Do not leave `TBD`, pending checklist items, or speculative follow-ups in the ledger.

- [ ] **Step 7: Run final gates after documentation edits**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Expected: green at the PR head.

- [ ] **Step 8: Commit closeout docs/evidence**

```bash
git add README.md CLAUDE.md docs/validation/hpa-523.md
git commit -m "docs: validate HPA-523 missions 4-5"
```

---

## Final Review Gate

Before marking the PR ready for review, verify all of the following:

- [ ] HPA-523 is still one PR; planning and implementation share the same branch.
- [ ] Only one new primary objective exists: `EliminateTarget`.
- [ ] No displacement-resistance/status/AI/objective framework was added.
- [ ] Bulwark is HP16 / Armor4 / Move1, pushable, initiative15.
- [ ] Controller is HP9 / Armor1 / Move2 with range2–4 damage3 Push1, initiative35.
- [ ] Controller cannot commit diagonal push targets.
- [ ] Mission 4 preserves both authored environmental solutions and wins on Bulwark KO with escorts alive.
- [ ] Mission 5 opening preserves the shared `(3,7)` dual-Artillery footprint and the deterministic Bulwark displacement line.
- [ ] Mission 5's Round-4 condition is optional pressure, not a primary deadline.
- [ ] Regular roster totals exactly six archetypes.
- [ ] glTF final structure is 13 scenes / 70 nodes / 13 meshes / 13 materials / 1 buffer.
- [ ] One–Five are authored; Six is the only terminal handoff.
- [ ] Base rewards through Mission 5 total exactly 2500.
- [ ] Save, upgrades, Continue, restart, VN, briefing, results, and upgrade flow remain continuous through Mission 5.
- [ ] README/CLAUDE/validation ledger match the shipped behavior.
- [ ] fmt, strict Clippy, all-target tests/coverage, and release build are green.