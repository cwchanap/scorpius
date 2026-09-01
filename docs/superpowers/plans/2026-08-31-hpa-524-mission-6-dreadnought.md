# HPA-524 Mission 6 Dreadnought Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 6 and the first Dreadnought boss as one player-visible HPA-524 slice, with one half-HP behavior change on the existing locked-intent path and a persisted Mission 7 handoff.

**Architecture:** Add one concrete `Dreadnought` archetype and teach the existing `unit_weapon` selector to choose weapon slot 1 at/below half HP for that archetype only. `build_intent` reuses that selector, so movement and future intent commitment change together while already-committed intents remain immutable. Mission 6 owns the boss values, weapons, board, escorts, dialogue, rewards, and opening geometry. Campaign routing keeps Mission 1 special and otherwise derives authored-vs-handoff from `mission_definition`, matching the existing `Proceed` seam.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md`

## Global Constraints

- One HPA-524 ticket = one PR. Continue implementation on this planning branch/PR.
- Keep the boss on normal single-cell `UnitState`, normal damage/knockout, normal locked `AttackIntent`, normal push, and normal campaign/save flow.
- Add exactly one boss archetype: `UnitArchetype::Dreadnought`.
- Mission 6 owns the boss factory and two boss weapons locally; do not add a shared boss module or threshold framework before Mission 7.
- Threshold is exactly half max HP: max 40 means Graviton at 21–40 and Overload at 0–20.
- Threshold is derived, not stored. No boss phase field/event/registry.
- Crossing the threshold during the player phase never changes the current committed intent.
- Dreadnought remains pushable; no displacement resistance.
- No turn limit, new objective shape, optional-objective shape, status system, behavior policy, phase scripting, multi-tile collision, parts, invulnerability, or second runtime.
- Mission 6 uses a 9×9 board, existing blocking only, no hazards or explosives.
- Primary is `EliminateTarget { target: DREADNOUGHT }`; bonus is `Turnabout`.
- Rewards are 800 + 250; Mission 6 unlocks `MissionId::Seven`.
- Mission IDs become One–Seven; One–Six authored, Seven terminal handoff.
- `Continue`: One -> story; any other authored mission -> Upgrade; unauthored handoff -> NextMission. Do not enumerate Two–Six.
- Reuse existing VN assets only.
- Append one Dreadnought scene to `assets/models/mission_one.gltf`; final counts 14 scenes / 77 nodes / 14 meshes / 14 materials / 1 buffer.
- CI gates remain `cargo fmt --check`, strict Clippy, `cargo test --all-targets`, llvm-cov, and release build.

## Risks

- **Threshold/intent timing.** Crossing 20 HP after Graviton commits must not alter that intent; the next intent must use Overload.
- **Close-pressure identity.** At/below 20 HP and distance 5, the ordinary attack-band planner must move the boss one cell closer because Overload max range is 4.
- **Opening geometry.** Dreadnought Cross1 on `(4,7)` contains `(5,7)`; Vanguard can vacate `(4,7)` and Interceptor can push Controller `(6,7)->(5,7)` through public movement/push paths.
- **Task ordering.** Registering Mission 6 invalidates existing library pins and the hardcoded Continue match immediately; those edits belong in Task 2, not Task 3.
- **Asset blast radius.** Existing glTF tests pin old global counts, and Controller's current `.skip(64)` loop is unbounded. Task 4 must update those invariants with the new scene.

---

### Task 1: Add Dreadnought threshold behavior on the existing enemy path

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/interaction.rs`
- Test: `src/domain/enemy.rs`

**Interfaces:**
- Consumes: `UnitState.weapons`, `unit_weapon`, `build_intent`, `attack_band_destination`, `AttackIntent`, exhaustive archetype matches.
- Produces: `Dreadnought`; half-HP selection inside `unit_weapon`; `build_intent` using the same selector; initiative 40; ordinary attack-band movement; temporary scene mapping until Task 4.

- [ ] **Step 1: Write threshold fixtures and failing tests**

Add a normal threshold fixture with the boss at `(3,1)` and target at `(3,5)`:

