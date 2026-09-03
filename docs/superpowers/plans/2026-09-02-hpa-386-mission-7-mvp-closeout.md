# HPA-386 Mission 7 and MVP Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 7, the Regent final boss, stable once-only campaign completion/Ending, focused board-first feedback improvements, and one evidence-driven seven-mission MVP tuning pass in the same HPA-386 PR.

**Architecture:** Reuse the Mission 6 half-HP weapon-slot seam for the second boss instead of adding a boss framework. Author Mission 7 as ordinary typed Rust content on a 9×9 board using existing push/explosive/hazard rules. Replace the old unauthored-Seven sentinel with `MissionDefinition.unlocks: Option<MissionId>` plus one persisted `CampaignState.completed` bit. Presentation changes stay on the existing event playback, Camera3d, and HUD UI `Text` paths.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus the existing Bevy `App` integration tests and GitHub Actions coverage/release gates.

**Spec:** `docs/superpowers/specs/2026-09-02-hpa-386-mission-7-mvp-closeout-design.md`

## Global Constraints

- One HPA-386 ticket = one PR. Continue implementation on `jack65786656/hpa-386-scorpius-m2-author-mission-7-and-finish-mvp`; do not create a second implementation PR.
- Keep exactly three playable mechs, six regular enemy archetypes, and two boss archetypes.
- Regent is one normal single-cell `UnitState`; no boss runtime, phase state, threshold registry, scripting, parts, invulnerability, multi-tile collision, or resistance model.
- Dreadnought and Regent share only the existing half-HP slot selector in `unit_weapon`; committed intents never mutate after commitment.
- Regent starts at HP52 / Armor4 / Move2 / Accuracy92 / Evasion8 / Initiative45; Command Barrage is 3–6 Cross1 dmg9 +10 hit 5% crit; Rupture Beam is 2–4 Single dmg12 +15 hit 10% crit.
- Mission 7 is **9×9** with the exact board/opening in the spec, `EliminateTarget { REGENT }`, `VictoryByRound { round: 6 }`, and 1000 + 300 credits.
- The Controller push landing `(5,7)` must remain empty. The explosive is `(3,7)`. Do not modify `is_open_for` or `resolve_push` to permit unit/explosive overlap.
- Mission Seven is authored and terminal: `MissionDefinition.unlocks: Option<MissionId>`; One–Six use `Some(next)`, Seven uses `None`.
- Persist exactly one new terminal field: `CampaignState.completed: bool`. No save version, serde default, converter, migration, or backward compatibility.
- Rename the old terminal `NextMission` screen to `Ending`; completed Continue routes there and never builds another battle.
- Skip audio. There is no existing audio path.
- Presentation additions are limited to attacker motion, transient damage-number **UI `Text`** on the existing `HudRoot`, and modest deterministic boss camera emphasis. Do not add `Text2d`, `Camera2d`, or replace the existing 3D impact mesh.
- Keep the 9×9-centered `grid_to_world`; no board-size camera/grid refactor unless the final playtest demonstrates a concrete clipping problem.
- Regent stays in `assets/models/mission_one.gltf`; final counts are 15 scenes / 84 nodes / 15 meshes / 15 materials / 1 buffer.
- Whole-campaign tuning changes authored values only when the recorded playtest demonstrates a concrete problem.
- No new dependency/crate, objective/status/AI/boss/narrative framework, seventh regular enemy, extra playable mech, new progression track, New Game+, second glTF, analytics/tuning system, or second PR.

## Known blast radius

- Adding `Regent` makes exhaustive `UnitArchetype` matches compile-time work in domain and presentation code.
- Changing `MissionDefinition.unlocks` changes every mission definition and any test that compares `unlocks`.
- Adding `CampaignState.completed` changes every direct `CampaignState { ... }` fixture and persisted snapshot.
- Registering Seven invalidates all old `mission_definition(Seven).is_none()` assertions and all routing that treated Seven as a sentinel.
- Renaming `GameScreen::NextMission` affects `app.rs`, campaign UI, `tests/campaign_flow.rs`, and `tests/presentation_app.rs`.
- Appending Regent changes every glTF test pinning global scene/node/mesh/material counts.

---

### Task 1: Add Regent as the second half-HP boss consumer

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/interaction.rs`
- Modify: `src/presentation/ui.rs`
- Test: `src/domain/enemy.rs`
- Test: `src/presentation/ui.rs`
- Test: `src/presentation/interaction.rs`

**Interfaces:**
- Consumes: existing Dreadnought `unit_weapon` behavior, `attack_band_destination`, `build_intent`, fixed initiative table, `HudSnapshot`, `execute_command`.
- Produces: `UnitArchetype::Regent`; shared half-HP selector; Regent initiative 45; normal attack-band movement; explicit enemy-only HUD/pilot behavior; temporary scene 13 mapping until Task 3.

- [ ] **Step 1: Add the Regent threshold fixture and failing 27/26 test**

In the existing `src/domain/enemy.rs` test module, add fixture IDs separate from authored Mission 7:

```rust
const REGENT: UnitId = UnitId(92);
const REGENT_PLAYER: UnitId = UnitId(93);
const COMMAND_BARRAGE: WeaponId = WeaponId(292);
const RUPTURE_BEAM: WeaponId = WeaponId(293);
```

Build a 7×7 planning fixture with Regent `(3,1)`, one player `(3,5)`, stats `52/4/2/92/8/0`, and these weapons:

```rust
squad::weapon(
    COMMAND_BARRAGE,
    "Command Barrage",
    3,
    6,
    WeaponShape::Cross1,
    9,
    10,
    5,
    0,
    false,
    false,
)

