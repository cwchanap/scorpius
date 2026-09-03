# HPA-386 Mission 7 and MVP Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 7, the Regent final boss, stable once-only campaign completion/Ending, focused board-first feedback improvements, and one evidence-driven seven-mission MVP tuning pass in the same HPA-386 PR.

**Architecture:** Reuse Mission 6's half-HP `unit_weapon` seam for the second boss. Author Mission 7 as typed Rust content on the existing 9x9 board convention and current push/explosive/hazard rules. Replace the unauthored-Seven sentinel with `MissionDefinition.unlocks: Option<MissionId>` plus one persisted `CampaignState.completed` bit. Keep presentation changes inside existing `EventPlayback`, Camera3d, 3D `EventEffect`, and HUD UI `Text` paths.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, Cargo tests, existing Bevy `App` integration tests, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-02-hpa-386-mission-7-mvp-closeout-design.md`

## Global Constraints

- One HPA-386 ticket = one PR. Continue on `jack65786656/hpa-386-scorpius-m2-author-mission-7-and-finish-mvp`.
- Keep three playable mechs, six regular enemy archetypes, and exactly two bosses.
- Regent is one normal single-cell `UnitState`; no boss runtime, stored phase, threshold registry, scripting, parts, invulnerability, multi-tile collision, or resistance.
- Dreadnought and Regent share only the half-HP slot selector in `unit_weapon`; committed intents remain immutable.
- Regent: HP52 / Armor4 / Move2 / Accuracy92 / Evasion8 / EN0 / Initiative45.
- Command Barrage: range3-6 / Cross1 / damage9 / hit+10 / crit5% / no push.
- Rupture Beam: range2-4 / Single / damage12 / hit+15 / crit10% / no push.
- Mission 7 is 9x9. Controller landing is `(5,7)`; explosive is separately `(3,7)`. Do not change `is_open_for` or `resolve_push`.
- Mission Seven is authored and terminal: One-Six `unlocks: Some(next)`, Seven `unlocks: None`.
- Persist exactly `CampaignState.completed: bool`; no save migration/default/versioning.
- Rename `GameScreen::NextMission` to `Ending`.
- Continue, final Aftermath, and Proceed all guard `completed` through one pure routing helper using the existing `CampaignUiAction` enum.
- Skip audio.
- Keep `EventEffect` 3D-only. Damage numbers are parallel HUD UI `Text` feedback with their own component/query/lifecycle inside `play_battle_events`.
- `world_to_viewport` failure skips the number only; `play_battle_events` remains non-`Result`.
- Reuse `text_font` from `presentation/ui.rs` by making it `pub(crate)`.
- Keep `grid_to_world` unchanged.
- Regent visual is scene14 in the existing glTF; final counts are 15 scenes / 84 nodes / 15 meshes / 15 materials / 1 buffer.
- Tune authored values only from recorded playtest evidence.
- No new dependency/crate, generic boss/objective/status/AI/narrative framework, seventh regular enemy, new playable mech, new progression track, New Game+, analytics, second glTF, asset pipeline, or second PR.

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

- [ ] **Step 1: Add failing exact-threshold coverage**

In `src/domain/enemy.rs` tests add fixture IDs:

```rust
const REGENT: UnitId = UnitId(92);
const REGENT_PLAYER: UnitId = UnitId(93);
const COMMAND_BARRAGE: WeaponId = WeaponId(292);
const RUPTURE_BEAM: WeaponId = WeaponId(293);
```

Build the Regent fixture with:

```rust
squad::unit(
    REGENT,
    "Regent",
    UnitArchetype::Regent,
    Faction::Enemy,
    squad::stats(52, 4, 2, 92, 8, 0),
    GridPos::new(3, 1),
    vec![COMMAND_BARRAGE, RUPTURE_BEAM],
)
```

and weapons:

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
```

```rust
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

Add one test that pins both bosses' boundaries:

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
        unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap())
            .unwrap()
            .id,
        GRAVITON
    );
    dreadnought.apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
    assert_eq!(dreadnought.unit(DREADNOUGHT).unwrap().hp, 20);
    assert_eq!(
        unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap())
            .unwrap()
            .id,
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