```rust
const DREADNOUGHT: UnitId = UnitId(90);
const TEST_PLAYER: UnitId = UnitId(91);
const GRAVITON: WeaponId = WeaponId(290);
const OVERLOAD: WeaponId = WeaponId(291);

fn dreadnought_threshold_fixture() -> BattleState {
    let boss = squad::unit(
        DREADNOUGHT,
        "Dreadnought",
        UnitArchetype::Dreadnought,
        Faction::Enemy,
        squad::stats(40, 3, 1, 90, 5, 0),
        GridPos::new(3, 1),
        vec![GRAVITON, OVERLOAD],
    );
    let player = squad::unit(
        TEST_PLAYER,
        "Target",
        UnitArchetype::Vanguard,
        Faction::Player,
        squad::stats(20, 3, 3, 78, 5, 7),
        GridPos::new(3, 5),
        vec![],
    );
    let weapons = vec![
        squad::weapon(GRAVITON, "Graviton Salvo", 3, 6, WeaponShape::Cross1, 8, 10, 5, 0, false, false),
        squad::weapon(OVERLOAD, "Overload Salvo", 1, 4, WeaponShape::Cross1, 10, 10, 10, 0, false, false),
    ];
    BattleState::new(
        BoardState::new(7, 7, [], [], []),
        vec![boss, player],
        weapons,
        MissionRules {
            primary: PrimaryObjective::EliminateAllEnemies,
            optional: OptionalObjective::VictoryByRound { round: 9 },
            opening_plan: &[],
        },
        7,
    )
}
```

Pin the boundary and immutable intent:

```rust
#[test]
fn dreadnought_switches_weapon_once_at_half_hp() {
    let mut battle = dreadnought_threshold_fixture();
    assert_eq!(unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap()).unwrap().id, GRAVITON);
    battle.apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);
    assert_eq!(battle.unit(DREADNOUGHT).unwrap().hp, 20);
    assert_eq!(unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap()).unwrap().id, OVERLOAD);
    battle.apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
    assert_eq!(unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap()).unwrap().id, OVERLOAD);
}

#[test]
fn crossing_threshold_does_not_rewrite_committed_dreadnought_intent() {
    let mut battle = dreadnought_threshold_fixture();
    battle.begin_round().unwrap();
    let committed = battle.intent_for(DREADNOUGHT).unwrap().clone();
    assert_eq!(committed.profile.weapon, GRAVITON);

    battle.apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);
    assert_eq!(battle.intent_for(DREADNOUGHT).unwrap(), &committed);

    let future = build_intent(&battle, DREADNOUGHT, Some(GridPos::new(3, 5))).unwrap();
    assert_eq!(future.profile.weapon, OVERLOAD);
}
```

Add the lower-risk but product-visible movement proof with a separate fixture placing the boss at `(3,0)` and the target at `(3,5)`:

```rust
#[test]
fn dreadnought_overload_closes_from_range_five() {
    let mut battle = dreadnought_close_pressure_fixture();
    battle.apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);

    let destination = choose_enemy_destination(&battle, DREADNOUGHT).unwrap();

    assert_eq!(destination, GridPos::new(3, 1));
    assert_eq!(destination.manhattan(GridPos::new(3, 5)), 4);
}
```

`dreadnought_close_pressure_fixture()` uses the same stats/weapons as the threshold fixture, boss `(3,0)`, target `(3,5)`, and a 7×7 empty board.

- [ ] **Step 2: Run focused tests and confirm red**

```bash
cargo test --lib dreadnought -- --nocapture
```

Expected: compile failure because `Dreadnought` does not exist and slot selection is still fixed to the first weapon.

- [ ] **Step 3: Add the archetype and single weapon selector**

In `src/domain/model.rs`, append `Dreadnought`.

Replace `unit_weapon` with:

```rust
fn unit_weapon<'a>(battle: &'a BattleState, unit: &UnitState) -> Result<&'a WeaponSpec, BattleError> {
    let index = match unit.archetype {
        UnitArchetype::Dreadnought if unit.hp * 2 <= unit.stats.max_hp => 1,
        _ => 0,
    };
    let id = unit
        .weapons
        .get(index)
        .copied()
        .ok_or(BattleError::InvalidTarget(unit.position))?;
    battle.weapon(id).ok_or(BattleError::UnknownWeapon(id))
}
```

Do not add phase/threshold state to `UnitState`.

- [ ] **Step 4: Make `build_intent` reuse `unit_weapon`**

Replace the independent `.first()` lookup with:

```rust
let attacker = battle.unit(attacker_id).ok_or(BattleError::UnknownUnit(attacker_id))?;
let weapon = unit_weapon(battle, attacker)?;
let weapon_id = weapon.id;
```

Leave the remaining intent/profile snapshot logic unchanged.

- [ ] **Step 5: Add ordinary movement and initiative**

Group Dreadnought with the existing attack-band branch:

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Bulwark
| UnitArchetype::Dreadnought => {
    let weapon = unit_weapon(battle, unit)?;
    Ok(attack_band_destination(&candidates, &players, weapon))
}
```

Initiative:

```rust
UnitArchetype::Dreadnought => 40,
UnitArchetype::Controller => 35,
```

Keep all enemy matches exhaustive.

- [ ] **Step 6: Keep presentation/interaction exhaustive**

Temporarily map `Dreadnought => 11` in `battlefield::scene_index` until Task 4. Add Dreadnought to enemy-only branches in `ui.rs` and `interaction.rs`; no pilot command or boss HUD.

- [ ] **Step 7: Run gates and commit**

```bash
cargo fmt --check
cargo test --lib dreadnought
cargo test --all-targets
git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add dreadnought threshold behavior"
```

---

### Task 2: Author Mission 6, register it cleanly, and pin escort redirection

**Files:**
- Create: `src/mission/mission_six.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/mission/mission_two.rs`
- Modify: `src/mission/mission_three.rs`
- Modify: `src/mission/mission_four.rs`
- Modify: `src/mission/mission_five.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Test: `src/mission/mission_six.rs`
- Test: existing mission-local definition pins in Missions 2–5

**Interfaces:**
- Consumes: `build_player_squad`, local `squad::{unit, stats, weapon}`, regular enemy factories, `EliminateTarget`, `Turnabout`, `assert_opening_plan_is_legal`, Task 1 selector.
- Produces: authored `MISSION_SIX_DEFINITION`, Seven terminal handoff, data-driven Continue routing, exact 9×9 encounter, 800/250 rewards.

- [ ] **Step 1: Add failing Mission 6 authoring tests**

Pin:

```rust
let battle = mission_six(7);
assert_eq!((battle.board().width(), battle.board().height()), (9, 9));
assert_eq!(battle.board().blocking_cells().collect::<Vec<_>>(), vec![
    GridPos::new(2, 4), GridPos::new(6, 4), GridPos::new(2, 5), GridPos::new(6, 5),
]);
assert_eq!(battle.board().hazard_cells().count(), 0);
assert_eq!(battle.board().explosives().count(), 0);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.max_hp, 40);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.armor, 3);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.movement, 1);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().weapons, vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO]);
assert_eq!(battle.rules().primary, PrimaryObjective::EliminateTarget { target: ids::DREADNOUGHT });
assert_eq!(battle.rules().optional, OptionalObjective::Turnabout);
```

Also pin four enemies and exact opening rows.

- [ ] **Step 2: Confirm red**

```bash
cargo test --lib mission::mission_six -- --nocapture
```

- [ ] **Step 3: Add `MissionId::Seven`, register Six, and immediately fix the library blast radius**

In `src/mission/mod.rs`:

```rust
pub mod mission_six;

pub enum MissionId {
    One, Two, Three, Four, Five, Six, Seven,
}
```

Display Seven as `7`, and register:

```rust
MissionId::Six => Some(&mission_six::MISSION_SIX_DEFINITION),
MissionId::Seven => None,
```

In the existing mission-local definition tests, replace terminal assertions on Six with Seven:

```rust
assert!(mission_definition(MissionId::Seven).is_none());
```

Do this in `mission_two.rs`, `mission_three.rs`, `mission_four.rs`, and `mission_five.rs`. Mission 5 may additionally assert `mission_definition(MissionId::Six).is_some()`.

- [ ] **Step 4: Remove the hardcoded Continue mission list in the same compile step**

In `apply_campaign_action`:

```rust
CampaignUiAction::Continue => match continue_game(&mut runtime.0) {
    Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
    Ok(id) => next_state.set(if mission_definition(id).is_some() {
        GameScreen::Upgrade
    } else {
        GameScreen::NextMission
    }),
    Err(error) => status.0 = error.to_string(),
},
```

This intentionally matches the existing `Proceed` authored-data seam. Do not add a new routing helper/table. Update comments that call Six the terminal handoff.