squad::weapon(
    RUPTURE_BEAM,
    "Rupture Beam",
    2,
    4,
    WeaponShape::Single,
    12,
    15,
    10,
    0,
    false,
    false,
)
```

Add:

```rust
#[test]
fn both_bosses_switch_at_their_exact_half_hp_boundary() {
    let mut regent = regent_threshold_fixture();
    regent.apply_direct_damage(REGENT, 25, DamageSource::Collision);
    assert_eq!(regent.unit(REGENT).unwrap().hp, 27);
    assert_eq!(
        unit_weapon(&regent, regent.unit(REGENT).unwrap()).unwrap().id,
        COMMAND_BARRAGE
    );

    regent.apply_direct_damage(REGENT, 1, DamageSource::Collision);
    assert_eq!(regent.unit(REGENT).unwrap().hp, 26);
    assert_eq!(
        unit_weapon(&regent, regent.unit(REGENT).unwrap()).unwrap().id,
        RUPTURE_BEAM
    );

    let mut dreadnought = dreadnought_threshold_fixture();
    dreadnought.apply_direct_damage(DREADNOUGHT, 19, DamageSource::Collision);
    assert_eq!(dreadnought.unit(DREADNOUGHT).unwrap().hp, 21);
    assert_eq!(
        unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap()).unwrap().id,
        GRAVITON
    );
    dreadnought.apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
    assert_eq!(dreadnought.unit(DREADNOUGHT).unwrap().hp, 20);
    assert_eq!(
        unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap()).unwrap().id,
        OVERLOAD
    );
}
```

- [ ] **Step 2: Add the failing immutable-intent regression**

```rust
#[test]
fn regent_threshold_crossing_changes_only_future_intents() {
    let mut battle = regent_threshold_fixture();
    battle.begin_round().unwrap();
    let committed = battle.intent_for(REGENT).unwrap().clone();
    assert_eq!(committed.profile.weapon, COMMAND_BARRAGE);

    battle.apply_direct_damage(REGENT, 26, DamageSource::Collision);
    assert_eq!(battle.unit(REGENT).unwrap().hp, 26);
    assert_eq!(battle.intent_for(REGENT).unwrap(), &committed);

    let future = build_intent(&battle, REGENT, Some(GridPos::new(3, 5))).unwrap();
    assert_eq!(future.profile.weapon, RUPTURE_BEAM);
}
```

- [ ] **Step 3: Confirm red**

Run:

```bash
cargo test --lib regent -- --nocapture
```

Expected: compile failure because `UnitArchetype::Regent` and its match arms do not exist.

- [ ] **Step 4: Add Regent and extend only `unit_weapon`**

In `src/domain/model.rs`, append `Regent` to `UnitArchetype`.

In `src/domain/enemy.rs`:

```rust
pub(crate) fn unit_weapon<'a>(
    battle: &'a BattleState,
    unit: &UnitState,
) -> Result<&'a WeaponSpec, BattleError> {
    let index = match unit.archetype {
        UnitArchetype::Dreadnought | UnitArchetype::Regent
            if unit.hp * 2 <= unit.stats.max_hp => 1,
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

Do not add a boss/phase data type.

- [ ] **Step 5: Add Regent to ordinary movement and initiative**

Extend the current attack-band branch:

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Bulwark
| UnitArchetype::Dreadnought
| UnitArchetype::Regent => {
    let weapon = unit_weapon(battle, unit)?;
    Ok(attack_band_destination(&candidates, &players, weapon))
}
```

Add Regent initiative 45 and extend `initiative_is_fixed_per_archetype_without_position`:

```rust
assert_eq!(initiative(&regent), 45);
assert!(initiative(&regent) > initiative(&dreadnought));
assert!(initiative(&regent) > initiative(&controller));
```

- [ ] **Step 6: Make every presentation/pilot arm explicit now**

This is checklist work, not deferred compiler discovery.

In `src/presentation/battlefield.rs` temporarily map:

```rust
UnitArchetype::Regent => 13,
```

Task 3 changes it to 14 after the Regent scene exists.

In `src/presentation/ui.rs`, add Regent to **both** enemy-only branches:

```rust
// HudSnapshot::can_pilot
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Artillery
| UnitArchetype::Flanker
| UnitArchetype::Bulwark
| UnitArchetype::Controller
| UnitArchetype::Dreadnought
| UnitArchetype::Regent => false,
```

```rust
// HudSnapshot::pilot_label
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Artillery
| UnitArchetype::Flanker
| UnitArchetype::Bulwark
| UnitArchetype::Controller
| UnitArchetype::Dreadnought
| UnitArchetype::Regent => "[P] PILOT",
```

In `src/presentation/interaction.rs`, add Regent to `CommandAction::PilotSkill` rejection:

```rust
| UnitArchetype::Regent => {
    return Err(BattleError::PilotSkillWrongUnit(unit_id));
}
```

Extend existing focused UI/interaction tests if they enumerate enemy archetypes; do not add a boss-specific command.

- [ ] **Step 7: Verify Task 1**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib regent
cargo test --all-targets
```

Expected: threshold/immutable-intent/initiative tests pass and every exhaustive Regent arm compiles.

- [ ] **Step 8: Commit**

```bash
git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/interaction.rs src/presentation/ui.rs
git commit -m "feat: add Regent boss behavior"
```

---

### Task 2: Author Mission 7 and replace the Seven sentinel with terminal campaign state

**Files:**
- Create: `src/mission/mission_seven.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/mission/mission_one.rs`
- Modify: `src/mission/mission_two.rs`
- Modify: `src/mission/mission_three.rs`
- Modify: `src/mission/mission_four.rs`
- Modify: `src/mission/mission_five.rs`
- Modify: `src/mission/mission_six.rs`
- Modify: `src/campaign/model.rs`
- Modify: `src/campaign/progression.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `src/presentation/interaction.rs`
- Modify: `src/app.rs`
- Modify: `tests/campaign_model.rs`
- Modify: `tests/campaign_flow.rs`
- Modify: `tests/campaign_persistence.rs`
- Modify: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: Task 1 Regent behavior, shared enemy factories, `build_player_squad`, current mission registration/session flow, `EliminateTarget`, `VictoryByRound`, hazards/explosives/push.
- Produces: authored `MISSION_SEVEN_DEFINITION`; exact legal 9×9 final encounter; `MissionDefinition.unlocks: Option<MissionId>`; `CampaignState.completed`; `CampaignError::CampaignComplete`; `GameScreen::Ending`; completed-save routing.

This must land as one coordinated task/commit because Seven changes meaning from unauthored sentinel to a playable terminal mission while the completion model and routing change with it.

- [ ] **Step 1: Write the terminal campaign tests first**

In `tests/campaign_persistence.rs`, add:

```rust
#[test]
fn final_mission_completion_is_persisted_and_idempotent() {
    let mut state = CampaignState {
        next_mission: MissionId::Seven,
        credits: 3300,
        upgrades: SquadUpgrades::default(),
        completed: false,
    };
    let definition = mission_definition(MissionId::Seven).unwrap();
    let result = mission_result(true, true);

    let receipt = state.complete_mission(definition, result).unwrap();
    assert_eq!(receipt.total_reward, 1300);
    assert_eq!(state.credits, 4600);
    assert_eq!(state.next_mission, MissionId::Seven);
    assert!(state.completed);

    let snapshot = state.clone();
    assert!(matches!(
        state.complete_mission(definition, result),
        Err(CampaignError::CampaignComplete)
    ));
    assert_eq!(state, snapshot);
}
```

Update every direct `CampaignState { ... }` test fixture to set `completed` explicitly. Do not use `..Default::default()` to hide the new field in tests intended to pin persistence shape.

- [ ] **Step 2: Write Mission 7 authoring tests before the factory**

Create `src/mission/mission_seven.rs` with the test module and IDs:

```rust
pub mod ids {
    pub use crate::mission::squad::ids::{GUNNER, INTERCEPTOR, VANGUARD};

    use crate::domain::model::{UnitId, WeaponId};

    pub const REGENT: UnitId = UnitId(71);
    pub const ARTILLERY: UnitId = UnitId(72);
    pub const CONTROLLER: UnitId = UnitId(73);
    pub const BULWARK: UnitId = UnitId(74);
    pub const FLANKER: UnitId = UnitId(75);
    pub const COMMAND_BARRAGE: WeaponId = WeaponId(209);
    pub const RUPTURE_BEAM: WeaponId = WeaponId(210);
}
```

Pin the board:

```rust
#[test]
fn mission_seven_authors_the_final_board_and_rules() {
    let battle = mission_seven(7);
    assert_eq!((battle.board().width(), battle.board().height()), (9, 9));
    assert_eq!(
        battle.board().blocking_cells().collect::<Vec<_>>(),
        vec![
            GridPos::new(2, 4),
            GridPos::new(6, 4),
            GridPos::new(2, 5),
            GridPos::new(6, 5),
        ]
    );
    assert_eq!(
        battle.board().hazard_cells().collect::<Vec<_>>(),
        vec![GridPos::new(3, 5), GridPos::new(5, 5)]
    );
    assert_eq!(battle.board().explosive_at(GridPos::new(3, 7)).unwrap().hp, 4);
    assert_eq!(
        battle.rules().primary,
        PrimaryObjective::EliminateTarget { target: ids::REGENT }
    );
    assert_eq!(
        battle.rules().optional,
        OptionalObjective::VictoryByRound { round: 6 }
    );
}
```

Pin deployment and opening exactly:

```rust
let expected = [
    (ids::REGENT, GridPos::new(4, 2), Some(ids::VANGUARD)),
    (ids::ARTILLERY, GridPos::new(2, 2), Some(ids::GUNNER)),
    (ids::CONTROLLER, GridPos::new(6, 7), Some(ids::VANGUARD)),
    (ids::BULWARK, GridPos::new(2, 6), Some(ids::VANGUARD)),
    (ids::FLANKER, GridPos::new(1, 8), Some(ids::GUNNER)),
];
```

Also call:

```rust
assert_opening_plan_is_legal(&mission_seven(1));
```

This is the first geometry gate. If it fails, fix authored coordinates; do not change generic opening/movement rules.

- [ ] **Step 3: Add the failing public seed-2 manipulation regression before the factory is considered complete**

Use only public battle actions. The exact line is:

```text
Vanguard    (4,7) -> (4,5)
Gunner      (3,8) -> (2,8)
Interceptor (5,8) -> (7,7)
Vector Pulse Controller (6,7) -> (5,7)
Explosive remains at (3,7)
```

Test the two distinct committed-footprint cells:

```rust
assert!(regent_intent.footprint.contains(&GridPos::new(5, 7)));
assert!(regent_intent.footprint.contains(&GridPos::new(3, 7)));
assert!(battle.board().has_live_explosive(GridPos::new(3, 7)));
assert!(!battle.board().has_live_explosive(GridPos::new(5, 7)));
```

Run the real Vector Pulse attack and pin the legal push:

```rust
let pulse_events = battle.attack(
    ids::INTERCEPTOR,
    squad::ids::VECTOR_PULSE,
    GridPos::new(6, 7),
).unwrap();

assert!(pulse_events.iter().any(|event| matches!(
    event,
    BattleEvent::UnitPushed { unit, to, .. }
        if *unit == ids::CONTROLLER && *to == GridPos::new(5, 7)
)));
```

Finish all three activations with normal reactions, then:

```rust
let events = battle.resolve_enemy_phase().unwrap();
```

Pin:

```rust
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::ExplosiveDamaged { position, .. }
        if *position == GridPos::new(3, 7)
)));
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::ExplosionTriggered { position, .. }
        if *position == GridPos::new(3, 7)
)));
assert!(battle.unit(ids::CONTROLLER).unwrap().hp < 6);
```

Also assert the Regent's committed weapon remains `COMMAND_BARRAGE` and that its footprint still contains `(5,7)` after player movement.

Do **not** assert an explosive at `(5,7)`. Do **not** alter `is_open_for` if the push fails; a failure means the authored geometry or setup is wrong.

- [ ] **Step 4: Confirm the intended red state**

Run:

```bash
cargo test --lib mission::mission_seven -- --nocapture
cargo test --test campaign_persistence final_mission_completion_is_persisted_and_idempotent -- --nocapture
```

Expected: compile/test failures because Mission Seven is not registered and terminal campaign fields/types do not exist.

- [ ] **Step 5: Change `MissionDefinition.unlocks` and register Seven atomically**

In `src/mission/mod.rs`:

```rust
pub mod mission_seven;