- [ ] **Step 4: Extend the existing selector and movement path**

Append `Regent` to `UnitArchetype`.

Change `unit_weapon` to:

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

Extend the attack-band branch:

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

Add `UnitArchetype::Regent => 45` to initiative.

- [ ] **Step 5: Make exhaustive presentation/pilot arms explicit**

Temporarily map Regent to scene13 until Task 3:

```rust
UnitArchetype::Regent => 13,
```

Add Regent to the enemy-only branches in both `HudSnapshot::can_pilot` and `HudSnapshot::pilot_label`, and to `CommandAction::PilotSkill` rejection.

Pin initiative ordering:

```rust
assert_eq!(initiative(&regent), 45);
assert!(initiative(&regent) > initiative(&dreadnought));
assert!(initiative(&regent) > initiative(&controller));
```

- [ ] **Step 6: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib regent
cargo test --all-targets

git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add Regent boss behavior"
```

---

### Task 2: Author Mission 7 and replace the Seven sentinel with stable completion

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
- Produces: `MISSION_SEVEN_DEFINITION`, legal seed2 final encounter, `unlocks: Option<MissionId>`, `completed`, `CampaignComplete`, `Ending`, and one pure `campaign_destination` helper.

This is one coordinated task because Seven changes from an unauthored sentinel into a playable terminal mission.

- [ ] **Step 1: Write final-completion and routing tests first**

Add:

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

In `campaign_ui.rs` tests, pin the future routing truth table for `campaign_destination`:

```text
Continue + completed -> Ending
Continue + unfinished One -> PreMissionStory
Continue + unfinished Seven -> Upgrade
AdvanceAftermath + completed -> Ending
AdvanceAftermath + unfinished -> Upgrade
Proceed + completed -> Ending
Proceed + unfinished Seven -> PreMissionStory
```

- [ ] **Step 2: Write Mission 7 board/opening tests before the factory**

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

Pin:

```rust
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

Also run `assert_opening_plan_is_legal(&mission_seven(1));`. If it fails, fix authored coordinates; do not change generic movement/opening rules.

- [ ] **Step 3: Copy Mission 6's public-path helper and add only the Gunner move**

Create `redirected_opening_ready_to_resolve()` in `mission_seven.rs` by copying the structure of Mission 6's helper. Keep the same public action/RNG order and add the Gunner move between Vanguard and Interceptor.

The sequence must be exactly:

```text
seed 2
begin_round
Vanguard (4,7) -> (4,5), Guard, finish
Gunner (3,8) -> (2,8), Guard, finish
Interceptor (5,8) -> (7,7)
Vector Pulse Controller at (6,7)
assert hit roll 11 and non-critical roll 27
assert Controller push (6,7) -> (5,7)
Interceptor Guard, finish
Controller HP == 6
```

Before player actions pin Regent's committed `Command Barrage` contains both `(3,7)` and `(5,7)`, that `(3,7)` holds a live explosive, and `(5,7)` does not.

Then add the resolution test using normal `resolve_enemy_phase()` and pin:

```rust
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::AttackRolled {
        attacker,
        weapon,
        target,
        roll: 52,
        hit: true,
        critical_roll: Some(37),
        critical: false,
        ..
    } if *attacker == ids::REGENT
        && *weapon == ids::COMMAND_BARRAGE
        && *target == ids::CONTROLLER
)));
```

```rust
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::ExplosiveDamaged { position, .. }
        if *position == GridPos::new(3, 7)
)));
```

```rust
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::ExplosionTriggered { position, .. }
        if *position == GridPos::new(3, 7)
)));
```

```rust
assert!(events.iter().any(|event| matches!(
    event,
    BattleEvent::IntentCanceled { attacker }
        if *attacker == ids::CONTROLLER
)));
```

Do not use a seed sweep, direct `resolve_push`, or test-only movement mutation to reconstruct the RNG sequence.

- [ ] **Step 4: Verify the intended red state**