- [ ] **Step 5: Implement the local boss and weapons**

In `mission_six.rs`:

```rust
pub mod ids {
    pub use crate::mission::squad::ids::{GUNNER, INTERCEPTOR, VANGUARD};
    use crate::domain::model::{UnitId, WeaponId};

    pub const DREADNOUGHT: UnitId = UnitId(61);
    pub const BULWARK: UnitId = UnitId(62);
    pub const CONTROLLER: UnitId = UnitId(63);
    pub const RIFLEMAN: UnitId = UnitId(64);
    pub const GRAVITON_SALVO: WeaponId = WeaponId(207);
    pub const OVERLOAD_SALVO: WeaponId = WeaponId(208);
}
```

Boss:

```rust
unit(
    ids::DREADNOUGHT,
    "Dreadnought",
    UnitArchetype::Dreadnought,
    Faction::Enemy,
    stats(40, 3, 1, 90, 5, 0),
    GridPos::new(4, 1),
    vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO],
)
```

Weapons:

```rust
weapon(ids::GRAVITON_SALVO, "Graviton Salvo", 3, 6, WeaponShape::Cross1, 8, 10, 5, 0, false, false)
weapon(ids::OVERLOAD_SALVO, "Overload Salvo", 1, 4, WeaponShape::Cross1, 10, 10, 10, 0, false, false)
```

Use existing `enemies::{bulwark, controller, rifleman}` for escorts. Do not add Dreadnought to shared regular factories.

- [ ] **Step 6: Author board, deployment, opening, rules, dialogue, and rewards**

Deployment:

```rust
SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
}
```

Opening:

```rust
static MISSION_SIX_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::DREADNOUGHT, destination: GridPos::new(4, 2), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::BULWARK, destination: GridPos::new(1, 7), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::CONTROLLER, destination: GridPos::new(6, 7), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(6, 6), target: Some(ids::INTERCEPTOR) },
];
```

Rules/definition:

```rust
const MISSION_SIX_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateTarget { target: ids::DREADNOUGHT },
    optional: OptionalObjective::Turnabout,
    opening_plan: &MISSION_SIX_OPENING,
};

pub const MISSION_SIX_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Six,
    unlocks: MissionId::Seven,
    build: mission_six_for_campaign,
    title: "Mission 6 — Break the Dreadnought",
    primary_objective: "Destroy the Dreadnought; escorts may be ignored.",
    optional_objective: "Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.",
    base_reward: 800,
    optional_reward: 250,
    pre_mission: DialogueScene { background: "vn/relay_nine_bg.png", lines: &PRE_MISSION_LINES },
    aftermath: DialogueScene { background: "vn/relay_nine_bg.png", lines: &AFTERMATH_LINES },
};
```

Use the spec's exact dialogue; no new assets.

- [ ] **Step 7: Pin opening legality and exact rows**

```rust
#[test]
fn mission_six_opening_rows_are_legal() {
    let battle = mission_six(7);
    assert_opening_plan_is_legal(&battle);
}
```

Also assert all four `(unit, destination, target)` tuples exactly.

- [ ] **Step 8: Drive the real public manipulation line and pin Turnabout**

Use the same public movement/push line for both geometry and objective coverage. First pin geometry with any stable seed: committed Graviton footprint contains `(5,7)`, Vanguard moves to `(4,5)`, Interceptor moves to `(7,7)`, Vector Pulse moves Controller `(6,7)->(5,7)`, and Controller's committed `(4,7)` footprint resolves empty.

For the bonus proof, use a deterministic seed sweep around that exact public line and require a run where the boss attack both rolls against Controller and emits `OptionalObjectiveCompleted` through normal `DamageSource::EnemyWeapon` handling:

```rust
#[test]
fn redirected_graviton_can_complete_turnabout() {
    let mut witnessed = false;

    for seed in 0..256 {
        let mut battle = mission_six(seed);
        battle.begin_round().unwrap();

        // Repeat the exact public Vanguard/Interceptor movement and Vector Pulse
        // displacement pinned by the geometry test, then resolve only the
        // already-committed Dreadnought intent.
        // ... exact calls are identical to the geometry test above ...

        let events = battle.resolve_intent_for_test(ids::DREADNOUGHT).unwrap();
        let rolled_on_controller = events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackRolled { attacker, target, .. }
                if *attacker == ids::DREADNOUGHT && *target == ids::CONTROLLER
        ));
        let completed = events
            .iter()
            .any(|event| matches!(event, BattleEvent::OptionalObjectiveCompleted));
        if rolled_on_controller && completed {
            witnessed = true;
            break;
        }
    }

    assert!(witnessed, "expected a deterministic seed to land redirected Graviton damage");
}
```