pub struct MissionDefinition {
    pub id: MissionId,
    pub unlocks: Option<MissionId>,
    pub build: MissionBuilder,
    pub title: &'static str,
    pub primary_objective: &'static str,
    pub optional_objective: &'static str,
    pub base_reward: u32,
    pub optional_reward: u32,
    pub pre_mission: DialogueScene,
    pub aftermath: DialogueScene,
}
```

Register:

```rust
MissionId::Seven => Some(&mission_seven::MISSION_SEVEN_DEFINITION),
```

Update existing definitions:

```text
One   -> Some(Two)
Two   -> Some(Three)
Three -> Some(Four)
Four  -> Some(Five)
Five  -> Some(Six)
Six   -> Some(Seven)
Seven -> None
```

Update old Seven-terminal assertions in the same edit. There must be no intermediate self-unlocking Seven.

- [ ] **Step 6: Add `CampaignState.completed` and exactly-once completion**

In `src/campaign/model.rs`:

```rust
pub struct CampaignState {
    pub next_mission: MissionId,
    pub credits: u32,
    pub upgrades: SquadUpgrades,
    pub completed: bool,
}
```

`new_game()` sets `completed: false`.

In `src/campaign/progression.rs`, add:

```rust
CampaignComplete,
```

to `CampaignError` and its `Display` implementation.

Update `complete_mission` so it checks `self.completed` before rewards, then:

```rust
match definition.unlocks {
    Some(next) => self.next_mission = next,
    None => self.completed = true,
}
```

Do not add migration/default compatibility.

- [ ] **Step 7: Author the Mission 7 factory/content exactly as the spec**

Deployment:

```rust
const MISSION_SEVEN_DEPLOYMENT: SquadDeployment = SquadDeployment {
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
    [
        GridPos::new(2, 4),
        GridPos::new(6, 4),
        GridPos::new(2, 5),
        GridPos::new(6, 5),
    ],
    [GridPos::new(3, 5), GridPos::new(5, 5)],
    [ExplosiveState {
        position: GridPos::new(3, 7),
        hp: 4,
        exploded: false,
    }],
)
```

Opening:

```rust
static MISSION_SEVEN_OPENING: [EnemyOpening; 5] = [
    EnemyOpening { unit: ids::REGENT, destination: GridPos::new(4, 2), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::ARTILLERY, destination: GridPos::new(2, 2), target: Some(ids::GUNNER) },
    EnemyOpening { unit: ids::CONTROLLER, destination: GridPos::new(6, 7), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::BULWARK, destination: GridPos::new(2, 6), target: Some(ids::VANGUARD) },
    EnemyOpening { unit: ids::FLANKER, destination: GridPos::new(1, 8), target: Some(ids::GUNNER) },
];
```

Regent:

```rust
unit(
    ids::REGENT,
    "Regent",
    UnitArchetype::Regent,
    Faction::Enemy,
    stats(52, 4, 2, 92, 8, 0),
    GridPos::new(4, 1),
    vec![ids::COMMAND_BARRAGE, ids::RUPTURE_BEAM],
)
```

Use regular factories for Artillery/Controller/Bulwark/Flanker. Add `Command Barrage` and `Rupture Beam` locally with the locked values from Global Constraints.

Definition:

```rust
pub const MISSION_SEVEN_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Seven,
    unlocks: None,
    build: mission_seven_for_campaign,
    title: "Mission 7 — Last Command",
    primary_objective: "Destroy the Regent and break the command net.",
    optional_objective: "Final Push: destroy the Regent by the end of Round 6.",
    base_reward: 1000,
    optional_reward: 300,
    pre_mission: DialogueScene { /* exact spec lines */ },
    aftermath: DialogueScene { /* exact spec lines */ },
};
```

When implementing the dialogue arrays, copy the exact three pre-mission and three aftermath lines from the spec; reuse existing VN assets only.

- [ ] **Step 8: Replace `NextMission` with `Ending` and route by `completed`**

In `src/app.rs`, rename the enum variant and OnEnter/OnExit systems:

```rust
Ending,
```

Rename `setup_next_mission_screen`/`next_mission_copy` to `setup_ending_screen`/`ending_copy`.

In `apply_campaign_action`:

```rust
CampaignUiAction::Continue => match continue_game(&mut runtime.0) {
    Ok(_) if runtime.0.state.as_ref().is_some_and(|state| state.completed) => {
        next_state.set(GameScreen::Ending)
    }
    Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
    Ok(_) => next_state.set(GameScreen::Upgrade),
    Err(error) => status.0 = error.to_string(),
},
```

For `AdvanceAftermath`, after the final line:

```rust
let completed = runtime
    .0
    .state
    .as_ref()
    .is_some_and(|state| state.completed);
