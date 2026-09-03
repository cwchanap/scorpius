# HPA-386 Mission 7 and MVP Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 7, the Regent final boss, stable once-only campaign completion/Ending, focused board-first feedback improvements, and one evidence-driven seven-mission MVP tuning pass in the same HPA-386 PR.

**Architecture:** Reuse Mission 6's half-HP weapon-slot seam for the second boss. Author Mission 7 as typed Rust content on the existing 9×9 board convention and current push/explosive/hazard rules. Replace the unauthored-Seven sentinel with `MissionDefinition.unlocks: Option<MissionId>` plus one persisted `CampaignState.completed` bit. Presentation changes stay on the existing event playback, Camera3d, 3D impact mesh, and HUD UI `Text` paths.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, Cargo tests, existing Bevy `App` integration tests, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-02-hpa-386-mission-7-mvp-closeout-design.md`

## Global Constraints

- One HPA-386 ticket = one PR. Continue on `jack65786656/hpa-386-scorpius-m2-author-mission-7-and-finish-mvp`.
- Keep exactly three playable mechs, six regular enemy archetypes, and two bosses.
- Regent is one normal single-cell `UnitState`; no boss runtime, stored phase, threshold registry, scripting, parts, invulnerability, multi-tile collision, or resistance.
- Dreadnought and Regent share only the half-HP slot selector in `unit_weapon`; committed intents remain immutable.
- Regent: HP52 / Armor4 / Move2 / Accuracy92 / Evasion8 / EN0 / Initiative45.
- Command Barrage: range3–6 / Cross1 / damage9 / hit+10 / crit5% / no push.
- Rupture Beam: range2–4 / Single / damage12 / hit+15 / crit10% / no push.
- Mission 7 is 9×9. Controller push landing is `(5,7)`; explosive is separately at `(3,7)`. Never change `is_open_for` or `resolve_push` to allow standing on a live explosive.
- Mission Seven is authored and terminal: One–Six `unlocks: Some(next)`, Seven `unlocks: None`.
- Persist exactly one new field: `CampaignState.completed: bool`. No save migration/default/versioning.
- Rename `GameScreen::NextMission` to `Ending`.
- Skip audio.
- Damage numbers use existing screen-space Bevy UI `Text` under `HudRoot`, projected through the existing Camera3d. Do not add `Text2d` or `Camera2d`.
- Existing 3D `DamageApplied` impact mesh stays in place.
- Keep `grid_to_world` unchanged; Mission 7 fits the current 9×9 centering.
- Regent visual is scene 14 in the existing `assets/models/mission_one.gltf`; final counts 15 scenes / 84 nodes / 15 meshes / 15 materials / 1 buffer.
- Tune authored values only from recorded playtest evidence.
- No new dependency/crate, generic boss/objective/status/AI/narrative framework, seventh regular enemy, new playable mech, new progression track, New Game+, analytics, second glTF, or second PR.

---

