# HPA-524 Mission 6 Dreadnought Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 6 and the first Dreadnought boss as one player-visible HPA-524 slice, with one half-HP behavior change on the existing locked-intent path and a persisted Mission 7 handoff.

**Architecture:** Add one concrete `Dreadnought` archetype and teach the existing `unit_weapon` selector to choose weapon slot 1 at/below half HP for that archetype only. `build_intent` reuses that selector, so movement and future intent commitment change together while already-committed intents remain immutable. Mission 6 owns the boss values, two weapons, board, escorts, dialogue, rewards, and first-round geometry; campaign/presentation reuse existing paths with Six becoming authored and Seven becoming the terminal handoff.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md`

## Global Constraints

- One HPA-524 ticket = one PR. Continue implementation on this planning branch/PR.
- Keep the boss on normal single-cell `UnitState`, normal damage/knockout, normal locked `AttackIntent`, normal push, and normal campaign/save flow.
- Add exactly one boss archetype here: `UnitArchetype::Dreadnought`.
- Mission 6 owns the boss factory and its two weapons locally; do not add a shared boss module or threshold data framework before Mission 7.
- Threshold is exactly half authored max HP: with max HP 40, Graviton Salvo applies at 21–40 HP and Overload Salvo at 0–20 HP.
- The threshold is derived, not stored. No boss phase field/event/registry is added.
- Crossing the threshold during the player phase never changes the already-committed current-round intent. Only a newly built future intent uses the new weapon.
- Dreadnought remains pushable. Do not add displacement resistance unless manual validation proves authored tuning cannot work without it.
- Do not add a primary turn limit, objective variant, optional-objective variant, status system, behavior tree/policy object, phase scripting, multi-tile collision, boss parts, invulnerability, or second battle runtime.
- Mission 6 uses a 9×9 board, existing blocking vocabulary only, no hazards, and no explosive props.
- Mission 6 primary is `EliminateTarget { target: DREADNOUGHT }`; bonus is existing `Turnabout`.
- Mission 6 rewards are 800 base + 250 optional and unlock `MissionId::Seven`.
- Mission IDs become One–Seven; One–Six are authored and Seven is the HPA-524 terminal handoff.
- Reuse existing VN assets only; add no VN files.
- Append one Dreadnought scene to `assets/models/mission_one.gltf`; no second glTF, texture, animation, generator, or runtime asset pipeline.
- Final asset counts are 14 scenes, 77 nodes, 14 meshes, 14 materials, 1 buffer.
- CI gates remain `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo llvm-cov --all-targets --lcov --output-path lcov.info`, and `cargo build --release`.

## Risks

- **Threshold/intent timing — highest risk.** The boss can cross 20 HP after committing Graviton Salvo. Current-round `AttackIntent` must remain unchanged; only the next call to `build_intent` may select Overload Salvo. Task 1 pins both sides of this contract before Mission 6 authoring.
- **Opening geometry is load-bearing.** The Dreadnought Cross1 committed on Vanguard `(4,7)` must contain `(5,7)`, Vanguard `(4,7)->(4,5)` and Interceptor `(5,8)->(7,7)` must be legal public movement paths, and Controller `(6,7)->(5,7)` must be a legal leftward Vector Pulse displacement. Task 2 drives the real opening and public movement/push geometry.
- **Old terminal routing is explicit.** `campaign_ui.rs` currently sends saved Mission 6 directly to `NextMission`. Task 3 must move Six into the authored resume group and make Seven the new handoff without adding a new `GameScreen`.

---

### Task 1: Add the Dreadnought threshold behavior on the existing enemy path

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/interaction.rs`
- Test: `src/domain/enemy.rs`

**Interfaces:**
- Consumes: `UnitState.weapons`, `unit_weapon`, `build_intent`, `attack_band_destination`, `AttackIntent`, exhaustive archetype matches.
- Produces: `UnitArchetype::Dreadnought`; half-HP slot selection inside `unit_weapon`; `build_intent` using the same selector; initiative 40; temporary scene 11 mapping until Task 4.

- [ ] **Step 1: Write the failing threshold and locked-intent tests**