advance_dialogue(
    cursor,
    mission.0.aftermath.lines.len(),
    if completed { GameScreen::Ending } else { GameScreen::Upgrade },
    next_state,
);
```

`Proceed` from Upgrade always targets an authored unfinished mission and therefore goes to `PreMissionStory`.

Ending has only `ReturnToTitle`.

- [ ] **Step 9: Update campaign integration tests as one blast-radius change**

In `tests/campaign_model.rs` pin:

```rust
assert_eq!(mission_definition(MissionId::Six).unwrap().unlocks, Some(MissionId::Seven));
assert_eq!(mission_definition(MissionId::Seven).unwrap().unlocks, None);
```

Pin base rewards:

```rust
let base_before_seven: u32 = [One, Two, Three, Four, Five, Six]
    .into_iter()
    .map(|id| mission_definition(id).unwrap().base_reward)
    .sum();
assert_eq!(base_before_seven, 3300);
```

In `tests/campaign_flow.rs` / `tests/presentation_app.rs`, pin:

```text
unfinished Seven Continue -> Upgrade
unfinished Seven Proceed -> PreMissionStory
completed Continue -> Ending
final aftermath -> Ending
Ending -> Title
```

In persistence tests pin `completed` both false and true across round-trips.

- [ ] **Step 10: Verify Task 2 with the risky test first**

Run in this order:

```bash
cargo fmt --check
cargo test --lib mission::mission_seven -- --nocapture
cargo test --test campaign_persistence final_mission_completion_is_persisted_and_idempotent -- --nocapture
cargo test --test campaign_flow -- --nocapture
cargo test --test presentation_app -- --nocapture
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