```bash
cargo test --lib mission::mission_seven -- --nocapture
cargo test --test campaign_persistence final_mission_completion_is_persisted_and_idempotent -- --nocapture
```

Expected: Mission Seven/terminal fields do not exist yet.

- [ ] **Step 5: Change mission terminality and campaign state atomically**

Change `MissionDefinition`:

```rust
pub unlocks: Option<MissionId>,
```

Register Seven and update definitions:

```text
One   -> Some(Two)
Two   -> Some(Three)
Three -> Some(Four)
Four  -> Some(Five)
Five  -> Some(Six)
Six   -> Some(Seven)
Seven -> None
```

Add:

```rust
pub completed: bool,
```

to `CampaignState`, set false in `new_game`, add `CampaignError::CampaignComplete`, and check it before reward mutation.

After awarding rewards:

```rust
match definition.unlocks {
    Some(next) => self.next_mission = next,
    None => self.completed = true,
}
```

No compatibility defaults or migrations.

- [ ] **Step 6: Implement Mission 7 exactly**

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
    EnemyOpening {
        unit: ids::REGENT,
        destination: GridPos::new(4, 2),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::ARTILLERY,
        destination: GridPos::new(2, 2),
        target: Some(ids::GUNNER),
    },
    EnemyOpening {
        unit: ids::CONTROLLER,
        destination: GridPos::new(6, 7),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::BULWARK,
        destination: GridPos::new(2, 6),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::FLANKER,
        destination: GridPos::new(1, 8),
        target: Some(ids::GUNNER),
    },
];
```

Regent unit:

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

Use existing factories for Artillery, Controller, Bulwark, and Flanker. Define the two local Regent weapons with the locked Global Constraints values.

Use exactly these dialogue arrays:

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
```

```rust
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
    title: "Mission 7 - Last Command",
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

- [ ] **Step 7: Add one pure terminal routing helper and use it in all three seams**

In `campaign_ui.rs` add:

```rust
fn campaign_destination(
    action: CampaignUiAction,
    state: &CampaignState,
) -> Option<GameScreen> {
    match action {
        CampaignUiAction::Continue if state.completed => Some(GameScreen::Ending),
        CampaignUiAction::Continue if state.next_mission == MissionId::One => {
            Some(GameScreen::PreMissionStory)
        }
        CampaignUiAction::Continue => Some(GameScreen::Upgrade),
        CampaignUiAction::AdvanceAftermath if state.completed => Some(GameScreen::Ending),
        CampaignUiAction::AdvanceAftermath => Some(GameScreen::Upgrade),
        CampaignUiAction::Proceed if state.completed => Some(GameScreen::Ending),
        CampaignUiAction::Proceed => Some(GameScreen::PreMissionStory),
        _ => None,
    }
}
```

Rename `GameScreen::NextMission` to `Ending` and the terminal screen setup/copy helpers accordingly.

After `continue_game` succeeds, route from the loaded state with `campaign_destination(CampaignUiAction::Continue, state)`.

For `AdvanceAftermath`, compute the destination from the persisted state before calling `advance_dialogue`; the destination is used only when the last aftermath line advances.

For `Proceed`, route with `campaign_destination(CampaignUiAction::Proceed, state)`. A completed save must go to Ending, never back into Mission 7 story.

- [ ] **Step 8: Update campaign blast-radius tests**

Pin:

```rust
assert_eq!(
    mission_definition(MissionId::Six).unwrap().unlocks,
    Some(MissionId::Seven)
);
assert_eq!(mission_definition(MissionId::Seven).unwrap().unlocks, None);
```

Pin base credits before Seven:

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

Integration routing must cover:

```text
unfinished Seven Continue -> Upgrade
unfinished Seven Proceed -> PreMissionStory
completed Continue -> Ending
completed AdvanceAftermath -> Ending
completed Proceed -> Ending
Ending -> Title
```

Persistence must round-trip both `completed: false` and `completed: true`.

- [ ] **Step 9: Verify Task 2 with risky tests first**

```bash
cargo fmt --check
cargo test --lib mission::mission_seven -- --nocapture
cargo test --test campaign_persistence final_mission_completion_is_persisted_and_idempotent -- --nocapture
cargo test --test campaign_flow -- --nocapture
cargo test --test presentation_app -- --nocapture
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