### Task 1: Add Regent as the second half-HP boss consumer

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/interaction.rs`

**Interfaces:**
- Consumes: Dreadnought `unit_weapon`, `attack_band_destination`, `initiative`, `HudSnapshot`, `execute_command`.
- Produces: `UnitArchetype::Regent`, shared half-HP selection, initiative45, ordinary enemy UI/pilot behavior.

- [ ] **Step 1: Add the failing exact-threshold regression**

In `src/domain/enemy.rs` tests, add Regent fixture IDs:

```rust
const REGENT: UnitId = UnitId(92);
const REGENT_PLAYER: UnitId = UnitId(93);
const COMMAND_BARRAGE: WeaponId = WeaponId(292);
const RUPTURE_BEAM: WeaponId = WeaponId(293);
```

Build Regent with `squad::unit(... stats(52, 4, 2, 92, 8, 0) ...)` and the two locked weapons from Global Constraints. Add:

```rust
#[test]
fn both_bosses_switch_at_their_exact_half_hp_boundary() {
    let mut regent = regent_threshold_fixture();
    regent.apply_direct_damage(REGENT, 25, DamageSource::Collision);
    assert_eq!(regent.unit(REGENT).unwrap().hp, 27);
    assert_eq!(unit_weapon(&regent, regent.unit(REGENT).unwrap()).unwrap().id, COMMAND_BARRAGE);
    regent.apply_direct_damage(REGENT, 1, DamageSource::Collision);
    assert_eq!(regent.unit(REGENT).unwrap().hp, 26);
    assert_eq!(unit_weapon(&regent, regent.unit(REGENT).unwrap()).unwrap().id, RUPTURE_BEAM);

    let mut dreadnought = dreadnought_threshold_fixture();
    dreadnought.apply_direct_damage(DREADNOUGHT, 19, DamageSource::Collision);
    assert_eq!(dreadnought.unit(DREADNOUGHT).unwrap().hp, 21);
    assert_eq!(unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap()).unwrap().id, GRAVITON);
    dreadnought.apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
    assert_eq!(dreadnought.unit(DREADNOUGHT).unwrap().hp, 20);
    assert_eq!(unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap()).unwrap().id, OVERLOAD);
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
    assert_eq!(battle.intent_for(REGENT).unwrap(), &committed);

    let future = build_intent(&battle, REGENT, Some(GridPos::new(3, 5))).unwrap();
    assert_eq!(future.profile.weapon, RUPTURE_BEAM);
}
```

- [ ] **Step 3: Verify red**

```bash
cargo test --lib regent -- --nocapture
```

Expected: compile failure because Regent does not exist.

- [ ] **Step 4: Implement the minimal shared boss seam**

Append `Regent` to `UnitArchetype` and change `unit_weapon` to:

```rust
let index = match unit.archetype {
    UnitArchetype::Dreadnought | UnitArchetype::Regent
        if unit.hp * 2 <= unit.stats.max_hp => 1,
    _ => 0,
};
```

Extend the normal attack-band arm with `UnitArchetype::Regent` and add Regent initiative45.

- [ ] **Step 5: Make every exhaustive presentation/pilot arm explicit**

In `src/presentation/battlefield.rs`, temporarily map Regent to scene13 until Task 3:

```rust
UnitArchetype::Regent => 13,
```

In both `HudSnapshot::can_pilot` and `HudSnapshot::pilot_label`, add Regent to the enemy-only arm. In `CommandAction::PilotSkill`, add Regent to the `PilotSkillWrongUnit` arm.

The expected forms are:

```rust
| UnitArchetype::Dreadnought
| UnitArchetype::Regent => false,
```

```rust
| UnitArchetype::Dreadnought
| UnitArchetype::Regent => "[P] PILOT",
```

```rust
| UnitArchetype::Dreadnought
| UnitArchetype::Regent => {
    return Err(BattleError::PilotSkillWrongUnit(unit_id));
}
```

Keep the existing regular-enemy variants in those same arms.

- [ ] **Step 6: Extend initiative coverage**

In `initiative_is_fixed_per_archetype_without_position`, construct Regent and pin:

```rust
assert_eq!(initiative(&regent), 45);
assert!(initiative(&regent) > initiative(&dreadnought));
assert!(initiative(&regent) > initiative(&controller));
```

- [ ] **Step 7: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib regent
cargo test --all-targets

git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add Regent boss behavior"
```

---

### Task 2: Author Mission 7 and add stable terminal campaign state

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
- Consumes: Task 1 Regent, regular enemy factories, `build_player_squad`, current campaign/session flow.
- Produces: `MISSION_SEVEN_DEFINITION`, legal seed2 final encounter, `unlocks: Option<MissionId>`, `completed`, `CampaignComplete`, `Ending`.

This is one coordinated task because Seven changes from an unauthored sentinel into a playable terminal mission.

- [ ] **Step 1: Write the final-completion regression first**

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

Update every direct `CampaignState { ... }` fixture to set `completed` explicitly.

- [ ] **Step 2: Write the Mission 7 board/opening tests before the factory**

Use authored IDs:

```rust
pub const REGENT: UnitId = UnitId(71);
pub const ARTILLERY: UnitId = UnitId(72);
pub const CONTROLLER: UnitId = UnitId(73);
pub const BULWARK: UnitId = UnitId(74);
pub const FLANKER: UnitId = UnitId(75);
pub const COMMAND_BARRAGE: WeaponId = WeaponId(209);
pub const RUPTURE_BEAM: WeaponId = WeaponId(210);
```

Pin the 9×9 board:

```rust
assert_eq!((battle.board().width(), battle.board().height()), (9, 9));
assert_eq!(
    battle.board().blocking_cells().collect::<Vec<_>>(),
    vec![
        GridPos::new(2, 4), GridPos::new(6, 4),
        GridPos::new(2, 5), GridPos::new(6, 5),
    ]
);
assert_eq!(
    battle.board().hazard_cells().collect::<Vec<_>>(),
    vec![GridPos::new(3, 5), GridPos::new(5, 5)]
);
assert_eq!(battle.board().explosive_at(GridPos::new(3, 7)).unwrap().hp, 4);
```

