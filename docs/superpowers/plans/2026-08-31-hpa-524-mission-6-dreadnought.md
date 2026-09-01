# HPA-524 Mission 6 Dreadnought Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 6 and the first Dreadnought boss as one player-visible HPA-524 slice, with one half-HP behavior change on the existing locked-intent path and a persisted Mission 7 handoff.

**Architecture:** Add one concrete `Dreadnought` archetype. Extend the existing `unit_weapon` selector so Dreadnought uses slot 1 at/below half HP, then make `build_intent` use that same selector. Mission 6 owns the boss values, weapons, board, escorts, dialogue, rewards, and opening geometry. Campaign routing keeps Mission 1 special and otherwise derives authored-vs-handoff from `mission_definition`, matching the existing `Proceed` seam.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, Cargo tests plus existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md`

## Global Constraints

- One HPA-524 ticket = one PR; implementation continues on this PR.
- One normal single-cell `UnitState`; no boss runtime, stored phase, threshold registry, or scripting.
- Threshold: HP 21–40 -> Graviton Salvo; HP 0–20 -> Overload Salvo.
- Committed intents never change when HP crosses the threshold.
- Dreadnought remains pushable; no resistance field/system.
- Mission 6 owns the boss factory and weapons locally; `mission/enemies.rs` stays the regular-roster layer.
- Mission 6: 9×9, existing blocking only, no hazards/explosives, `EliminateTarget`, `Turnabout`, rewards 800 + 250.
- Mission IDs become One–Seven; One–Six authored, Seven terminal.
- `Continue`: One -> story; other authored IDs -> Upgrade; unauthored handoff -> NextMission. Do not enumerate Two–Six.
- Reuse existing VN assets and one checked-in glTF.
- Final glTF counts: 14 scenes, 77 nodes, 14 meshes, 14 materials, 1 buffer.
- No new objective shape, optional shape, status system, behavior policy, turn-limit system, progression system, save migration, dependency, crate, Mission 7 content, or second PR.

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
- Consumes: `UnitState.weapons`, `unit_weapon`, `build_intent`, `attack_band_destination`, exhaustive archetype matches.
- Produces: `UnitArchetype::Dreadnought`, derived half-HP weapon selection, initiative 40, attack-band movement, temporary scene mapping.

- [ ] **Step 1: Add failing threshold fixtures/tests**

Add constants:

```rust
const DREADNOUGHT: UnitId = UnitId(90);
const TEST_PLAYER: UnitId = UnitId(91);
const GRAVITON: WeaponId = WeaponId(290);
const OVERLOAD: WeaponId = WeaponId(291);
```

Add this fixture:

```rust
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
    BattleState::new(
        BoardState::new(7, 7, [], [], []),
        vec![boss, player],
        vec![
            squad::weapon(GRAVITON, "Graviton Salvo", 3, 6, WeaponShape::Cross1, 8, 10, 5, 0, false, false),
            squad::weapon(OVERLOAD, "Overload Salvo", 1, 4, WeaponShape::Cross1, 10, 10, 10, 0, false, false),
        ],
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

Add this second fixture to prove close pressure:

```rust
fn dreadnought_close_pressure_fixture() -> BattleState {
    let boss = squad::unit(
        DREADNOUGHT,
        "Dreadnought",
        UnitArchetype::Dreadnought,
        Faction::Enemy,
        squad::stats(40, 3, 1, 90, 5, 0),
        GridPos::new(3, 0),
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
    BattleState::new(
        BoardState::new(7, 7, [], [], []),
        vec![boss, player],
        vec![
            squad::weapon(GRAVITON, "Graviton Salvo", 3, 6, WeaponShape::Cross1, 8, 10, 5, 0, false, false),
            squad::weapon(OVERLOAD, "Overload Salvo", 1, 4, WeaponShape::Cross1, 10, 10, 10, 0, false, false),
        ],
        MissionRules {
            primary: PrimaryObjective::EliminateAllEnemies,
            optional: OptionalObjective::VictoryByRound { round: 9 },
            opening_plan: &[],
        },
        7,
    )
}

#[test]
fn dreadnought_overload_closes_from_range_five() {
    let mut battle = dreadnought_close_pressure_fixture();
    battle.apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);
    let destination = choose_enemy_destination(&battle, DREADNOUGHT).unwrap();
    assert_eq!(destination, GridPos::new(3, 1));
    assert_eq!(destination.manhattan(GridPos::new(3, 5)), 4);
}
```

- [ ] **Step 2: Verify red**

```bash
cargo test --lib dreadnought -- --nocapture
```

Expected: compile failure because Dreadnought does not exist and weapon selection is still slot 0.

- [ ] **Step 3: Implement the concrete archetype and selector**

Append `Dreadnought` to `UnitArchetype`.

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

- [ ] **Step 4: Make intent construction use the selector**

At the start of `build_intent`:

```rust
let attacker = battle.unit(attacker_id).ok_or(BattleError::UnknownUnit(attacker_id))?;
let weapon = unit_weapon(battle, attacker)?;
let weapon_id = weapon.id;
```

Remove the independent `.first()` path. Leave profile/footprint snapshot logic unchanged.

- [ ] **Step 5: Use ordinary attack-band movement and initiative 40**

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Bulwark
| UnitArchetype::Dreadnought => {
    let weapon = unit_weapon(battle, unit)?;
    Ok(attack_band_destination(&candidates, &players, weapon))
}
```

```rust
UnitArchetype::Dreadnought => 40,
UnitArchetype::Controller => 35,
```

- [ ] **Step 6: Keep presentation matches exhaustive**

Temporarily map `Dreadnought => 11` in `scene_index`. Add Dreadnought to enemy-only pilot-skill branches in `ui.rs` and `interaction.rs`. Add no boss HUD/command.

- [ ] **Step 7: Verify and commit**

```bash
cargo fmt --check
cargo test --lib dreadnought
cargo test --all-targets
git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add dreadnought threshold behavior"
```

---

### Task 2: Author Mission 6 and register it without breaking the library

**Files:**
- Create: `src/mission/mission_six.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/mission/mission_two.rs`
- Modify: `src/mission/mission_three.rs`
- Modify: `src/mission/mission_four.rs`
- Modify: `src/mission/mission_five.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Test: `src/mission/mission_six.rs`

**Interfaces:**
- Consumes: shared squad builder, local `squad::{unit, stats, weapon}`, regular enemy factories, `EliminateTarget`, `Turnabout`, shared opening validator.
- Produces: `MISSION_SIX_DEFINITION`, Seven terminal handoff, data-driven Continue routing, exact Mission 6 encounter.

- [ ] **Step 1: Add failing authoring tests**

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

Also assert exactly four enemy IDs and these rows:

```rust
[
    (ids::DREADNOUGHT, GridPos::new(4, 2), Some(ids::VANGUARD)),
    (ids::BULWARK, GridPos::new(1, 7), Some(ids::VANGUARD)),
    (ids::CONTROLLER, GridPos::new(6, 7), Some(ids::VANGUARD)),
    (ids::RIFLEMAN, GridPos::new(6, 6), Some(ids::INTERCEPTOR)),
]
```

- [ ] **Step 2: Verify red**

```bash
cargo test --lib mission::mission_six -- --nocapture
```

- [ ] **Step 3: Register Six/Seven and repair existing library pins immediately**

In `src/mission/mod.rs`:

```rust
pub mod mission_six;
```

Add `Seven` to `MissionId`, display it as `7`, and register:

```rust
MissionId::Six => Some(&mission_six::MISSION_SIX_DEFINITION),
MissionId::Seven => None,
```

In `mission_two.rs`, `mission_three.rs`, `mission_four.rs`, and `mission_five.rs`, change the old terminal assertion to:

```rust
assert!(mission_definition(MissionId::Seven).is_none());
```

In Mission 5 also assert:

```rust
assert!(mission_definition(MissionId::Six).is_some());
```

- [ ] **Step 4: Replace the leftover hardcoded Continue list**

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

Update comments that call Six terminal. Add no routing table/helper.

- [ ] **Step 5: Implement the local boss IDs/factory/weapons**

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

```rust
fn dreadnought() -> UnitState {
    unit(
        ids::DREADNOUGHT,
        "Dreadnought",
        UnitArchetype::Dreadnought,
        Faction::Enemy,
        stats(40, 3, 1, 90, 5, 0),
        GridPos::new(4, 1),
        vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO],
    )
}

fn graviton_salvo() -> WeaponSpec {
    weapon(ids::GRAVITON_SALVO, "Graviton Salvo", 3, 6, WeaponShape::Cross1, 8, 10, 5, 0, false, false)
}

fn overload_salvo() -> WeaponSpec {
    weapon(ids::OVERLOAD_SALVO, "Overload Salvo", 1, 4, WeaponShape::Cross1, 10, 10, 10, 0, false, false)
}
```

Use existing Bulwark, Controller, Rifleman factories/weapons.

- [ ] **Step 6: Author deployment/opening/rules/definition**

```rust
const MISSION_SIX_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

static MISSION_SIX_OPENING: [EnemyOpening; 4] = [
    EnemyOpening { unit: ids::DREADNOUGHT, destination: GridPos::new(4, 2), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::BULWARK, destination: GridPos::new(1, 7), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::CONTROLLER, destination: GridPos::new(6, 7), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::RIFLEMAN, destination: GridPos::new(6, 6), target: Some(ids::INTERCEPTOR) },
];

const MISSION_SIX_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateTarget { target: ids::DREADNOUGHT },
    optional: OptionalObjective::Turnabout,
    opening_plan: &MISSION_SIX_OPENING,
};
```

Board is 9×9 with blocking `(2,4) (6,4) (2,5) (6,5)` and empty hazard/explosive lists.

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

Use the spec's three pre-mission and two aftermath lines verbatim.

- [ ] **Step 7: Pin opening legality**

```rust
#[test]
fn mission_six_opening_rows_are_legal() {
    assert_opening_plan_is_legal(&mission_six(7));
}
```

- [ ] **Step 8: Add a test-only public-line helper**

```rust
fn redirect_controller_into_graviton(battle: &mut BattleState) {
    battle.begin_round().unwrap();
    let boss_intent = battle.intent_for(ids::DREADNOUGHT).unwrap();
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
}
```

- [ ] **Step 9: Pin geometry and Turnabout**

```rust
#[test]
fn mission_six_redirect_line_puts_controller_in_locked_boss_footprint() {
    let mut battle = mission_six(7);
    redirect_controller_into_graviton(&mut battle);
    assert_eq!(battle.unit(ids::VANGUARD).unwrap().position, GridPos::new(4, 5));
    let events = battle.resolve_intent_for_test(ids::CONTROLLER).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        BattleEvent::AttackHitEmpty { attacker, cell, .. }
            if *attacker == ids::CONTROLLER && *cell == GridPos::new(4, 7)
    )));
}

#[test]
fn redirected_graviton_can_complete_turnabout() {
    let mut witnessed = false;
    for seed in 0..256 {
        let mut battle = mission_six(seed);
        redirect_controller_into_graviton(&mut battle);
        let events = battle.resolve_intent_for_test(ids::DREADNOUGHT).unwrap();
        let rolled_on_controller = events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackRolled { attacker, target, .. }
                if *attacker == ids::DREADNOUGHT && *target == ids::CONTROLLER
        ));
        let completed = events.iter().any(|event| matches!(event, BattleEvent::OptionalObjectiveCompleted));
        if rolled_on_controller && completed {
            witnessed = true;
            break;
        }
    }
    assert!(witnessed, "expected a deterministic seed to land redirected Graviton damage");
}
```

- [ ] **Step 10: Pin target victory and ordinary displacement**

```rust
#[test]
fn dreadnought_ko_wins_with_escorts_alive() {
    let mut battle = mission_six(7);
    battle.apply_direct_damage(ids::DREADNOUGHT, 99, DamageSource::PlayerWeapon(squad::ids::RAIL_RIFLE));
    assert!(battle.result().is_some_and(|result| result.victory));
    assert!(!battle.unit(ids::BULWARK).unwrap().is_knocked_out());
}
```

For displacement, move Vanguard to `(3,3)` and Dreadnought to `(4,3)` with `move_unit_direct_for_test`, call `resolve_push(ids::VANGUARD, ids::DREADNOUGHT)`, then assert Dreadnought is at `(5,3)` and the events contain `UnitPushed { unit: ids::DREADNOUGHT, from: (4,3), to: (5,3) }`.

- [ ] **Step 11: Verify library and commit**

```bash
cargo fmt --check
cargo test --lib mission::mission_six
cargo test --lib
git add src/mission/mod.rs src/mission/mission_six.rs src/mission/mission_two.rs src/mission/mission_three.rs src/mission/mission_four.rs src/mission/mission_five.rs src/presentation/campaign_ui.rs
git commit -m "feat: author Mission 6 Dreadnought encounter"
```

Expected: library stays green immediately after Six becomes authored.

---

### Task 3: Advance campaign/save integration through Mission 6

**Files:**
- Modify: `tests/campaign_model.rs`
- Modify: `tests/campaign_flow.rs`
- Modify: `tests/campaign_persistence.rs`

**Interfaces:**
- Consumes: `MISSION_SIX_DEFINITION`, `complete_current_mission`, Task 2 Continue routing, existing `Proceed` check.
- Produces: base credits 3300 through Six, optional reward 250 coverage, Seven persistence, Six-authored/Seven-terminal routing tests.

- [ ] **Step 1: Update campaign model expectations**

```rust
let six = mission_definition(MissionId::Six).unwrap();
assert_eq!(six.id, MissionId::Six);
assert_eq!(six.unlocks, MissionId::Seven);
assert_eq!(six.title, "Mission 6 — Break the Dreadnought");
assert_eq!((six.base_reward, six.optional_reward), (800, 250));
assert_eq!(mission_definition(MissionId::Seven), None);
```

Extend the existing base-reward sum through Six and assert `3300`.

- [ ] **Step 2: Extend progression with the real API**

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

Add a fresh completion with `optional_complete: true` and assert `(base_reward, optional_reward, total_reward) == (800, 250, 1050)`.

- [ ] **Step 3: Move Continue assertions from Six to Seven**

Update the existing end-of-Mission-5 Continue assertion to expect `GameScreen::Upgrade` at Six.

```rust
assert_eq!(route_continue(MissionId::Six), Some(GameScreen::Upgrade));
assert_eq!(route_continue(MissionId::Seven), Some(GameScreen::NextMission));
```

- [ ] **Step 4: Move Proceed assertions from Six to Seven**

For Six:

```rust
assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));
```

Add a Seven fixture and assert:

```rust
assert_eq!(pending(&next), Some(GameScreen::NextMission));
```

- [ ] **Step 5: Pin Seven persistence and upgrades**

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

- [ ] **Step 6: Pin Mission 6 story/briefing**

```rust
let definition = mission_definition(MissionId::Six).unwrap();
let copy = briefing_copy(definition);
assert!(copy.contains("Mission 6 — Break the Dreadnought"));
assert!(copy.contains("800 credits"));
assert!(copy.contains("+250 credits"));
assert_eq!(dialogue_snapshot(&definition.pre_mission, DialogueCursor(0)).speaker, "Control");
assert_eq!(dialogue_snapshot(&definition.aftermath, DialogueCursor(1)).speaker, "Control");
```

- [ ] **Step 7: Verify and commit**

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
- Consumes: current one-buffer glTF and existing appended-unit tests.
- Produces: scene 13, final counts 14/77/14/14/1, bounded Controller test loop, permanent scene mapping.

- [ ] **Step 1: Update old global-count tests and bound Controller loop**

In the Flanker test:

```rust
assert_eq!(scenes.len(), 14);
assert_eq!(nodes.len(), 77);
assert_eq!(meshes.len(), 14);
assert_eq!(materials.len(), 14);
```

In the Bulwark/Controller test, apply the same counts and replace:

```rust
for (index, part) in nodes.iter().enumerate().skip(64) {
```

with:

```rust
for (index, part) in nodes.iter().enumerate().skip(64).take(6) {
```

- [ ] **Step 2: Add failing Dreadnought structure test**

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
    assert_eq!(materials[13]["pbrMetallicRoughness"]["baseColorFactor"], serde_json::json!([0.55, 0.08, 0.12, 1.0]));
    assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 3: Verify red**

```bash
cargo test --lib presentation::assets::tests -- --nocapture
```

- [ ] **Step 4: Append scene/root/parts/mesh/material**

Append:

```text
Scene 13: name Dreadnought, nodes [70]
Root 70: name Dreadnought Root, children [71,72,73,74,75,76], scale [1.12,1.12,1.12]
Parts 71–76: copy Bulwark part transforms, rename with Dreadnought prefix, mesh 13
Mesh 13: Dreadnought Crimson, same POSITION/NORMAL accessors, material 13
Material 13: Dreadnought Crimson, baseColorFactor [0.55,0.08,0.12,1.0], same metallic/roughness shape as existing unit materials
```

Do not modify buffers/accessors.

- [ ] **Step 5: Update scene loading/mapping**

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 14;
```

```rust
UnitArchetype::Dreadnought => 13,
```

- [ ] **Step 6: Verify and commit**

```bash
python -m json.tool assets/models/mission_one.gltf >/dev/null
cargo fmt --check
cargo test --lib presentation::assets::tests
cargo test --test presentation_app
cargo test --all-targets
git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: present the Dreadnought boss"
```

---

### Task 5: Close HPA-524 with validation and shipped docs

**Files:**
- Create: `docs/validation/hpa-524.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: spec/plan only if manual tuning changes locked values.

- [ ] **Step 1: Document shipped facts**

Document only:

```text
Missions 1–6 authored; Mission 7 handoff
six regular enemy archetypes plus one Dreadnought boss
Graviton above half HP; Overload at/below half HP
future planning changes; committed intents remain locked
boss remains pushable
save/upgrade flow advances through Mission 6
```

- [ ] **Step 2: Run and record automated gates**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Record final test count in `docs/validation/hpa-524.md`.

- [ ] **Step 3: Perform the real campaign playthrough**

Run `cargo run` and record evidence that:

1. Mission 5 completion reaches Mission 6 upgrade/story flow;
2. opening Graviton Cross1 is readable;
3. Vanguard can vacate `(4,7)` and Interceptor can push Controller to `(5,7)`;
4. redirected boss fire can complete Turnabout;
5. crossing 21+ -> <=20 after commitment leaves current telegraph unchanged;
6. next planning commits Overload and closes from range 5 to range 4;
7. Dreadnought can be pushed normally;
8. Dreadnought KO with escorts alive wins immediately;
9. aftermath/reward/upgrade persist Seven and Continue reaches the Mission 7 handoff;
10. encounter duration is within the intended short tactical-session target.

Tune only authored HP/damage/opening positions if pacing is poor.

- [ ] **Step 4: Re-run full gates after tuning**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

- [ ] **Step 5: Scope self-review**

Verify:

```text
one new boss archetype
no phase/threshold framework
no new objective shapes
no resistance
no Mission 7 content
locked intent regression covered
Overload close-pressure movement covered
redirected boss Turnabout covered
Six authored; Seven terminal
Continue data-driven after Mission 1
old glTF count tests updated; Controller loop bounded
one ticket / one PR
```

- [ ] **Step 6: Commit closeout**

```bash
git add README.md CLAUDE.md docs/validation/hpa-524.md docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md docs/superpowers/plans/2026-08-31-hpa-524-mission-6-dreadnought.md
git commit -m "docs: validate HPA-524 Mission 6"
```

- [ ] **Step 7: Keep implementation on this PR**

Do not open a second implementation PR. Mark this draft ready only after final gates and validation evidence are complete.