The Mission 7 public manipulation test must pass **without any `is_open_for`/`resolve_push` edit**.

- [ ] **Step 11: Commit**

```bash
git add src/mission src/campaign src/presentation/campaign_ui.rs src/presentation/interaction.rs src/app.rs tests/campaign_model.rs tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs
git commit -m "feat: author Mission 7 and campaign ending"
```

---

### Task 3: Append the Regent visual to the existing glTF

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Test: `src/presentation/assets.rs`

**Interfaces:**
- Consumes: existing one-buffer glTF append pattern, Task 1 temporary Regent scene mapping.
- Produces: scene 14 Regent; 15 scene handles; final scene mapping.

- [ ] **Step 1: Add the failing final-count/Regent asset test**

In `src/presentation/assets.rs` add:

```rust
#[test]
fn regent_scene_is_authored_as_the_final_violet_boss() {
    let gltf = mission_gltf();
    let scenes = gltf["scenes"].as_array().unwrap();
    let nodes = gltf["nodes"].as_array().unwrap();
    let meshes = gltf["meshes"].as_array().unwrap();
    let materials = gltf["materials"].as_array().unwrap();

    assert_eq!(scenes.len(), 15);
    assert_eq!(scenes[14]["name"], "Regent");
    assert_eq!(scenes[14]["nodes"], serde_json::json!([77]));
    assert_eq!(nodes.len(), 84);
    assert_eq!(nodes[77]["scale"], serde_json::json!([1.20, 1.20, 1.20]));
    assert_eq!(
        nodes[77]["children"],
        serde_json::json!([78, 79, 80, 81, 82, 83])
    );
    for (index, part) in nodes.iter().enumerate().skip(78).take(6) {
        assert_eq!(part["mesh"], 14, "node {index} must use mesh 14");
    }
    assert_eq!(meshes.len(), 15);
    assert_eq!(meshes[14]["name"], "Regent Violet");
    assert_eq!(meshes[14]["primitives"][0]["material"], 14);
    assert_eq!(materials.len(), 15);
    assert_eq!(materials[14]["name"], "Regent Violet");
    assert_eq!(
        materials[14]["pbrMetallicRoughness"]["baseColorFactor"],
        serde_json::json!([0.42, 0.14, 0.78, 1.0])
    );
    assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
}
```