Pin opening rows:

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

If this fails, fix authored coordinates. Do not change generic movement/opening rules.

- [ ] **Step 3: Write the public seed2 manipulation regression before implementation is considered green**

After `begin_round()`, assert Regent `Command Barrage` contains both distinct cells:

```rust
let regent_intent = battle.intent_for(ids::REGENT).unwrap().clone();
assert_eq!(regent_intent.profile.weapon, ids::COMMAND_BARRAGE);
assert!(regent_intent.footprint.contains(&GridPos::new(3, 7)));
assert!(regent_intent.footprint.contains(&GridPos::new(5, 7)));
assert!(battle.board().has_live_explosive(GridPos::new(3, 7)));
assert!(!battle.board().has_live_explosive(GridPos::new(5, 7)));
```

Run real player actions:

```text
Vanguard    (4,7) -> (4,5)
Gunner      (3,8) -> (2,8)
Interceptor (5,8) -> (7,7)
Vector Pulse Controller (6,7) -> (5,7)
```

Pin the push:

```rust
assert!(pulse_events.iter().any(|event| matches!(
    event,
    BattleEvent::UnitPushed { unit, to, .. }
        if *unit == ids::CONTROLLER && *to == GridPos::new(5, 7)
)));
```

Finish reactions and call normal `resolve_enemy_phase()`. With seed2, pin Regent's ordinary hit and the separate prop trigger:

```rust
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::AttackRolled {
        attacker,
        weapon,
        target,
        roll: 52,
        hit: true,
        critical: false,
        ..
    } if *attacker == ids::REGENT
        && *weapon == ids::COMMAND_BARRAGE
        && *target == ids::CONTROLLER
)));
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::ExplosionTriggered { position, .. }
        if *position == GridPos::new(3, 7)
)));
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::IntentCanceled { attacker }
        if *attacker == ids::CONTROLLER
)));
```

This test must stay on `BattleState::attack` + `resolve_enemy_phase`; no seed sweep or direct push shortcut.

- [ ] **Step 4: Verify red**

```bash
cargo test --lib mission::mission_seven -- --nocapture
cargo test --test campaign_persistence final_mission_completion_is_persisted_and_idempotent -- --nocapture
```

Expected: Mission Seven/terminal fields do not exist yet.

- [ ] **Step 5: Change mission terminality atomically**

In `MissionDefinition`:

```rust
pub unlocks: Option<MissionId>,
```

Register Seven and update all definitions:

```text
One   -> Some(Two)
Two   -> Some(Three)
Three -> Some(Four)
Four  -> Some(Five)
Five  -> Some(Six)
Six   -> Some(Seven)
Seven -> None
```

`mission_definition(MissionId::Seven)` now returns `Some(&MISSION_SEVEN_DEFINITION)`.

- [ ] **Step 6: Add exactly-once persisted completion**

In `CampaignState`:

```rust
pub completed: bool,
```

`new_game()` sets false. Add `CampaignError::CampaignComplete` and check it before reward mutation.

After rewards:

```rust
match definition.unlocks {
    Some(next) => self.next_mission = next,
    None => self.completed = true,
}
```

Do not add compatibility defaults or migrations.

- [ ] **Step 7: Implement Mission 7 exactly**

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
        GridPos::new(2, 4), GridPos::new(6, 4),
        GridPos::new(2, 5), GridPos::new(6, 5),
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

Regent uses local weapons 209/210 and `stats(52, 4, 2, 92, 8, 0)`. Escorts use existing factories.

Dialogue arrays are exact and reuse existing portraits:

```rust
static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "The last command node is ahead. The Regent is broadcasting firing solutions to everything still standing.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Then we make its final order point the wrong way.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Break the Regent. Once the command net drops, Relay Nine is ours.",
        portrait: "vn/control_alert.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "Regent down. The remaining signatures are scattering.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Relay Nine is secure. Bring everyone home.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Copy. Mission complete.",
        portrait: "vn/vanguard_neutral.png",
    },
];
```

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
    pre_mission: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &PRE_MISSION_LINES,
    },
    aftermath: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &AFTERMATH_LINES,
    },
};
```

- [ ] **Step 8: Replace sentinel UI with Ending**

Rename `GameScreen::NextMission` to `Ending`, and rename `setup_next_mission_screen` / `next_mission_copy` to `setup_ending_screen` / `ending_copy`.

Continue routing:

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

After final aftermath, route to Ending when `runtime.0.state.completed`; otherwise route to Upgrade. `Proceed` from Upgrade always goes to `PreMissionStory` because Seven is now authored.

- [ ] **Step 9: Update campaign blast-radius tests**

Pin:

```rust
assert_eq!(mission_definition(MissionId::Six).unwrap().unlocks, Some(MissionId::Seven));
assert_eq!(mission_definition(MissionId::Seven).unwrap().unlocks, None);
```

Base credits before Seven remain:

```rust
let base_before_seven: u32 = [
    MissionId::One,
    MissionId::Two,
    MissionId::Three,
    MissionId::Four,
    MissionId::Five,
    MissionId::Six,
]
.into_iter()
.map(|id| mission_definition(id).unwrap().base_reward)
.sum();
assert_eq!(base_before_seven, 3300);
```

Integration routing must pin:

```text
unfinished Seven Continue -> Upgrade
unfinished Seven Proceed -> PreMissionStory
completed Continue -> Ending
final aftermath -> Ending
Ending -> Title
```

Persistence must round-trip both `completed: false` and `completed: true`.

- [ ] **Step 10: Verify Task 2 with the geometry test first**

```bash
cargo fmt --check
cargo test --lib mission::mission_seven -- --nocapture
cargo test --test campaign_persistence final_mission_completion_is_persisted_and_idempotent -- --nocapture
cargo test --test campaign_flow -- --nocapture
cargo test --test presentation_app -- --nocapture
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

The Mission 7 manipulation regression must pass without modifying `src/domain/battle.rs` or `src/domain/environment.rs`.

- [ ] **Step 11: Commit**

```bash
git add src/mission src/campaign src/presentation/campaign_ui.rs src/presentation/interaction.rs src/app.rs tests/campaign_model.rs tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs
git commit -m "feat: author Mission 7 and campaign ending"
```

---

### Task 3: Append the Regent glTF scene

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`

**Interfaces:**
- Consumes: existing single-buffer append pattern and Task 1 temporary scene13 mapping.
- Produces: Regent scene14 and final 15/84/15/15/1 asset counts.

- [ ] **Step 1: Add the failing Regent asset test**

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
    assert_eq!(nodes[77]["children"], serde_json::json!([78, 79, 80, 81, 82, 83]));
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

Update existing global-count assertions to the same final counts.

- [ ] **Step 2: Verify red**

```bash
cargo test --lib presentation::assets::tests::regent_scene_is_authored_as_the_final_violet_boss -- --nocapture
```

Expected: current glTF has 14 scenes.

- [ ] **Step 3: Append the scene without a new asset pipeline**

Append scene14 -> root77 -> part nodes78–83, mesh14, material14. Reuse current cube POSITION/NORMAL accessors and the one buffer. Material is `Regent Violet`, `[0.42, 0.14, 0.78, 1.0]`.

Set:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 15;
```

and final mapping:

```rust
UnitArchetype::Regent => 14,
```

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::assets
cargo test --all-targets

git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: add Regent battlefield visual"
```

---

### Task 4: Extend existing playback/UI for final combat feedback

**Files:**
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/playback.rs`
- Modify: `tests/presentation_app.rs` only if the existing integration-test module is the clearer home for the UI-entity assertion

**Interfaces:**
- Consumes: `EventPlayback`, `UnitVisual`, `EventEffect`, `HudRoot`, Camera3d, `grid_to_world`.
- Produces: attacker pulse, projected damage-number UI `Text`, boss camera shake with exact restore. Existing 3D impacts remain.

- [ ] **Step 1: Add focused red tests for pure transform/copy behavior**

```rust
fn damage_number_text(amount: i16) -> String {
    format!("-{amount}")
}

fn attack_scale(progress: f32) -> f32 {
    let pulse = (progress * PI).sin();
    UNIT_SCALE * (1.0 + pulse * 0.10)
}
```

Pin `damage_number_text(7) == "-7"`, scale at progress0/1 equals `UNIT_SCALE`, midpoint is larger.

Add:

```rust
fn boss_camera_transform(rest: Transform, progress: f32) -> Transform {
    let mut transform = rest;
    let pulse = (progress * PI).sin();
    transform.translation.x += pulse * 0.08;
    transform.translation.z -= pulse * 0.05;
    transform
}
```

Pin progress0 and 1 exactly equal rest, midpoint differs.

- [ ] **Step 2: Verify red**

```bash
cargo test --lib presentation::playback -- --nocapture
```

Expected: helpers/component do not exist.

- [ ] **Step 3: Tag the existing Camera3d, not a new camera**

In `battlefield.rs`:

```rust
#[derive(Component, Clone, Copy)]
pub(crate) struct BattleCamera {
    pub rest: Transform,
}
```

Use the existing camera configuration exactly:

```rust
let rest = Transform::from_xyz(10.8, 12.4, 12.2).looking_at(Vec3::ZERO, Vec3::Y);
commands.spawn((
    Camera3d::default(),
    MeshPickingCamera,
    Projection::from(OrthographicProjection {
        scaling_mode: ScalingMode::FixedVertical {
            viewport_height: 12.8,
        },
        ..OrthographicProjection::default_3d()
    }),
    rest,
    BattleCamera { rest },
));
```

- [ ] **Step 4: Add attacker pulse without deleting target feedback**

For `AttackRolled`, apply `attack_scale(progress)` to the attacker visual. Preserve the existing hit-target pulse, `DamageApplied` shake, KO shrink, and counter pulse.

- [ ] **Step 5: Keep `spawn_event_effect` intact and add a separate HUD-number helper**

Do not replace its `DamageApplied` impact branch.

Add:

```rust
#[derive(Component)]
struct DamageNumberEffect {
    origin: Vec2,
}
```

Add a helper that takes an already-projected viewport position and spawns actual Bevy UI:

```rust
fn spawn_damage_number(
    commands: &mut Commands,
    hud_root: Entity,
    viewport: Vec2,
    amount: i16,
) {
    commands.spawn((
        Text::new(damage_number_text(amount)),
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
}
```

When `DamageApplied` starts, project the target through the existing 3D battle camera:

```rust
let viewport = camera.world_to_viewport(
    camera_global_transform,
    grid_to_world(target.position) + Vec3::Y * 0.8,
)?;
```

Then call `spawn_damage_number`. Animate `Node.top` from `origin.y` to `origin.y - 24.0` over the current event fraction and despawn on event completion.

No `Text2d`, `Camera2d`, font asset, or second damage calculation.

- [ ] **Step 6: Test the actual UI entity helper**

Create a minimal `App`/World with a `HudRoot`, call `spawn_damage_number(..., Vec2::new(320.0, 240.0), 7)`, apply deferred commands, and assert one child entity has:

```text
Text("-7")
DamageNumberEffect { origin: (320,240) }
Node.position_type == Absolute
```

The Task 5 rendered playtest validates that the real `world_to_viewport` placement is visually correct.

- [ ] **Step 7: Add boss-only camera emphasis and exact restoration**

During `AttackRolled`, inspect attacker archetype. For Dreadnought or Regent use:

```rust
*camera_transform = boss_camera_transform(camera.rest, timer.fraction());
```

For every non-boss event and after event completion:

```rust
*camera_transform = camera.rest;
```

- [ ] **Step 8: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::playback -- --nocapture
cargo test --test presentation_app -- --nocapture
cargo test --all-targets