The Mission 7 public-path regression must pass without modifying `src/domain/battle.rs` or `src/domain/environment.rs`.

- [ ] **Step 10: Commit**

```bash
git add src/mission src/campaign src/presentation/campaign_ui.rs src/presentation/interaction.rs src/app.rs tests/campaign_model.rs tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs
git commit -m "feat: author Mission 7 and campaign ending"
```

---

### Task 3: Append the Regent glTF scene and update every global-count pin

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`

**Interfaces:**
- Consumes: existing one-buffer glTF append pattern and Task 1 temporary scene13 mapping.
- Produces: Regent scene14 and final 15/84/15/15/1 counts.

- [ ] **Step 1: Add the failing Regent scene test**

Add:

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

- [ ] **Step 2: Update every existing hard-coded global count in the same edit**

Change all old `14` scene/mesh/material and `77` node global assertions to the final values in these existing tests:

```text
flanker_scene_is_authored_with_own_mesh_material_and_root_scale
bulwark_and_controller_scenes_are_authored_with_own_meshes_and_roots
dreadnought_scene_is_authored_as_a_larger_crimson_unit
```

Keep their scene indices/node ranges unchanged. All four tests, including the new Regent test, must agree on:

```text
scenes 15
nodes 84
meshes 15
materials 15
buffers 1
```

- [ ] **Step 3: Verify red**

```bash
cargo test --lib presentation::assets -- --nocapture
```

Expected: current glTF has 14 scenes/77 nodes and no scene14.

- [ ] **Step 4: Append Regent using the existing buffer/accessors**

Append scene14 -> root77 -> parts78-83, mesh14, material14. Reuse existing cube POSITION/NORMAL accessors and the one buffer. Material is `Regent Violet`, base color `[0.42, 0.14, 0.78, 1.0]`.

Set:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 15;
```

and final mapping:

```rust
UnitArchetype::Regent => 14,
```

- [ ] **Step 5: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::assets
cargo test --all-targets

git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: add Regent battlefield visual"
```

---

### Task 4: Extend EventPlayback with parallel HUD damage-number lifecycle

**Files:**
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/playback.rs`
- Modify: `src/presentation/ui.rs`
- Test: `src/presentation/playback.rs`
- Test: `tests/presentation_app.rs` only if the existing App-level fixture is a clearer home for the Camera3d projection case

**Interfaces:**
- Consumes: `EventPlayback`, `EventEffect`, `UnitVisual`, `HudRoot`, `text_font`, Camera3d, `grid_to_world`.
- Produces: attacker pulse, parallel `DamageNumberEffect`, boss camera shake with exact restore. Existing 3D impact feedback remains unchanged.

- [ ] **Step 1: Add failing lifecycle helpers/tests**

Add:

```rust
#[derive(Component)]
struct DamageNumberEffect {
    origin: Vec2,
}
```

Add pure transform helpers:

```rust
fn attack_scale(progress: f32) -> f32 {
    let pulse = (progress * PI).sin();
    UNIT_SCALE * (1.0 + pulse * 0.10)
}
```

```rust
fn boss_camera_transform(rest: Transform, progress: f32) -> Transform {
    let mut transform = rest;
    let pulse = (progress * PI).sin();
    transform.translation.x += pulse * 0.08;
    transform.translation.z -= pulse * 0.05;
    transform
}
```

Pin start/end equal rest/base scale and midpoint differs.

Add a small `App` test for the actual damage-number lifecycle. Spawn a `HudRoot`, call the same damage-number spawn helper used by playback with viewport `(320,240)` and amount 7, run the same animation helper at fraction 0.5, then invoke the same cleanup helper called from the finished branch. Assert:

```text
Text is "-7"
Node.top moved above 240
DamageNumberEffect entity no longer exists after cleanup
```

Do not satisfy Task 4 with a string-only formatting test.