Update existing Flanker/Bulwark/Controller/Dreadnought global-count assertions to final 15/84/15/15 counts without changing their own node ranges.

- [ ] **Step 2: Confirm red**

```bash
cargo test --lib presentation::assets::tests::regent_scene_is_authored_as_the_final_violet_boss -- --nocapture
```

Expected: current glTF has 14 scenes and no scene 14.

- [ ] **Step 3: Append Regent using the existing cube accessors/buffer**

Append exactly:

```text
scene 14 -> root 77
root 77 -> children 78..83
root scale -> 1.20
mesh 14 -> existing POSITION/NORMAL accessors, material 14
material 14 -> Regent Violet [0.42, 0.14, 0.78, 1.0]
```

No new accessor, buffer, image, texture, animation, or asset file.

- [ ] **Step 4: Raise the scene count and switch the final mapping**

In `src/presentation/assets.rs`:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 15;
```

In `src/presentation/battlefield.rs` replace the Task 1 temporary mapping:

```rust
UnitArchetype::Regent => 14,
```

- [ ] **Step 5: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::assets
cargo test --all-targets
```

```bash
git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: add Regent battlefield visual"
```

---

### Task 4: Fill the concrete combat-feedback gaps on the existing playback/UI paths

**Files:**
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/playback.rs`
- Modify: `src/app.rs` only if scheduling/order needs one explicit system relation
- Test: `src/presentation/playback.rs`
- Test: `tests/presentation_app.rs` only if an App-level spawn/projection assertion is clearer there

**Interfaces:**
- Consumes: existing `EventPlayback`, `UnitVisual`, `EventEffect`, `HudRoot`, `grid_to_world`, battle Camera3d.
- Produces: attacker pulse; `BattleCamera { rest }`; transient projected HUD `Text` damage numbers; deterministic boss shake with exact restore. Existing 3D impact mesh stays unchanged.

- [ ] **Step 1: Add pure/focused failing helpers for attacker emphasis and damage copy**

Keep formatting logic trivial and testable:

```rust
fn damage_number_text(amount: i16) -> String {
    format!("-{amount}")
}

#[test]
fn damage_number_uses_the_domain_applied_amount() {
    assert_eq!(damage_number_text(7), "-7");
    assert_eq!(damage_number_text(12), "-12");
}
```

Add an attacker-scale helper so the pulse does not accumulate transform error:

```rust
fn attack_scale(progress: f32) -> f32 {
    let pulse = (progress * PI).sin();
    UNIT_SCALE * (1.0 + pulse * 0.10)
}
```

Pin start/end at `UNIT_SCALE` and midpoint above it.

- [ ] **Step 2: Add the failing camera-rest behavior test**

Define in `battlefield.rs`:

```rust
#[derive(Component, Clone, Copy)]
pub(crate) struct BattleCamera {
    pub rest: Transform,
}
```

In the playback test module, construct a camera transform and verify the camera-emphasis helper is computed from `rest`, not the previously-mutated transform:

```rust
let rest = Transform::from_xyz(10.8, 12.4, 12.2)
    .looking_at(Vec3::ZERO, Vec3::Y);
let shaken = boss_camera_transform(rest, 0.5);
assert_ne!(shaken.translation, rest.translation);
assert_eq!(boss_camera_transform(rest, 0.0), rest);
assert_eq!(boss_camera_transform(rest, 1.0), rest);
```

- [ ] **Step 3: Confirm red**

```bash
cargo test --lib presentation::playback -- --nocapture
```

Expected: helpers/component behavior do not exist yet.

- [ ] **Step 4: Tag the existing Camera3d with its immutable rest transform**

In `setup_mission_scene`:

```rust
let rest = Transform::from_xyz(10.8, 12.4, 12.2)
    .looking_at(Vec3::ZERO, Vec3::Y);