git add src/presentation/battlefield.rs src/presentation/playback.rs tests/presentation_app.rs
git commit -m "feat: polish board-first combat feedback"
```

---

### Task 5: Run the clean seven-mission playthrough and tune only evidenced values

**Files:**
- Create: `docs/validation/hpa-386.md`
- Modify authored mission/squad/progression files only when the ledger demonstrates a concrete issue
- Add a targeted regression beside every mechanical tuning change

**Interfaces:**
- Consumes: complete Tasks 1–4 flow.
- Produces: measured full-campaign evidence and only justified authored tuning.

- [ ] **Step 1: Create the validation ledger before playing**

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

Add sections for total time, boss-threshold timing, telegraph readability, and presentation observations.

- [ ] **Step 2: Play New Game -> Ending through the real UI**

Do not seed later missions for the acceptance timing run. Record wall-clock minutes, rounds, restarts, credits, purchases, and bonus result immediately after each mission.

- [ ] **Step 3: Record intent-manipulation evidence per mission**

A `yes` requires a concrete move/event where reading or manipulating committed intent materially changes the outcome. At least 4 of 7 rows must be `yes` for acceptance.

- [ ] **Step 4: Validate the corrected Mission 7 geometry manually**

Confirm:

```text
Regent footprint contains explosive (3,7) and empty push cell (5,7)
Vector Pulse moves Controller to (5,7)
explosive remains at (3,7)
Regent barrage damages Controller and triggers explosive
telegraphs remain readable
```

If this fails, tune authored Mission 7 coordinates only.

- [ ] **Step 5: Validate presentation polish**

Confirm:

```text
existing impact mesh still appears
damage UI number appears near target and clears
attacker pulse does not leave stale scale
boss shake is modest and returns exactly to rest
no Camera2d/Text2d exists in battle presentation
```

If number placement is poor, tune only UI offset/font/rise distance.

- [ ] **Step 6: Tune only from recorded evidence**

Use this order:

```text
1. placement/opening geometry
2. enemy count
3. boss HP/threshold timing
4. authored round pressure
5. weapon values
6. upgrade/reward values
```

For every mechanical tuning edit, add a focused regression and then run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

- [ ] **Step 7: Commit the playtest pass**

If tuning changed source/tests:

```bash
git add docs/validation/hpa-386.md src/mission src/campaign tests
git commit -m "balance: tune the seven-mission MVP from playtest"
```

If no source tuning was required:

```bash
git add docs/validation/hpa-386.md
git commit -m "docs: record HPA-386 campaign playtest"
```

---

### Task 6: Close out MVP docs and final gates

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md` only where campaign notes are stale
- Modify: `docs/validation/hpa-386.md`

**Interfaces:**
- Consumes: final implementation + Task 5 evidence.
- Produces: current project docs and fresh final-head gate evidence.

- [ ] **Step 1: Update docs to final campaign state**

Document:

```text
7 authored missions
6 regular enemies + 2 bosses
Mission 7 -> Campaign Complete -> Return to Title
completed Continue -> Ending
board-first battle presentation
no mission select, NG+, or dedicated battle-animation scene
```

Remove text saying Seven is an unauthored handoff.

- [ ] **Step 2: Add the final gate ledger headings**

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

Record actual output/results after running; do not guess the final test count.

- [ ] **Step 3: Run local CI-equivalent gates fresh**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Every command must exit 0 before the ledger says PASS.

- [ ] **Step 4: Re-check acceptance facts**

Verify against code + ledger:

```text
Seven authored and unlocks None
completion persisted exactly once
all seven missions have primary + optional objective + VN context
regular roster six; bosses two
New Game -> Ending recorded
measured first-playthrough time recorded
>=4/7 intent-manipulation rows yes
base-only 3300-before-Seven progression test passes
Regent 27/26 and Dreadnought 21/20 tests pass
Controller push (5,7) + explosive trigger (3,7) regression passes
no Text2d/Camera2d added
```

- [ ] **Step 5: Check PR CI on the final head**

Do not reuse an earlier CI result after a later source/test commit. Record `Build + lint` and `Unit test` only for the current final head.

- [ ] **Step 6: Commit docs closeout and re-run final minimum gates**

```bash
git add README.md CLAUDE.md docs/validation/hpa-386.md
git commit -m "docs: close out the Scorpius MVP"

cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Do not mark HPA-386 Done or make the PR ready-for-review until the final head's validation evidence and required gates are current.

---

## Plan self-review checklist

- Mission 7 is 9×9 everywhere; no y=9 coordinate exists.
- Explosive `(3,7)` and Controller landing `(5,7)` are distinct committed-footprint cells.
- `src/domain/battle.rs` and `src/domain/environment.rs` are not Mission 7 implementation files.
- Seed2 regression uses real `BattleState::attack` + `resolve_enemy_phase`.
- Dreadnought 21/20 and Regent 27/26 thresholds are tested together.
- Task 1 explicitly lists `HudSnapshot::can_pilot`, `HudSnapshot::pilot_label`, and `CommandAction::PilotSkill` Regent arms.
- `unlocks` becomes `Option<MissionId>` once in Task 2.
- `completed` is explicit in direct fixtures; no compatibility default exists.
- `Ending` replaces `NextMission` after Task 2.
- Regent maps temporarily to scene13 only until Task 3; final scene is14.
- Damage numbers are HUD UI `Text` plus existing Camera3d projection; no `Text2d`/`Camera2d`.
- Existing 3D impact effect remains.
- No step creates a second PR.