In `src/domain/enemy.rs` tests add constants and a tiny boss fixture:

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
        squad::weapon(
            GRAVITON,
            "Graviton Salvo",
            3,
            6,
            WeaponShape::Cross1,
            8,
            10,
            5,
            0,
            false,
            false,
        ),
        squad::weapon(
            OVERLOAD,
            "Overload Salvo",
            1,
            4,
            WeaponShape::Cross1,
            10,
            10,
            10,
            0,
            false,
            false,
        ),
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

Add:

```rust
#[test]
fn dreadnought_switches_weapon_once_at_half_hp() {
    let mut battle = dreadnought_threshold_fixture();
    assert_eq!(unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap()).unwrap().id, GRAVITON);

    battle
        .apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);
    assert_eq!(battle.unit(DREADNOUGHT).unwrap().hp, 20);
    assert_eq!(unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap()).unwrap().id, OVERLOAD);

    battle
        .apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
    assert_eq!(unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap()).unwrap().id, OVERLOAD);
}

#[test]
fn crossing_threshold_does_not_rewrite_committed_dreadnought_intent() {
    let mut battle = dreadnought_threshold_fixture();
    battle.begin_round().unwrap();
    let committed = battle.intent_for(DREADNOUGHT).unwrap().clone();
    assert_eq!(committed.profile.weapon, GRAVITON);

    battle
        .apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);

    assert_eq!(battle.intent_for(DREADNOUGHT).unwrap(), &committed);
    let future = build_intent(&battle, DREADNOUGHT, Some(GridPos::new(3, 5))).unwrap();
    assert_eq!(future.profile.weapon, OVERLOAD);
}
```

- [ ] **Step 2: Run the focused tests and confirm red**

```bash
cargo test --lib dreadnought -- --nocapture
```

Expected: compile failure because `UnitArchetype::Dreadnought` does not exist and `unit_weapon` always selects slot 0.

- [ ] **Step 3: Add the concrete archetype and one-way weapon selector**

In `src/domain/model.rs`, append:

```rust
Dreadnought,
```

Replace `unit_weapon` in `src/domain/enemy.rs` with:

```rust
fn unit_weapon<'a>(
    battle: &'a BattleState,
    unit: &UnitState,
) -> Result<&'a WeaponSpec, BattleError> {
    let weapon_index = match unit.archetype {
        UnitArchetype::Dreadnought if unit.hp * 2 <= unit.stats.max_hp => 1,
        _ => 0,
    };
    let weapon_id = unit
        .weapons
        .get(weapon_index)
        .copied()
        .ok_or(BattleError::InvalidTarget(unit.position))?;
    battle
        .weapon(weapon_id)
        .ok_or(BattleError::UnknownWeapon(weapon_id))
}
```

Do not add phase state or threshold data to `UnitState`.

- [ ] **Step 4: Make intent construction use the same selected weapon**

At the start of `build_intent`, replace the direct first-weapon lookup with:

```rust
let attacker = battle
    .unit(attacker_id)
    .ok_or(BattleError::UnknownUnit(attacker_id))?;
let weapon = unit_weapon(battle, attacker)?;
let weapon_id = weapon.id;
```

Leave the remainder of `AttackProfile` construction unchanged so the selected weapon is snapshotted into the immutable intent.

- [ ] **Step 5: Give Dreadnought ordinary attack-band movement and initiative 40**