commands.spawn((
    Camera3d::default(),
    MeshPickingCamera,
    Projection::from(/* existing orthographic projection */),
    rest,
    BattleCamera { rest },
));
```

Do not create another camera.

- [ ] **Step 5: Add attacker pulse without replacing existing target feedback**

Extend `animate_unit_event`:

```rust
BattleEvent::AttackRolled { attacker, .. } if *attacker == visual.0 => {
    transform.scale = Vec3::splat(attack_scale(progress));
}
```

Keep the existing hit-target pulse, `DamageApplied` shake, KO shrink, and counter pulse arms. If match-arm overlap requires combining logic, preserve both attacker and target behavior explicitly rather than deleting one.

- [ ] **Step 6: Add projected damage-number UI on top of the existing impact mesh**

Add a private component in `playback.rs`:

```rust
#[derive(Component)]
struct DamageNumberEffect {
    origin: Vec2,
}
```

Do **not** modify `spawn_event_effect`'s `DamageApplied` impact-mesh branch.

When a new `BattleEvent::DamageApplied { target, amount, .. }` begins:

1. Resolve the target's current grid cell from `BattleRuntime`.
2. Project `grid_to_world(position) + Vec3::Y * 0.8` through the existing `BattleCamera` `Camera::world_to_viewport`.
3. Spawn Bevy UI `Text` under the existing `HudRoot`:

```rust
commands.spawn((
    Text::new(damage_number_text(*amount)),
    TextFont {
        font_size: FontSize::Px(28.0),
        ..default()
    },
    TextColor(Color::WHITE),
    Node {
        position_type: PositionType::Absolute,
        left: px(viewport.x),
        top: px(viewport.y),
        ..default()
    },
    DamageNumberEffect { origin: viewport },
    Pickable::IGNORE,
    ChildOf(hud_root),
));
```

Animate its `top` from `origin.y` to `origin.y - 24.0` using the current event fraction. Despawn it when that event finishes, alongside the existing event effect cleanup.

Do not add `Text2d`, `Camera2d`, a font asset, or another damage calculation.

- [ ] **Step 7: Add boss-only camera emphasis and exact restore**

During current `AttackRolled`, inspect `battle.unit(attacker).archetype`.

For Dreadnought or Regent:

```rust
camera_transform = boss_camera_transform(camera.rest, timer.fraction());
```

For all other events and after current-event completion:

```rust
*camera_transform = camera.rest;
```

Keep amplitude low and deterministic.

- [ ] **Step 8: Add one App-level assertion for the UI path**

Use a minimal `App`/World fixture with:

```text
BattleRuntime
HudRoot
BattleCamera + Camera + GlobalTransform
BattleEventQueue containing one DamageApplied
EventPlayback
```

After the playback system starts the event, assert one entity exists with:

```text
DamageNumberEffect
Text("-7")
Node { position_type: Absolute, ... }
```

This proves the actual UI entity is created; the manual playtest in Task 5 validates visual placement/readability.

- [ ] **Step 9: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::playback -- --nocapture
cargo test --test presentation_app -- --nocapture
cargo test --all-targets
```

```bash
git add src/presentation/battlefield.rs src/presentation/playback.rs src/app.rs tests/presentation_app.rs
git commit -m "feat: polish board-first combat feedback"
```

---

### Task 5: Run the clean seven-mission playthrough and tune only evidenced values

**Files:**
- Create: `docs/validation/hpa-386.md`
- Modify only if evidence requires it: `src/mission/mission_one.rs`
- Modify only if evidence requires it: `src/mission/mission_two.rs`
- Modify only if evidence requires it: `src/mission/mission_three.rs`
- Modify only if evidence requires it: `src/mission/mission_four.rs`
- Modify only if evidence requires it: `src/mission/mission_five.rs`
- Modify only if evidence requires it: `src/mission/mission_six.rs`
- Modify only if evidence requires it: `src/mission/mission_seven.rs`
- Modify only if evidence requires it: `src/mission/squad.rs`
- Modify only if evidence requires it: `src/campaign/progression.rs`
- Add targeted tests beside any changed authored rule/value

**Interfaces:**
- Consumes: complete Tasks 1–4 gameplay/presentation flow.
- Produces: measured full-campaign evidence and only the smallest authored tuning justified by it.

- [ ] **Step 1: Create the validation ledger before playing**

Start `docs/validation/hpa-386.md` with:

```markdown
# HPA-386 validation

## Campaign playthrough

| Mission | Minutes | Rounds | Restarts | Bonus? | Intent manipulation materially rewarded? | Notes / tuning |
| --- | ---: | ---: | ---: | --- | --- | --- |
| 1 | | | | | | |
| 2 | | | | | | |
| 3 | | | | | | |
| 4 | | | | | | |
| 5 | | | | | | |
| 6 | | | | | | |
| 7 | | | | | | |

## Progression ledger

| Transition | Credits before | Reward | Upgrade purchases | Credits after |
| --- | ---: | ---: | --- | ---: |
| New Game -> M1 | 0 | | | |
| M1 -> M2 | | | | |
| M2 -> M3 | | | | |
| M3 -> M4 | | | | |
| M4 -> M5 | | | | |
| M5 -> M6 | | | | |
| M6 -> M7 | | | | |
| M7 -> Ending | | | | |
```

Also add headings for total time, boss-threshold timing, telegraph readability, and presentation observations.

- [ ] **Step 2: Start from a clean save and play only through the real UI flow**

Use New Game and the real story/briefing/battle/aftermath/upgrade/Continue paths. Do not seed later missions for the acceptance timing run.

Record wall-clock minutes and rounds immediately after each mission.

- [ ] **Step 3: For each mission, record one concrete intent-manipulation verdict**

A mission counts only if reading/manipulating a committed enemy intent materially changes the tactical outcome. Record the concrete move/event; do not mark `yes` because telegraphs merely existed.

Final acceptance requires at least 4 of 7 `yes` rows.

- [ ] **Step 4: Validate Mission 7's corrected geometry manually**

Confirm in the rendered game:

```text
Regent Command Barrage footprint contains explosive (3,7) and empty push cell (5,7)
Vector Pulse moves Controller into (5,7)
explosive remains separately visible at (3,7)
Regent barrage hits the displaced Controller and triggers the explosive
telegraph remains readable at final encounter density
```

If this fails, tune the authored Mission 7 coordinates. Do not change occupancy semantics.

- [ ] **Step 5: Validate feedback polish**

During normal attacks and both boss fights verify:

```text
existing impact mesh still appears
attacker motion is short and does not leave transforms scaled
floating damage UI number appears near the target and clears
boss shake is modest and camera returns exactly to rest
no UI number requires/creates Camera2d
```