When implementing the test, do not leave the comment placeholder above: inline the exact public calls from the geometry test so the test is standalone. Do not add friendly-fire special cases.

- [ ] **Step 9: Prove target victory and normal boss displacement**

```rust
#[test]
fn dreadnought_ko_wins_with_escorts_alive() {
    let mut battle = mission_six(7);
    battle.apply_direct_damage(ids::DREADNOUGHT, 99, DamageSource::PlayerWeapon(squad::ids::RAIL_RIFLE));
    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(!battle.unit(ids::BULWARK).unwrap().is_knocked_out());
}
```

For displacement, place a player and Dreadnought in one row with the existing test helper, call `resolve_push`, and assert one-cell movement with no resistance path.

- [ ] **Step 10: Run library gates and commit**

```bash
cargo fmt --check
cargo test --lib mission::mission_six
cargo test --lib
git add src/mission/mod.rs src/mission/mission_six.rs src/mission/mission_two.rs src/mission/mission_three.rs src/mission/mission_four.rs src/mission/mission_five.rs src/presentation/campaign_ui.rs
git commit -m "feat: author Mission 6 Dreadnought encounter"
```

Expected: the whole library is green immediately after Six becomes authored; no non-exhaustive Continue match or stale `Six is None` pin remains.

---

### Task 3: Advance campaign/save integration through Mission 6

**Files:**
- Modify: `tests/campaign_model.rs`
- Modify: `tests/campaign_flow.rs`
- Modify: `tests/campaign_persistence.rs`

**Interfaces:**
- Consumes: `MISSION_SIX_DEFINITION`, `complete_current_mission`, data-driven Continue from Task 2, existing `Proceed` check.
- Produces: 3300 base-credit total through Mission 6, 250 optional reward coverage, persisted Seven handoff, Six-authored/Seven-terminal route assertions, story/briefing coverage.

- [ ] **Step 1: Update campaign model expectations for authored Six**

In `tests/campaign_model.rs`:

```rust
let six = mission_definition(MissionId::Six).unwrap();
assert_eq!(six.id, MissionId::Six);
assert_eq!(six.unlocks, MissionId::Seven);
assert_eq!(six.title, "Mission 6 — Break the Dreadnought");
assert_eq!((six.base_reward, six.optional_reward), (800, 250));
assert_eq!(mission_definition(MissionId::Seven), None);
```

Extend the base-reward sum through Six and assert `3300`.

- [ ] **Step 2: Extend progression with the real completion API**

Use `complete_current_mission`, not a nonexistent `persist_completion`:

```rust
let receipt = complete_current_mission(
    &mut session,
    mission_definition(MissionId::Six).unwrap(),
    MissionResult {
        victory: true,
        optional_complete: false,
        rounds: 4,
    },
)
.unwrap();
assert_eq!((receipt.base_reward, receipt.optional_reward), (800, 0));
assert_eq!(session.state.as_ref().unwrap().next_mission, MissionId::Seven);
assert_eq!(session.state.as_ref().unwrap().credits, 3300);
```

Add a focused fresh-session completion with `optional_complete: true` and assert optional reward 250 / total Mission 6 reward 1050.

- [ ] **Step 3: Move existing Continue terminal assertions from Six to Seven**

Update the existing flow that currently says `Continue at Six -> NextMission`:

```rust
assert_eq!(pending(&next), Some(GameScreen::Upgrade));
```

Update the reusable route assertions:

```rust
assert_eq!(route_continue(MissionId::Six), Some(GameScreen::Upgrade));
assert_eq!(route_continue(MissionId::Seven), Some(GameScreen::NextMission));
```

Do not create a new routing helper in production.

- [ ] **Step 4: Move Proceed handoff assertions from Six to Seven**

Existing `Proceed` at Six now must open Mission 6 story:

```rust
assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));
```

Add the same fixture with `next_mission: MissionId::Seven` and assert `NextMission`.

- [ ] **Step 5: Pin Seven save round-trip and upgrade continuity**