In `choose_enemy_destination`:

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Bulwark
| UnitArchetype::Dreadnought => {
    let weapon = unit_weapon(battle, unit)?;
    Ok(attack_band_destination(&candidates, &players, weapon))
}
```

In `initiative`:

```rust
UnitArchetype::Dreadnought => 40,
UnitArchetype::Controller => 35,
```

Keep every enemy archetype explicit.

- [ ] **Step 6: Keep presentation/interaction exhaustive while the real boss scene is not authored yet**

In `src/presentation/battlefield.rs`, temporarily compile with:

```rust
UnitArchetype::Dreadnought => 11,
```

Task 4 changes it to scene 13.

Where `ui.rs` and `interaction.rs` enumerate enemy archetypes for pilot-skill availability/errors, include Dreadnought with Rifleman/Striker/Artillery/Flanker/Bulwark/Controller. Do not give the boss a pilot command or special HUD.

- [ ] **Step 7: Run focused and all-target gates**

```bash
cargo fmt --check
cargo test --lib dreadnought
cargo test --all-targets
```

Expected: threshold tests pass; existing regular enemies still use their first weapon and all exhaustive matches compile.

- [ ] **Step 8: Commit the threshold slice**

```bash
git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add dreadnought threshold behavior"
```

---

### Task 2: Author Mission 6 and its escort-manipulation geometry

**Files:**
- Create: `src/mission/mission_six.rs`
- Modify: `src/mission/mod.rs`
- Test: `src/mission/mission_six.rs`

**Interfaces:**
- Consumes: `build_player_squad`, `squad::{unit, stats, weapon}`, regular enemy factories, `MissionRules`, `EliminateTarget`, `Turnabout`, `assert_opening_plan_is_legal`, threshold selector from Task 1.
- Produces: `MISSION_SIX_DEFINITION`, Mission IDs Seven/handoff, boss IDs 61/207/208, exact 9×9 encounter, 800/250 rewards.

- [ ] **Step 1: Add the failing Mission 6 authoring tests**

Create `src/mission/mission_six.rs` with the test module first. Pin:

```rust
assert_eq!(battle.board().width(), 9);
assert_eq!(battle.board().height(), 9);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.max_hp, 40);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.armor, 3);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.movement, 1);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().weapons, vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO]);
assert_eq!(battle.rules().primary, PrimaryObjective::EliminateTarget { target: ids::DREADNOUGHT });
assert_eq!(battle.rules().optional, OptionalObjective::Turnabout);
```

Also assert exact blocking cells `(2,4) (6,4) (2,5) (6,5)`, no hazards/explosives, exact four-enemy roster, and exact opening rows.

- [ ] **Step 2: Run the Mission 6 test target and confirm red**

```bash
cargo test --lib mission::mission_six -- --nocapture
```

Expected: compile failure because the module/definition/IDs do not exist.

- [ ] **Step 3: Add MissionId Seven and register Mission 6**

In `src/mission/mod.rs`:

```rust
pub mod mission_six;
```

Extend `MissionId` and display:

```rust
Six,
Seven,
```

Register:

```rust
MissionId::Six => Some(&mission_six::MISSION_SIX_DEFINITION),
MissionId::Seven => None,
```

Do not change `MissionDefinition` or campaign persistence structures.

- [ ] **Step 4: Implement the local Dreadnought factory and exact weapons**

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

Create the boss with:

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

Author the weapons exactly:

```rust
weapon(ids::GRAVITON_SALVO, "Graviton Salvo", 3, 6, WeaponShape::Cross1, 8, 10, 5, 0, false, false)
weapon(ids::OVERLOAD_SALVO, "Overload Salvo", 1, 4, WeaponShape::Cross1, 10, 10, 10, 0, false, false)
```

Use existing `enemies::{bulwark, controller, rifleman}` and their weapon factories for escorts.

- [ ] **Step 5: Author the board, deployment, opening, rules, dialogue, and rewards**

Use player deployment:

```rust
SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
}
```

Opening rows:

```rust
static MISSION_SIX_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::DREADNOUGHT, destination: GridPos::new(4, 2), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::BULWARK, destination: GridPos::new(1, 7), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::CONTROLLER, destination: GridPos::new(6, 7), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(6, 6), target: Some(ids::INTERCEPTOR) },
];
```

Rules:

```rust
const MISSION_SIX_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateTarget { target: ids::DREADNOUGHT },
    optional: OptionalObjective::Turnabout,
    opening_plan: &MISSION_SIX_OPENING,
};
```

Definition:

```rust
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

Use the exact dialogue from the spec; add no assets.

- [ ] **Step 6: Pin the authored opening with the shared validator**

Add:

```rust
#[test]
fn mission_six_opening_rows_are_legal() {
    let battle = mission_six(7);
    assert_opening_plan_is_legal(&battle);
}
```

Also pin all four `(unit, destination, target)` tuples exactly so the generic validator cannot hide authoring drift.

- [ ] **Step 7: Drive the real opening manipulation geometry**

Add a test that:

```rust
let mut battle = mission_six(7);
battle.begin_round().unwrap();
let boss_intent = battle.intent_for(ids::DREADNOUGHT).unwrap().clone();
assert_eq!(boss_intent.profile.weapon, ids::GRAVITON_SALVO);
assert!(boss_intent.footprint.contains(&GridPos::new(5, 7)));

battle.begin_activation(ids::VANGUARD).unwrap();
battle.move_unit(ids::VANGUARD, GridPos::new(4, 5)).unwrap();
battle.choose_reaction(ids::VANGUARD, Reaction::Guard).unwrap();
battle.finish_activation(ids::VANGUARD).unwrap();

battle.begin_activation(ids::INTERCEPTOR).unwrap();
battle.move_unit(ids::INTERCEPTOR, GridPos::new(7, 7)).unwrap();
let preview = battle
    .preview_attack(ids::INTERCEPTOR, squad::ids::VECTOR_PULSE, GridPos::new(6, 7))
    .unwrap();
assert_eq!(preview.push_destination, Some(GridPos::new(5, 7)));
battle.resolve_push(ids::INTERCEPTOR, ids::CONTROLLER).unwrap();
assert_eq!(battle.unit(ids::CONTROLLER).unwrap().position, GridPos::new(5, 7));

let boss_events = battle.resolve_intent_for_test(ids::DREADNOUGHT).unwrap();
assert!(boss_events.iter().any(|event| matches!(
    event,
    BattleEvent::AttackRolled { attacker, target, .. }
        if *attacker == ids::DREADNOUGHT && *target == ids::CONTROLLER
)));
```

Also assert Controller's committed center remains Vanguard's old `(4,7)` footprint and resolves into empty space after Vanguard moves. Do not require either RNG roll to hit.

- [ ] **Step 8: Prove target-only victory and normal boss displacement**

Add:

```rust
#[test]
fn dreadnought_ko_wins_with_escorts_alive() {
    let mut battle = mission_six(7);
    battle.apply_direct_damage(
        ids::DREADNOUGHT,
        99,
        DamageSource::PlayerWeapon(squad::ids::RAIL_RIFLE),
    );
    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(!battle.unit(ids::BULWARK).unwrap().is_knocked_out());
}
```

For displacement, place Vanguard/Dreadnought in one row with `move_unit_direct_for_test`, call `resolve_push`, and assert Dreadnought moves one cell plus no new resistance event/error exists.

- [ ] **Step 9: Run Mission 6 and full library tests**

```bash
cargo fmt --check
cargo test --lib mission::mission_six
cargo test --lib
```

Expected: Mission 6 authoring, opening geometry, target victory, and normal push behavior all pass.

- [ ] **Step 10: Commit Mission 6**

```bash
git add src/mission/mod.rs src/mission/mission_six.rs
git commit -m "feat: author Mission 6 Dreadnought encounter"
```

---

### Task 3: Advance campaign/save/Continue through Mission 6 to the Mission 7 handoff

**Files:**
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `tests/campaign_flow.rs`
- Modify: `tests/campaign_persistence.rs`
- Test: `tests/campaign_flow.rs`
- Test: `tests/campaign_persistence.rs`

**Interfaces:**
- Consumes: `MISSION_SIX_DEFINITION`, `CampaignState::complete_mission`, `continue_game`, `mission_definition`, existing `Proceed` authored-vs-handoff check.
- Produces: Six resumes as authored; Seven is terminal; Mission 6 rewards/persistence/upgrade continuity are integration-tested.

- [ ] **Step 1: Extend the current red campaign progression test through Mission 6**

In the existing integration test that currently ends after Mission 5 with 2500 base credits and `MissionId::Six`, continue:

```rust
let receipt = persist_completion(
    &mut session,
    mission_definition(MissionId::Six).unwrap(),
    victory(false),
)
.unwrap();
assert_eq!((receipt.base_reward, receipt.optional_reward), (800, 0));
assert_eq!(session.state.as_ref().unwrap().next_mission, MissionId::Seven);
assert_eq!(session.state.as_ref().unwrap().credits, 3300);
```

Add a second focused completion with `victory(true)` or the existing helper's optional flag set, and assert the optional reward is 250.

- [ ] **Step 2: Add saved-Continue routing cases for Six and Seven before changing production routing**

Extend `title_continue_routes_by_the_saved_next_mission`:

```rust
// Six is now authored and resumes at Upgrade so the player may spend credits first.
store_state(MissionId::Six);
apply_campaign_action(CampaignUiAction::Continue, ...);
assert_eq!(pending(&next), Some(GameScreen::Upgrade));

// Seven is HPA-524's terminal handoff.
store_state(MissionId::Seven);
apply_campaign_action(CampaignUiAction::Continue, ...);
assert_eq!(pending(&next), Some(GameScreen::NextMission));
```