If projected numbers are visually poor, tune only their UI offset/font size/rise distance. Do not introduce a new rendering stack.

- [ ] **Step 6: Decide whether tuning is required from evidence**

Use this order only:

```text
1. placement/opening geometry
2. enemy count
3. boss HP/threshold timing
4. authored round pressure
5. weapon values
6. upgrade/reward values
```

For every changed value, write the measured problem and the before/after value into the ledger.

- [ ] **Step 7: Add a regression test for every tuning change that can regress mechanically**

Examples:

```text
changed turn pressure -> pin objective round
changed boss HP -> pin max HP and exact threshold boundary
changed opening position -> pin exact row + opening legality
changed reward/cost -> pin campaign math
```

Do not add tests merely for subjective pacing copy.

- [ ] **Step 8: Re-run affected tests after each tuning edit**

For mission changes:

```bash
cargo test --lib mission::mission_<name> -- --nocapture
```

For progression changes:

```bash
cargo test --test campaign_model -- --nocapture
cargo test --test campaign_persistence -- --nocapture
```

Then after all tuning:

```bash
cargo test --all-targets
```

- [ ] **Step 9: Commit the evidence/tuning pass**

If values changed:

```bash
git add docs/validation/hpa-386.md src/mission src/campaign tests
git commit -m "balance: tune the seven-mission MVP from playtest"
```

If no values changed:

```bash
git add docs/validation/hpa-386.md
git commit -m "docs: record HPA-386 campaign playtest"
```

---

### Task 6: Close out MVP documentation and CI-equivalent gates

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md` only where current campaign/mission notes are now stale
- Modify: `docs/validation/hpa-386.md`
- No source changes unless a gate exposes a concrete regression

**Interfaces:**
- Consumes: final implementation and Task 5 evidence.
- Produces: current seven-mission docs and recorded closeout gate evidence.

- [ ] **Step 1: Update player-facing/project docs to the final state**

README must state:

```text
7 authored missions
6 regular enemy archetypes + 2 bosses
Mission 7 -> Campaign Complete -> Return to Title
Continue on a completed save reopens Ending
board-first battle presentation only
no New Game+, mission select, or dedicated battle-animation scene
```

Update any old text saying Seven is an unauthored handoff.

- [ ] **Step 2: Add the final automated gate section to the validation ledger**

Add headings for these commands and record the actual command output/result when run:

```markdown
## Final automated gates

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets`
- `cargo llvm-cov --all-targets --lcov --output-path lcov.info`
- `cargo build --release`
- PR `Build + lint`
- PR `Unit test`
```

Record the actual test count printed by `cargo test --all-targets`; do not pre-fill or guess it in the plan.

- [ ] **Step 3: Run the local CI-equivalent gates fresh**

Run exactly:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Every command must exit 0 before recording PASS.

- [ ] **Step 4: Re-check the acceptance facts against code + ledger**

Verify explicitly:

```text
Mission Seven is authored and unlocks None
Campaign completion is persisted once
all seven missions have primary + optional objectives and VN context
regular roster = six; bosses = two
no battle-animation scene exists
clean New Game -> Ending playthrough recorded
measured first-playthrough time recorded
>=4/7 intent-manipulation rows are yes
base-only 3300-before-Seven progression test passes
Regent 27/26 and Dreadnought 21/20 tests pass
corrected Controller landing (5,7) and explosive (3,7) regression passes
no Text2d/Camera2d added for damage numbers
```

- [ ] **Step 5: Record PR CI only after the current head has finished**

Check the PR's current-head `Build + lint` and `Unit test`. If a later review-fix commit changes source/tests, do not claim the earlier CI run validates the new head.

- [ ] **Step 6: Commit documentation closeout**

```bash
git add README.md CLAUDE.md docs/validation/hpa-386.md
git commit -m "docs: close out the Scorpius MVP"
```

- [ ] **Step 7: Final branch verification**

After the closeout commit, re-run at minimum:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Do not mark HPA-386 Done or make the PR ready-for-review until the final head's required gates and validation evidence are current.

---

## Plan self-review checklist

Before implementation starts, this plan must remain internally consistent on these points:

- Mission 7 is 9×9 everywhere; no y=9 coordinate exists.
- Explosive is `(3,7)`; Controller push destination is `(5,7)`; they are never the same cell.
- `is_open_for` / `resolve_push` are not implementation files for Mission 7 geometry.
- Seed 2 public regression goes through real `BattleState::attack` and `resolve_enemy_phase()`.
- Regent authored IDs are 71/209–210; Task 1 fixture IDs 92/292–293 stay test-only.
- `MissionDefinition.unlocks` becomes `Option<MissionId>` once in Task 2; later tasks use only the new type.
- `CampaignState.completed` is explicit in all direct fixtures; no migration/default is introduced.
- `GameScreen::Ending` replaces `NextMission`; later tasks use only `Ending`.
- Regent's temporary scene 13 mapping exists only between Tasks 1 and 3; final mapping is 14.
- Damage numbers are Bevy UI `Text` under `HudRoot`, projected by the existing Camera3d; no `Text2d` or `Camera2d` is added.
- Existing 3D `DamageApplied` impact effect remains intact.
- Every Regent exhaustive arm required by `HudSnapshot::can_pilot`, `HudSnapshot::pilot_label`, and `CommandAction::PilotSkill` is explicit in Task 1.
- No step creates a second PR.