In `tests/campaign_persistence.rs`:

```rust
let state = CampaignState {
    next_mission: MissionId::Seven,
    credits: 1234,
    upgrades: SquadUpgrades {
        vanguard: UpgradeLevels { hp: 1, armor: 2, mobility: 0, weapon: 3 },
        gunner: UpgradeLevels::default(),
        interceptor: UpgradeLevels { hp: 2, armor: 1, mobility: 1, weapon: 2 },
    },
};
save.store(&state).unwrap();
assert_eq!(save.load().unwrap(), Some(state));
```

No migration/version layer.

- [ ] **Step 6: Pin Mission 6 presentation through public helpers**

```rust
let definition = mission_definition(MissionId::Six).unwrap();
let copy = briefing_copy(definition);
assert!(copy.contains("Mission 6 — Break the Dreadnought"));
assert!(copy.contains("800 credits"));
assert!(copy.contains("+250 credits"));
assert_eq!(dialogue_snapshot(&definition.pre_mission, DialogueCursor(0)).speaker, "Control");
assert_eq!(dialogue_snapshot(&definition.aftermath, DialogueCursor(1)).speaker, "Control");
```

- [ ] **Step 7: Run all campaign targets and commit**

```bash
cargo fmt --check
cargo test --test campaign_model
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --all-targets
git add tests/campaign_model.rs tests/campaign_flow.rs tests/campaign_persistence.rs
git commit -m "test: cover Mission 6 campaign continuity"
```

---