Use the test's existing save/session construction style; do not add a second routing helper solely for these cases.

- [ ] **Step 3: Run the campaign-flow test and confirm the Six route is red**

```bash
cargo test --test campaign_flow title_continue_routes_by_the_saved_next_mission -- --nocapture
```

Expected: saved Six still routes to `NextMission` under the current production match.

- [ ] **Step 4: Move the hardcoded terminal routing from Six to Seven**

In `apply_campaign_action`:

```rust
Ok(MissionId::Two
    | MissionId::Three
    | MissionId::Four
    | MissionId::Five
    | MissionId::Six) => next_state.set(GameScreen::Upgrade),
Ok(MissionId::Seven) => next_state.set(GameScreen::NextMission),
```

Update comments/doc comments that call Six the terminal handoff. Leave `CampaignUiAction::Proceed` unchanged: it already uses `mission_definition(state.next_mission).is_some()` and therefore sends Six into pre-mission story and Seven to `NextMission` automatically.

- [ ] **Step 5: Pin persisted MissionId Seven and upgrade continuity**

In `tests/campaign_persistence.rs` add a normal save round-trip:

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

This is a normal schema update only; add no migration/version conversion.

- [ ] **Step 6: Pin Mission 6 story/briefing/aftermath through public presentation helpers**

In `tests/campaign_flow.rs`, use `mission_definition(MissionId::Six).unwrap()` and assert:

```rust
assert!(briefing_copy(definition).contains("Mission 6 — Break the Dreadnought"));
assert!(briefing_copy(definition).contains("800 credits"));
assert!(briefing_copy(definition).contains("+250 credits"));
assert_eq!(dialogue_snapshot(&definition.pre_mission, DialogueCursor(0)).speaker, "Control");
assert_eq!(dialogue_snapshot(&definition.aftermath, DialogueCursor(1)).speaker, "Control");
```

No new UI widget or boss phase banner is introduced.

- [ ] **Step 7: Run campaign/save/all-target tests**

```bash
cargo fmt --check
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --all-targets
```

Expected: Mission 1–6 progression remains stable; saved Six resumes to Upgrade; Seven stays a handoff; upgrades/credits round-trip.

- [ ] **Step 8: Commit campaign continuity**

```bash
git add src/presentation/campaign_ui.rs tests/campaign_flow.rs tests/campaign_persistence.rs
git commit -m "feat: advance campaign through Mission 6"
```

---

### Task 4: Give the Dreadnought one distinct checked-in visual

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Test: `src/presentation/assets.rs`

**Interfaces:**
- Consumes: existing one-buffer glTF, scene 11 Bulwark root/parts as a structural template, `MISSION_ONE_SCENE_COUNT`, `scene_index`.
- Produces: scene 13 Dreadnought; counts 14/77/14/14/1; permanent `Dreadnought -> 13` mapping.

- [ ] **Step 1: Write the failing asset-structure test**

Add to `src/presentation/assets.rs`:

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

- [ ] **Step 2: Run the asset test and confirm red**

```bash
cargo test --lib dreadnought_scene_is_authored -- --nocapture
```

Expected: scene count is still 13 and scene 13 does not exist.

- [ ] **Step 3: Append the exact Dreadnought glTF entries without touching the buffer**

In `assets/models/mission_one.gltf`:

- append scene `{ "name": "Dreadnought", "nodes": [70] }`;
- duplicate Bulwark root/part transform structure from nodes 56–62 into nodes 70–76;
- set node 70 name to `Dreadnought Root`, children to `[71,72,73,74,75,76]`, and scale to `[1.12,1.12,1.12]`;
- keep the six copied part translations/scales/rotations unchanged, rename them with `Dreadnought` prefixes, and set every part's `mesh` to `13`;
- append mesh 13 named `Dreadnought Crimson`, using the same POSITION/NORMAL accessors as existing cube meshes and `material: 13`;
- append material 13 named `Dreadnought Crimson` with `baseColorFactor: [0.55,0.08,0.12,1.0]`; keep the same metallic/roughness shape used by the other unit materials;
- do not add or modify buffer/accessor binary data.