- [ ] **Step 2: Verify red**

```bash
cargo test --lib presentation::playback -- --nocapture
```

Expected: `DamageNumberEffect`, lifecycle helpers, and `BattleCamera` do not exist.

- [ ] **Step 3: Reuse the existing font helper**

Change in `src/presentation/ui.rs`:

```rust
pub(crate) fn text_font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}
```

Import `ui::{HudRoot, text_font}` in `playback.rs`. Do not create another font helper or font asset.

- [ ] **Step 4: Tag the existing Camera3d, not a new camera**

In `battlefield.rs` add:

```rust
#[derive(Component, Clone, Copy)]
pub(crate) struct BattleCamera {
    pub rest: Transform,
}
```

Spawn the existing camera with the same projection and a stored rest transform:

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

- [ ] **Step 5: Extend `play_battle_events` with a sibling UI query**

Keep `EventEffectQuery` unchanged and add a separate query type:

```rust
type DamageNumberQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static DamageNumberEffect, &'static mut Node),
>;
```

When a current event is active:

```text
tick timer
animate_unit_event
animate_effects on EventEffect only
animate_damage_numbers on DamageNumberEffect only
apply/restore boss camera
```

In the existing `if finished` branch:

```rust
for (entity, _) in &mut effects {
    commands.entity(entity).despawn();
}
for (entity, _, _) in &mut damage_numbers {
    commands.entity(entity).despawn();
}
playback.current = None;
```

Do not tag damage numbers as `EventEffect`; that would send a UI node through the 3D `Transform` animation path.

- [ ] **Step 6: Spawn a damage number in addition to the existing 3D impact**

Leave the existing call unchanged:

```rust
spawn_event_effect(&mut commands, root, &event, &battle, &mission_assets);
```

For `BattleEvent::DamageApplied { target, amount, .. }`, resolve the target position and call:

```rust
if let Ok(viewport) = camera.world_to_viewport(
    camera_global_transform,
    grid_to_world(target.position) + Vec3::Y * 0.8,
) {
    spawn_damage_number(&mut commands, hud_root, viewport, *amount);
}
```

`play_battle_events` remains `fn ... {}`. Do not use `?`; projection failure skips only the number.

The spawn helper must use:

```rust
commands.spawn((
    Text::new(format!("-{amount}")),
    text_font(28.0),
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

Animate only `Node.top`:

```rust
node.top = px(effect.origin.y - 24.0 * progress);
```

- [ ] **Step 7: Add attacker pulse and boss camera restore**

On `AttackRolled`, apply `attack_scale(progress)` to the attacker without deleting the existing target-hit pulse.

For Dreadnought/Regent attacks set:

```rust
*camera_transform = boss_camera_transform(camera.rest, timer.fraction());
```

For all non-boss events and after the current event finishes:

```rust
*camera_transform = camera.rest;
```

- [ ] **Step 8: Verify Task 4**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::playback -- --nocapture
cargo test --test presentation_app -- --nocapture
cargo test --all-targets
```

The focused lifecycle test must prove the damage-number entity is removed by the same cleanup helper used from `play_battle_events`' finished branch.

- [ ] **Step 9: Commit**

```bash
git add src/presentation/battlefield.rs src/presentation/playback.rs src/presentation/ui.rs tests/presentation_app.rs
git commit -m "feat: polish board-first combat feedback"
```

---

### Task 5: Run the clean seven-mission playthrough and tune only evidenced values

**Files:**
- Create: `docs/validation/hpa-386.md`
- Modify authored mission/squad/progression files only when the ledger demonstrates a concrete issue
- Add a targeted regression beside every mechanical tuning change

**Interfaces:**
- Consumes: complete Tasks 1-4 flow.
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

- [ ] **Step 3: Record concrete intent-manipulation evidence**

A `yes` requires a move/event where reading or manipulating committed intent materially changes the tactical outcome. At least 4 of 7 rows must be `yes`.

- [ ] **Step 4: Validate corrected Mission 7 geometry**

Confirm:

```text
Regent footprint contains explosive (3,7) and empty push cell (5,7)
Vector Pulse moves Controller to (5,7)
explosive remains at (3,7)
Regent barrage damages Controller
ExplosiveDamaged and ExplosionTriggered both name (3,7)
Controller intent cancels after knockout
```

If this fails, tune authored Mission 7 coordinates only.

- [ ] **Step 5: Validate campaign terminal guard**

Confirm through real UI and focused fixtures:

```text
final Aftermath -> Ending
completed Continue -> Ending
completed Proceed fixture -> Ending, never Mission 7 story
unfinished Seven Continue -> Upgrade
unfinished Seven Proceed -> PreMissionStory
```

- [ ] **Step 6: Validate EventPlayback feedback lifecycle**

Confirm:

```text
existing 3D impact mesh still appears
HUD damage number appears near target
number rises and disappears after its DamageApplied event
damage numbers do not accumulate
attacker pulse leaves no stale scale
boss shake is modest and camera returns exactly to rest
no battle Camera2d/Text2d exists
```

If number placement is poor, tune only UI offset/font/rise distance.

- [ ] **Step 7: Tune only from recorded evidence**

Use this order:

```text
1. placement/opening geometry
2. enemy count
3. boss HP/threshold timing
4. authored round pressure
5. weapon values
6. upgrade/reward values
```

For every mechanical tuning edit, add a focused regression and run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

- [ ] **Step 8: Commit the playtest pass**

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
no mission select, New Game+, or dedicated battle-animation scene
```

Remove text saying Seven is an unauthored handoff.

- [ ] **Step 2: Add final gate ledger headings**

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
Continue/Aftermath/Proceed completed routes all end at Ending
all seven missions have primary + optional objective + VN context
regular roster six; bosses two
New Game -> Ending recorded
measured first-playthrough time recorded
>=4/7 intent-manipulation rows yes
base-only 3300-before-Seven progression test passes
one level-2 track per mech costs 1800 total
Regent 27/26 and Dreadnought 21/20 tests pass
Controller push (5,7) + ExplosiveDamaged/ExplosionTriggered (3,7) pass
DamageNumberEffect lifecycle spawn/animate/despawn test passes
all old glTF global-count pins are 15/84/15/15/1
no Text2d/Camera2d added to battle presentation
```

- [ ] **Step 5: Check PR CI on the final head**

Do not reuse an earlier CI result after a later source/test commit. Record `Build + lint` and `Unit test` only for the current final head.

- [ ] **Step 6: Commit docs closeout and re-run minimum final gates**

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

## Plan Self-Review Checklist

- Mission 7 is 9x9 everywhere; no y=9 coordinate exists.
- Explosive `(3,7)` and Controller landing `(5,7)` are distinct committed-footprint cells.
- `src/domain/battle.rs` and `src/domain/environment.rs` are not Mission 7 implementation files.
- Mission 7 copies Mission 6's public helper order and adds only the Gunner move before the same Vector Pulse RNG sequence.
- Seed2 pins Vector Pulse hit11/noncrit27, Regent hit52/noncrit37.
- `ExplosiveDamaged` and `ExplosionTriggered` both pin `(3,7)`.
- Dreadnought 21/20 and Regent 27/26 thresholds are tested together.
- `unlocks` becomes `Option<MissionId>` once in Task 2.
- `completed` is explicit in direct fixtures; no compatibility default exists.
- One `campaign_destination` helper covers completed routing for Continue, AdvanceAftermath, and Proceed without a new routing enum.
- Completed Proceed goes to Ending, never back into Mission 7.
- Regent maps temporarily to scene13 only until Task 3; final scene is14.
- Every old hard-coded glTF global count is updated in Task 3.
- `EventEffect` remains 3D-only.
- `DamageNumberEffect` has a sibling query, Node animation, and shared finished-branch cleanup.
- `world_to_viewport` uses `if let Ok`; `play_battle_events` does not become `Result`.
- `text_font` is reused via `pub(crate)`.
- Damage-number tests cover cleanup, not only formatting.
- No step creates a second PR.