### Task 4: Append the Dreadnought visual and update existing glTF invariants

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`

**Interfaces:**
- Consumes: single-buffer glTF, Bulwark root/parts pattern, `MISSION_ONE_SCENE_COUNT`, `scene_index`.
- Produces: scene 13 Dreadnought; 14/77/14/14/1 counts; bounded Controller assertion; permanent `Dreadnought -> 13` mapping.

- [ ] **Step 1: Update the existing asset tests before adding the scene**

In `flanker_scene_is_authored_with_own_mesh_material_and_root_scale`, change global counts to the final values while keeping all Flanker indices unchanged:

```rust
assert_eq!(scenes.len(), 14);
assert_eq!(nodes.len(), 77);
assert_eq!(meshes.len(), 14);
assert_eq!(materials.len(), 14);
```

In `bulwark_and_controller_scenes_are_authored_with_own_meshes_and_roots`, make the same global count changes and fix the existing unbounded Controller loop:

```rust
for (index, part) in nodes.iter().enumerate().skip(64).take(6) {
    assert_eq!(part["mesh"], 12, "node {index} must use mesh 12");
}
```

Do not change existing scene/root/mesh/material indices for Flanker, Bulwark, or Controller.

- [ ] **Step 2: Add the failing Dreadnought structure assertions**

```rust
#[test]
fn dreadnought_scene_is_authored_as_a_larger_crimson_unit() {
    let gltf = mission_gltf();
    let scenes = gltf["scenes"].as_array().unwrap();
    let nodes = gltf["nodes"].as_array().unwrap();
    let meshes = gltf["meshes"].as_array().unwrap();
    let materials = gltf["materials"].as_array().unwrap();

    assert_eq!(scenes.len(), 14);
    assert_eq!(scenes[13]["name"], "Dreadnought");
    assert_eq!(scenes[13]["nodes"], serde_json::json!([70]));
    assert_eq!(nodes.len(), 77);
    assert_eq!(nodes[70]["scale"], serde_json::json!([1.12, 1.12, 1.12]));
    assert_eq!(nodes[70]["children"], serde_json::json!([71, 72, 73, 74, 75, 76]));
    for (index, part) in nodes.iter().enumerate().skip(71).take(6) {
        assert_eq!(part["mesh"], 13, "node {index} must use mesh 13");
    }
    assert_eq!(meshes.len(), 14);
    assert_eq!(meshes[13]["name"], "Dreadnought Crimson");
    assert_eq!(meshes[13]["primitives"][0]["material"], 13);
    assert_eq!(materials.len(), 14);
    assert_eq!(materials[13]["name"], "Dreadnought Crimson");
    assert_eq!(
        materials[13]["pbrMetallicRoughness"]["baseColorFactor"],
        serde_json::json!([0.55, 0.08, 0.12, 1.0])
    );
    assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 3: Confirm red**

```bash
cargo test --lib presentation::assets::tests -- --nocapture
```

Expected: final count/scene assertions fail until glTF is appended.

- [ ] **Step 4: Append scene/root/parts/mesh/material without buffer changes**

In `assets/models/mission_one.gltf`:

- append scene `{ "name": "Dreadnought", "nodes": [70] }`;
- copy Bulwark's six-part transform structure into root 70 / parts 71–76;
- root name `Dreadnought Root`, children `[71,72,73,74,75,76]`, scale `[1.12,1.12,1.12]`;
- rename parts with `Dreadnought` prefixes and set each `mesh` to 13;
- append mesh 13 `Dreadnought Crimson`, reusing existing POSITION/NORMAL accessors and material 13;
- append material 13 `Dreadnought Crimson` with `[0.55,0.08,0.12,1.0]` and the same metallic/roughness shape as existing unit materials;
- do not add or change buffer/accessor binary data.

- [ ] **Step 5: Update loading/mapping and run gates**

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 14;
```

Change temporary mapping to:

```rust
UnitArchetype::Dreadnought => 13,
```

Run:

```bash
python -m json.tool assets/models/mission_one.gltf >/dev/null
cargo fmt --check
cargo test --lib presentation::assets::tests
cargo test --test presentation_app
cargo test --all-targets
```

- [ ] **Step 6: Commit**

```bash
git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: present the Dreadnought boss"
```

---

### Task 5: Close HPA-524 with validation, docs, and full gates

**Files:**
- Create: `docs/validation/hpa-524.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify spec/plan only if authored tuning changes locked values.

**Interfaces:**
- Consumes: completed Mission 6 implementation and existing validation style.
- Produces: reproducible automated/manual evidence and truthful shipped docs.

- [ ] **Step 1: Update shipped product facts only**

Document:

```text
- authored campaign runs Missions 1–6 and hands off to Mission 7
- regular roster remains six archetypes
- Mission 6 adds one single-cell Dreadnought boss
- Graviton above half HP; Overload at/below half HP
- threshold affects future planning only; committed intents stay locked
- boss remains pushable
- save/upgrade flow advances through Mission 6
```

Do not document Mission 7 content, generic boss systems, or resistance as shipped.

- [ ] **Step 2: Create the validation ledger and run automated gates**

Record final output summaries for:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Record final test count.

- [ ] **Step 3: Perform the real Mission 6 campaign playthrough**

Run `cargo run` and verify:

1. saved Mission 5 completion reaches the Mission 6 upgrade/story flow;
2. opening Graviton Cross1 is readable;
3. Vanguard vacates `(4,7)` and Interceptor can push Controller to `(5,7)`;
4. redirected boss fire can complete Turnabout;
5. crossing 21+ -> <=20 after commitment does not rewrite the current telegraph;
6. next planning commits Overload and closes distance when outside range 4;
7. Dreadnought can still be pushed normally;
8. Dreadnought defeat with escorts alive wins immediately;
9. aftermath/reward/upgrade persist Seven; return to title and Continue shows the Mission 7 handoff;
10. record encounter duration and any authored-value tuning.

If pacing is wrong, tune only Mission 6 HP/damage/opening positions and update the locked docs in this PR. Do not add systems as a tuning response.

- [ ] **Step 4: Re-run full gates after manual tuning**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

- [ ] **Step 5: Self-review scope**

Verify:

```text
- exactly one new boss archetype
- no threshold/phase registry or stored boss phase
- no new objective/optional shape
- no displacement resistance
- no Mission 7 content
- target victory works with escorts alive
- current intent stays immutable across threshold crossing
- Overload close-pressure move is covered
- redirected boss fire covers Turnabout
- Six authored; Seven terminal
- Continue is data-driven after Mission 1
- old glTF tests updated and Controller loop bounded
- spec/plan match final tuned implementation
- one ticket / one PR preserved
```

- [ ] **Step 6: Commit closeout evidence**

```bash
git add README.md CLAUDE.md docs/validation/hpa-524.md docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md docs/superpowers/plans/2026-08-31-hpa-524-mission-6-dreadnought.md
git commit -m "docs: validate HPA-524 Mission 6"
```

- [ ] **Step 7: Keep implementation in this same PR**

Do not open a second implementation PR. Mark this draft ready only after the final gates and manual ledger are complete.