- [ ] **Step 4: Update Bevy scene loading and permanent archetype mapping**

In `src/presentation/assets.rs`:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 14;
```

In `src/presentation/battlefield.rs`, replace the Task 1 temporary mapping with:

```rust
UnitArchetype::Dreadnought => 13,
```

- [ ] **Step 5: Validate JSON and run presentation/asset tests**

```bash
python -m json.tool assets/models/mission_one.gltf >/dev/null
cargo fmt --check
cargo test --lib presentation::assets::tests
cargo test --test presentation_app
```

Expected: glTF parses; Dreadnought scene/count tests pass; existing scenes remain unchanged.

- [ ] **Step 6: Commit the Dreadnought visual**

```bash
git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: present the Dreadnought boss"
```

---

### Task 5: Close HPA-524 with validation, documentation, and full gates

**Files:**
- Create: `docs/validation/hpa-524.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify as needed only for test-backed defects found by the gates/playthrough; do not widen product scope.

**Interfaces:**
- Consumes: completed Mission 6 implementation, existing validation-ledger style, project CI commands.
- Produces: reproducible HPA-524 evidence and planning docs that remain truthful after implementation.

- [ ] **Step 1: Update product documentation to the shipped six-mission state**

In `README.md` and `CLAUDE.md`, update only concrete shipped facts:

```text
- authored campaign now runs Missions 1–6 and hands off to Mission 7;
- regular roster remains six archetypes;
- Mission 6 adds one single-cell Dreadnought boss;
- Dreadnought commits Graviton Salvo above half HP and Overload Salvo at/below half HP;
- threshold affects future planning only; committed intents stay locked;
- boss remains pushable;
- save/upgrade flow now advances through Mission 6.
```

Do not document Mission 7 content, generic boss systems, resistance, or future features as shipped.

- [ ] **Step 2: Create the validation ledger with exact automated commands**

Create `docs/validation/hpa-524.md` and record these commands with their final pass/fail output summary:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Also record the final test count reported by `cargo test --all-targets`.

- [ ] **Step 3: Perform the real campaign/manual Mission 6 playthrough**

Run:

```bash
cargo run
```

From the real campaign flow:

1. reach/start Mission 6 from the saved Mission 5 completion path;
2. verify the opening Graviton Salvo Cross1 is readable;
3. vacate Vanguard `(4,7)`, move Interceptor to `(7,7)`, and push Controller toward `(5,7)` to confirm the intended redirection line is practical;
4. cross Dreadnought from 21+ HP to 20-or-less after an intent is committed and confirm that current telegraph stays Graviton Salvo;
5. confirm the next planning pass commits Overload Salvo with the shorter range/higher damage presentation;
6. push the Dreadnought once and confirm normal one-cell displacement still works;
7. defeat Dreadnought with at least one escort alive and confirm immediate victory;
8. finish aftermath/reward/upgrade, return to title, Continue, and confirm `MISSION 7 UNLOCKED` handoff from persisted state;
9. record approximate encounter duration and any authored HP/damage tuning made.

If the fight is too short/long, tune only Mission 6 HP/damage/opening positions and update the spec/plan locked values in this same PR. Do not add new systems as a tuning response.

- [ ] **Step 4: Re-run full gates after any manual tuning**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Expected: every gate green on the final product commit.

- [ ] **Step 5: Self-review scope and planning truthfulness**

Verify all of the following before marking HPA-524 complete:

```text
- exactly one new boss archetype
- no threshold/phase registry or stored boss phase
- no new objective/optional-objective shape
- no displacement resistance
- no Mission 7 content
- Mission 6 target victory works with escorts alive
- current-round intent remains immutable across threshold crossing
- Six is authored; Seven is terminal
- spec/plan values match final tuned implementation
- one ticket / one PR preserved
```

- [ ] **Step 6: Commit closeout evidence**

```bash
git add README.md CLAUDE.md docs/validation/hpa-524.md docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md docs/superpowers/plans/2026-08-31-hpa-524-mission-6-dreadnought.md
git commit -m "docs: validate HPA-524 Mission 6"
```

- [ ] **Step 7: Keep implementation in this same PR**

Do not open a second implementation PR. The draft planning PR created for HPA-524 is the review unit for Tasks 1–5 and should be marked ready only after the final gates/manual ledger are complete.