# HPA-386 Mission 7 and MVP Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 7, the Regent final boss, stable once-only campaign completion/ending, small board-first combat feedback improvements, and an evidence-driven seven-mission MVP tuning/validation pass in the same HPA-386 PR.

**Architecture:** Reuse the Mission 6 half-HP weapon-slot seam for one second boss archetype instead of adding a boss framework. Author Mission 7 through the existing mission/combat/environment vocabulary. Replace the old unauthored-Seven sentinel with `MissionDefinition.unlocks: Option<MissionId>` plus one persisted `CampaignState.completed` bit. Presentation work stays inside the current event playback and battlefield camera path.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus the existing Bevy `App` integration tests and GitHub Actions coverage/release gates.

**Spec:** `docs/superpowers/specs/2026-09-02-hpa-386-mission-7-mvp-closeout-design.md`

## Global Constraints

- One HPA-386 ticket = one PR. Continue implementation on `jack65786656/hpa-386-scorpius-m2-author-mission-7-and-finish-mvp`; do not split implementation into another PR.
- Keep exactly three playable mechs, six regular enemy archetypes, and two boss archetypes.
- Regent is one normal single-cell `UnitState`; no boss runtime, threshold registry, stored phase, scripting, parts, invulnerability, multi-tile collision, or resistance model.
- Both concrete bosses switch from weapon slot 0 to slot 1 at/below half HP through `unit_weapon`; committed intents never mutate after commitment.
- Regent values are locked initially at HP52 / Armor4 / Move2 / Accuracy92 / Evasion8 / Initiative45; Command Barrage 3–6 Cross1 dmg9 +10 hit 5% crit; Rupture Beam 2–4 Single dmg12 +15 hit 10% crit.
- Mission 7 is 10×10 with the exact initial board/opening in the spec, `EliminateTarget { REGENT }`, `VictoryByRound { round: 6 }`, and 1000 + 300 credits.
- Mission Seven is authored and terminal: `MissionDefinition.unlocks: Option<MissionId>`, One–Six use `Some(next)`, Seven uses `None`.
- Persist exactly one new completion field: `CampaignState.completed: bool`. No save version, serde default, converter, migration, or backward compatibility.
- Rename the old terminal `NextMission` screen to `Ending`; a completed save routes there and never attempts to build another battle.
- Skip audio. There is no existing audio path and HPA-386 must not create one for optional polish.
- Presentation additions are limited to attacker attack motion, transient damage numbers, and modest deterministic boss camera emphasis on the existing playback timeline.
- Regent visual stays in `assets/models/mission_one.gltf`; final counts are 15 scenes / 84 nodes / 15 meshes / 15 materials / 1 buffer.
- Whole-campaign tuning changes authored values only when the recorded playtest demonstrates a concrete problem. No analytics/tuning framework.
- No new dependency/crate, generic objective/status/AI/boss/narrative system, new regular enemy, new playable mech, new progression track, New Game+, second glTF, or second PR.

## Known blast radius

- Adding `Regent` makes all exhaustive `UnitArchetype` matches compile-time work in domain/presentation code.
- Changing `MissionDefinition.unlocks` changes every mission definition and tests that compare `unlocks`.
- Adding `CampaignState.completed` changes all direct `CampaignState { ... }` fixtures and save snapshots. This break is intentional; do not mask it with compatibility code.
- Registering Mission Seven invalidates all old assertions that `mission_definition(Seven).is_none()` and all Continue/Proceed routing that treated Seven as terminal.
- Renaming `GameScreen::NextMission` affects `app.rs`, campaign UI, `tests/campaign_flow.rs`, and `tests/presentation_app.rs`.
- Appending the Regent glTF scene changes every test pinning global scene/node/mesh/material counts.

---

### Task 1: Add Regent as the second half-HP boss consumer

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/interaction.rs`
- Modify: `src/presentation/ui.rs`
- Test: `src/domain/enemy.rs`

**Interfaces:**
- Consumes: `UnitArchetype::Dreadnought`, `unit_weapon`, `attack_band_destination`, `build_intent`, current fixed initiative table, existing enemy-only presentation/interaction match arms.
- Produces: `UnitArchetype::Regent`; shared half-HP slot selection for Dreadnought + Regent; Regent initiative 45; normal attack-band movement; temporary Regent scene 13 mapping until Task 3.

- [ ] **Step 1: Add failing Regent fixtures and threshold tests in `src/domain/enemy.rs`**

Use explicit test IDs that do not collide with authored missions:

```rust
const REGENT: UnitId = UnitId(92);
const REGENT_PLAYER: UnitId = UnitId(93);
const COMMAND_BARRAGE: WeaponId = WeaponId(292);
const RUPTURE_BEAM: WeaponId = WeaponId(293);
```

Build the Regent with:

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

Pin the exact 27/26 boundary and preserve the Dreadnought 21/20 boundary in the same test module:

```rust
#[test]
fn both_bosses_switch_weapon_slots_at_their_exact_half_hp_boundary() {
    let mut regent = regent_threshold_fixture();
    assert_eq!(
        unit_weapon(&regent, regent.unit(REGENT).unwrap()).unwrap().id,
        COMMAND_BARRAGE
    );
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
    assert_eq!(
        unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap()).unwrap().id,
        OVERLOAD
    );
}
```

- [ ] **Step 2: Add a failing committed-intent regression for Regent**

Place Regent at `(3,1)`, player at `(3,5)`, begin the round above half HP, then cross to 26 HP:

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

Expected: compile failure because `UnitArchetype::Regent` and its behavior do not exist.

- [ ] **Step 4: Add `Regent` and share only the half-HP slot rule**

Append `Regent` to `UnitArchetype` in `src/domain/model.rs`.

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

Do not introduce a `BossPhase`, threshold table, trait, callback, or boss data object.

- [ ] **Step 5: Put Regent on the existing movement/initiative path**

Extend the ordinary attack-band branch:

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

Add:

```rust
UnitArchetype::Regent => 45,
```

and extend the fixed-initiative test:

```rust
assert_eq!(initiative(&regent), 45);
assert!(initiative(&regent) > initiative(&dreadnought));
assert!(initiative(&regent) > initiative(&controller));
```

- [ ] **Step 6: Keep exhaustive presentation/interaction matches compiling**

Until Task 3 appends the visual, temporarily map:

```rust
UnitArchetype::Regent => 13,
```

in `scene_index`, borrowing the Dreadnought scene only for this intermediate green commit.

Add Regent to the existing enemy-only `PilotSkillWrongUnit` / HUD / interaction match arms. Do not add boss-only commands or HUD state.

- [ ] **Step 7: Verify and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib regent
cargo test --all-targets
```

Expected: Regent threshold/immutable-intent/initiative tests pass and all exhaustive archetype matches compile.

Commit:

```bash
git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/interaction.rs src/presentation/ui.rs
git commit -m "feat: add Regent boss behavior"
```

---

### Task 2: Author Mission 7 and replace the terminal handoff with stable campaign completion

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
- Test: all files above

**Interfaces:**
- Consumes: Task 1's `Regent`, shared `unit_weapon`, normal enemy factories, `build_player_squad`, `EliminateTarget`, `VictoryByRound`, board hazards/explosives, `complete_current_mission`, `ActiveMission`, existing campaign screen/action helpers.
- Produces: `MISSION_SEVEN_DEFINITION`; authored Mission Seven; `MissionDefinition.unlocks: Option<MissionId>`; `CampaignState.completed`; once-only final completion; `GameScreen::Ending`; completed-save Continue routing; exact final encounter and public opening-manipulation regression.

This is one coordinated task because `MissionId::Seven` changes meaning from an unauthored sentinel to a playable terminal mission. Do not split registration and terminal routing into separate commits that knowingly leave `cargo test --all-targets` red or create a transient self-unlocking final mission.

- [ ] **Step 1: Write failing campaign terminal-model tests first**

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

Update the existing Seven save round-trip fixture to expect `completed: true`, and add one unfinished Seven round-trip with `completed: false`.

- [ ] **Step 2: Write failing Mission 7 authoring tests in the new module**

Define IDs:

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
assert_eq!((battle.board().width(), battle.board().height()), (10, 10));
assert_eq!(
    battle.board().blocking_cells().collect::<Vec<_>>(),
    vec![
        GridPos::new(2, 4),
        GridPos::new(7, 4),
        GridPos::new(2, 5),
        GridPos::new(7, 5),
    ]
);
assert_eq!(
    battle.board().hazard_cells().collect::<Vec<_>>(),
    vec![GridPos::new(3, 5), GridPos::new(5, 5)]
);
assert_eq!(battle.board().explosive_at(GridPos::new(5, 8)).unwrap().hp, 4);
assert_eq!(battle.unit(ids::REGENT).unwrap().stats, stats(52, 4, 2, 92, 8, 0));
assert_eq!(
    battle.rules().primary,
    PrimaryObjective::EliminateTarget { target: ids::REGENT }
);
assert_eq!(
    battle.rules().optional,
    OptionalObjective::VictoryByRound { round: 6 }
);
```

Pin exact opening rows:

```rust
[
    (ids::REGENT, GridPos::new(4, 2), Some(ids::VANGUARD)),
    (ids::ARTILLERY, GridPos::new(2, 3), Some(ids::GUNNER)),
    (ids::CONTROLLER, GridPos::new(6, 8), Some(ids::VANGUARD)),
    (ids::BULWARK, GridPos::new(2, 7), Some(ids::VANGUARD)),
    (ids::FLANKER, GridPos::new(1, 9), Some(ids::GUNNER)),
]
```

Call the shared `assert_opening_plan_is_legal(&mission_seven(2))`.

- [ ] **Step 3: Write the failing public opening-manipulation regression with seed 2**

Build the normal authored opening with `mission_seven(2)` and `begin_round()`.

Execute this exact player line:

```rust
battle.begin_activation(ids::VANGUARD).unwrap();
battle.move_unit(ids::VANGUARD, GridPos::new(4, 6)).unwrap();
battle.choose_reaction(ids::VANGUARD, Reaction::Guard).unwrap();
battle.finish_activation(ids::VANGUARD).unwrap();

battle.begin_activation(ids::INTERCEPTOR).unwrap();
battle.move_unit(ids::INTERCEPTOR, GridPos::new(7, 8)).unwrap();
let pulse_events = battle
    .attack(
        ids::INTERCEPTOR,
        squad::ids::VECTOR_PULSE,
        GridPos::new(6, 8),
    )
    .unwrap();
assert!(pulse_events.iter().any(|event| {
    matches!(
        event,
        BattleEvent::UnitPushed { unit, to, .. }
            if *unit == ids::CONTROLLER && *to == GridPos::new(5, 8)
    )
}));
battle.choose_reaction(ids::INTERCEPTOR, Reaction::Guard).unwrap();
battle.finish_activation(ids::INTERCEPTOR).unwrap();

battle.begin_activation(ids::GUNNER).unwrap();
battle.choose_reaction(ids::GUNNER, Reaction::Guard).unwrap();
battle.finish_activation(ids::GUNNER).unwrap();

let events = battle.resolve_enemy_phase().unwrap();
```

Pin seed 2's existing RNG call order from the real Vector Pulse path:

```text
Vector Pulse hit roll 11, non-critical roll 27
Regent Command Barrage hit roll 52, non-critical roll 37
```

Assert the Regent `AttackRolled` binds `weapon: ids::COMMAND_BARRAGE`, `attacker: ids::REGENT`, and `target: ids::CONTROLLER`; assert `ExplosionTriggered { position: (5,8), .. }`; assert the Regent event occurs before any Controller resolution event. Do not use a seed sweep or call `resolve_intent_for_test`.

- [ ] **Step 4: Confirm the coordinated task is red**

Run:

```bash
cargo test --lib mission::mission_seven -- --nocapture
cargo test --test campaign_persistence final_mission_completion_is_persisted_and_idempotent -- --nocapture
```

Expected: compile failures because Mission Seven, `completed`, and `CampaignError::CampaignComplete` do not exist.

- [ ] **Step 5: Change `MissionDefinition.unlocks` to `Option<MissionId>` and update all seven definitions together**

In `src/mission/mod.rs`:

```rust
pub struct MissionDefinition {
    pub id: MissionId,
    pub unlocks: Option<MissionId>,
    // existing fields unchanged
}
```

Update definitions:

```text
Mission 1 -> Some(Two)
Mission 2 -> Some(Three)
Mission 3 -> Some(Four)
Mission 4 -> Some(Five)
Mission 5 -> Some(Six)
Mission 6 -> Some(Seven)
Mission 7 -> None
```

Register:

```rust
pub mod mission_seven;
```

and:

```rust
MissionId::Seven => Some(&mission_seven::MISSION_SEVEN_DEFINITION),
```

Update every `unlocks` assertion in mission/integration tests in the same edit.

- [ ] **Step 6: Add explicit campaign completion with no compatibility layer**

In `CampaignState`:

```rust
pub struct CampaignState {
    pub next_mission: MissionId,
    pub credits: u32,
    pub upgrades: SquadUpgrades,
    pub completed: bool,
}
```

and:

```rust
pub fn new_game() -> Self {
    Self {
        next_mission: MissionId::One,
        credits: 0,
        upgrades: SquadUpgrades::default(),
        completed: false,
    }
}
```

Do not add `#[serde(default)]` or a migration/version field.

Add:

```rust
CampaignComplete,
```

to `CampaignError`, with display copy:

```rust
CampaignError::CampaignComplete => write!(f, "campaign is already complete"),
```

At the start of `complete_mission`, before reward mutation:

```rust
if self.completed {
    return Err(CampaignError::CampaignComplete);
}
```

After calculating the receipt reward:

```rust
match definition.unlocks {
    Some(next) => self.next_mission = next,
    None => self.completed = true,
}
```

Leave `next_mission == Seven` after final completion. `complete_current_mission` keeps its current copy -> mutate -> persist -> replace transaction shape unchanged.

Update every direct `CampaignState { ... }` fixture under `src/` and `tests/` with an explicit `completed` value so the break stays visible.

- [ ] **Step 7: Implement `mission_seven.rs` with only existing combat vocabulary**

Use:

```rust
const MISSION_SEVEN_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 8),
    gunner: GridPos::new(3, 9),
    interceptor: GridPos::new(5, 9),
};
```

Board:

```rust
BoardState::new(
    10,
    10,
    [
        GridPos::new(2, 4),
        GridPos::new(7, 4),
        GridPos::new(2, 5),
        GridPos::new(7, 5),
    ],
    [GridPos::new(3, 5), GridPos::new(5, 5)],
    [ExplosiveState {
        position: GridPos::new(5, 8),
        hp: 4,
        exploded: false,
    }],
)
```

Regent:

```rust
unit(
    ids::REGENT,
    "Regent",
    UnitArchetype::Regent,
    Faction::Enemy,
    stats(52, 4, 2, 92, 8, 0),
    GridPos::new(4, 0),
    vec![ids::COMMAND_BARRAGE, ids::RUPTURE_BEAM],
)
```

Escorts:

```rust
enemies::artillery(ids::ARTILLERY, "Siege Artillery", GridPos::new(2, 2));
enemies::controller(ids::CONTROLLER, "Controller", GridPos::new(8, 8));
enemies::bulwark(ids::BULWARK, "Bulwark", GridPos::new(1, 7));
enemies::flanker(ids::FLANKER, "Flanker", GridPos::new(0, 9));
```

Regent weapons exactly match the Global Constraints. Reuse the regular factories' weapon specs for the escorts.

Mission rules:

```rust
const MISSION_SEVEN_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateTarget { target: ids::REGENT },
    optional: OptionalObjective::VictoryByRound { round: 6 },
    opening_plan: &MISSION_SEVEN_OPENING,
};
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
    // exact dialogue below
};
```

Use the exact pre/aftermath dialogue from the spec with existing `vn/relay_nine_bg.png`, `control_*`, and `vanguard_neutral.png` assets.

- [ ] **Step 8: Pin target-only victory with escorts alive**

Add a Mission 7 test that directly damages only Regent through the existing public damage seam and asserts victory while at least one escort remains above 0 HP:

```rust
battle.apply_direct_damage(
    ids::REGENT,
    99,
    DamageSource::PlayerWeapon(squad::ids::PILE_LANCE),
);
assert!(battle.result().is_some_and(|result| result.victory));
assert!(!battle.unit(ids::BULWARK).unwrap().is_knocked_out());
```

Do not add a new primary objective.

- [ ] **Step 9: Replace `NextMission` sentinel routing with `Ending`**

Rename in `GameScreen`:

```rust
NextMission -> Ending
```

Rename campaign UI helpers:

```text
next_mission_copy -> ending_copy
setup_next_mission_screen -> setup_ending_screen
```

`ending_copy` should read the persisted state and begin with:

```rust
format!(
    "CAMPAIGN COMPLETE\nRelay Nine secured.\n\nFinal credits: {}\n\n...",
    state.credits,
)
```

Register `OnEnter/OnExit(GameScreen::Ending)` in `app.rs`.

Change Continue routing to use `completed` first:

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

Change aftermath completion:

```rust
let next_screen = if runtime
    .0
    .state
    .as_ref()
    .is_some_and(|state| state.completed)
{
    GameScreen::Ending
} else {
    GameScreen::Upgrade
};
advance_dialogue(cursor, mission.0.aftermath.lines.len(), next_screen, next_state);
```

`Proceed` now always sends an unfinished campaign from Upgrade to `PreMissionStory`; remove the `mission_definition(...).is_some()` terminal-sentinel branch.

Keep Ending's only action as `ReturnToTitle`.

- [ ] **Step 10: Update campaign model/flow assertions for authored Seven and terminal `None`**

In `tests/campaign_model.rs`, pin:

```rust
assert_eq!(mission_definition(MissionId::One).unwrap().unlocks, Some(MissionId::Two));
// ... through Six
let seven = mission_definition(MissionId::Seven).unwrap();
assert_eq!(seven.unlocks, None);
assert_eq!(seven.base_reward, 1000);
assert_eq!(seven.optional_reward, 300);
```

Pin reward totals:

```text
Base through Seven: 4300
Optional through Seven: 1250
All rewards: 5550
```

In `tests/campaign_flow.rs`, replace old Seven-terminal expectations with:

```text
Continue on unfinished Seven -> Upgrade
Proceed on unfinished Seven -> PreMissionStory
Continue on completed Seven -> Ending
Final aftermath -> Ending
Earlier aftermath -> Upgrade
```

Add a Mission 7 `enter_battle` regression that proves campaign upgrades project into the final mission and `ActiveMission.id == Seven`.

In `tests/presentation_app.rs`, rename any `NextMission` screen setup/cleanup expectation to `Ending`.

- [ ] **Step 11: Prove base rewards make meaningful upgrades without optionals**

In `tests/campaign_persistence.rs`, add a deterministic progression test:

```rust
#[test]
fn base_rewards_through_six_can_buy_a_level_two_track_for_each_mech() {
    let mut state = CampaignState::new_game();
    for id in [
        MissionId::One,
        MissionId::Two,
        MissionId::Three,
        MissionId::Four,
        MissionId::Five,
        MissionId::Six,
    ] {
        state
            .complete_mission(mission_definition(id).unwrap(), mission_result(true, false))
            .unwrap();
    }
    assert_eq!(state.credits, 3300);
    assert_eq!(state.next_mission, MissionId::Seven);

    for mech in [PlayerMech::Vanguard, PlayerMech::Gunner, PlayerMech::Interceptor] {
        state.purchase_upgrade(mech, UpgradeTrack::Weapon).unwrap(); // 200
        state.purchase_upgrade(mech, UpgradeTrack::Weapon).unwrap(); // 400
    }

    assert_eq!(state.credits, 1500);
    assert_eq!(state.upgrades.vanguard.weapon, 2);
    assert_eq!(state.upgrades.gunner.weapon, 2);
    assert_eq!(state.upgrades.interceptor.weapon, 2);
}
```

This proves optionals accelerate/customize progression but are not required for meaningful pre-final upgrades.

- [ ] **Step 12: Verify the whole coordinated vertical slice and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib mission::mission_seven
cargo test --test campaign_model
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --test presentation_app
cargo test --all-targets
```

Expected: Mission Seven is playable through the shared definition path, final completion is persisted once, and completed Continue/aftermath terminate at Ending.

Commit:

```bash
git add src/mission src/campaign src/presentation/campaign_ui.rs src/presentation/interaction.rs src/app.rs tests/campaign_model.rs tests/campaign_flow.rs tests/campaign_persistence.rs tests/presentation_app.rs
git commit -m "feat: ship Mission 7 campaign ending"
```

---

### Task 3: Give the Regent its own checked-in glTF scene

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Test: `src/presentation/assets.rs`

**Interfaces:**
- Consumes: Task 1's temporary Regent->13 scene mapping and the existing single-buffer glTF append pattern used by Flanker/Bulwark/Controller/Dreadnought.
- Produces: scene 14 / root 77 / part nodes 78–83 / mesh+material 14; `MISSION_ONE_SCENE_COUNT = 15`; Regent->14 mapping.

- [ ] **Step 1: Make the Regent asset test fail before editing the glTF**

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

Update old global count assertions from 14/77/14/14 to 15/84/15/15 while leaving their scene-specific indices unchanged.

- [ ] **Step 2: Confirm red**

Run:

```bash
cargo test --lib presentation::assets::tests -- --nocapture
```

Expected: Regent scene/global counts fail against the 14-scene asset.

- [ ] **Step 3: Append scene 14 using the existing cube accessors and one buffer**

Edit `assets/models/mission_one.gltf` following the Dreadnought append shape:

```text
scene 14 name Regent -> node 77
node 77 name Regent Root, scale [1.20,1.20,1.20], children 78..83
nodes 78..83 reuse mesh 14
mesh 14 name Regent Violet, primitive material 14, POSITION accessor 0, NORMAL accessor 1
material 14 name Regent Violet, baseColorFactor [0.42,0.14,0.78,1.0]
```

Do not add a buffer, accessor, texture, image, animation, or second asset file.

- [ ] **Step 4: Wire the final scene count/index**

Change:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 15;
```

and:

```rust
UnitArchetype::Regent => 14,
```

in `scene_index`.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::assets::tests
cargo test --all-targets
```

Commit:

```bash
git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: add Regent battle visual"
```

---

### Task 4: Fill the remaining board-first combat feedback gaps

**Files:**
- Modify: `src/presentation/mod.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/playback.rs`
- Test: `src/presentation/playback.rs`
- Test: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: existing `BattleEventQueue`/`EventPlayback`, `BattleEvent::AttackRolled` / `DamageApplied`, `UnitVisual`, `EventEffect`, battle camera, `BattleRuntime` archetype lookup.
- Produces: short attacker motion, transient `Text2d` damage numbers, deterministic small camera emphasis for Dreadnought/Regent attacks, guaranteed transform reset after each event.

Do not replace selection/reachable/telegraph/impact/KO presentation that already exists, and do not add audio or a cinematic camera controller.

- [ ] **Step 1: Add pure helper tests before spawning new entities**

In `playback.rs`, add small helpers and tests around values that matter:

```rust
fn damage_number_text(amount: i16) -> String {
    format!("-{amount}")
}

fn boss_attack_emphasis(archetype: UnitArchetype) -> bool {
    matches!(archetype, UnitArchetype::Dreadnought | UnitArchetype::Regent)
}
```

Tests:

```rust
#[test]
fn damage_number_uses_applied_damage_amount() {
    assert_eq!(damage_number_text(7), "-7");
    assert_eq!(damage_number_text(12), "-12");
}

#[test]
fn camera_emphasis_is_limited_to_the_two_bosses() {
    assert!(boss_attack_emphasis(UnitArchetype::Dreadnought));
    assert!(boss_attack_emphasis(UnitArchetype::Regent));
    assert!(!boss_attack_emphasis(UnitArchetype::Controller));
    assert!(!boss_attack_emphasis(UnitArchetype::Vanguard));
}
```

- [ ] **Step 2: Introduce focused presentation markers, not a framework**

In `presentation/mod.rs` add:

```rust
#[derive(Component)]
pub struct DamageNumberEffect;

#[derive(Component, Clone, Copy)]
pub struct BattleCamera {
    pub rest: Transform,
}
```

When spawning the existing `Camera3d`, construct its transform once and attach `BattleCamera { rest: transform }`.

Do not create camera modes, shot queues, or timeline types.

- [ ] **Step 3: Add attacker motion on `AttackRolled`**

Extend `animate_unit_event` so the attacker also receives a short pulse:

```rust
BattleEvent::AttackRolled { attacker, .. } if *attacker == visual.0 => {
    let pulse = (progress * PI).sin();
    transform.scale = Vec3::splat(UNIT_SCALE * (1.0 + pulse * 0.10));
}
```

Retain the existing target-hit pulse/damage shake. Ensure the end of an event restores scale to `UNIT_SCALE` for visible units instead of accumulating prior-event scale.

- [ ] **Step 4: Spawn and animate one damage number per `DamageApplied`**

In `spawn_event_effect`, handle:

```rust
BattleEvent::DamageApplied { target, amount, .. }
```

by resolving the target position from `BattleRuntime` and spawning a child under the presentation root with:

```rust
Text2d::new(damage_number_text(*amount)),
TextFont {
    font_size: FontSize::Px(28.0),
    ..default()
},
TextColor(Color::WHITE),
Transform::from_translation(grid_to_world(position) + Vec3::new(0.0, 0.9, 0.0)),
DamageNumberEffect,
EventEffect,
Pickable::IGNORE,
ChildOf(root),
```

Use the existing effect timer. During `animate_effects`, detect damage numbers and raise them by a bounded offset derived from current progress; do not increment translation cumulatively every frame.

- [ ] **Step 5: Add small deterministic boss camera emphasis and reset**

Pass a query for `(&BattleCamera, &mut Transform)` into playback.

When the current event is `AttackRolled { attacker, .. }`, look up the attacker in `BattleRuntime`. If `boss_attack_emphasis(unit.archetype)`:

```rust
let shake = (progress * PI * 4.0).sin() * 0.06 * (1.0 - progress);
transform.translation = camera.rest.translation + Vec3::new(shake, 0.0, -shake * 0.5);
```

For all other events, and when an event finishes:

```rust
*transform = camera.rest;
```

Do not change projection/zoom.

- [ ] **Step 6: Add a Bevy `App` regression that effects are transient**

In `tests/presentation_app.rs`, exercise playback with a queued `DamageApplied` event and assert a `DamageNumberEffect` exists while the event is active, then advance time/update until the event finishes and assert it is despawned. Keep this renderer-free; do not snapshot pixels.

- [ ] **Step 7: Verify and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::playback
cargo test --test presentation_app
cargo test --all-targets
```

Commit:

```bash
git add src/presentation/mod.rs src/presentation/battlefield.rs src/presentation/playback.rs tests/presentation_app.rs
git commit -m "feat: polish board-first combat feedback"
```

---

### Task 5: Play Missions 1–7 in order and tune only evidence-backed authored values

**Files:**
- Create: `docs/validation/hpa-386.md`
- Modify only if evidence requires: `src/mission/mission_one.rs`
- Modify only if evidence requires: `src/mission/mission_two.rs`
- Modify only if evidence requires: `src/mission/mission_three.rs`
- Modify only if evidence requires: `src/mission/mission_four.rs`
- Modify only if evidence requires: `src/mission/mission_five.rs`
- Modify only if evidence requires: `src/mission/mission_six.rs`
- Modify only if evidence requires: `src/mission/mission_seven.rs`
- Modify only if evidence requires: `src/mission/squad.rs`
- Modify only if evidence requires: `src/mission/enemies.rs`
- Modify only if evidence requires: `src/campaign/progression.rs`
- Test: focused tests beside whichever authored values change

**Interfaces:**
- Consumes: the complete New Game -> Missions 1–7 -> Ending flow from Tasks 1–4.
- Produces: measured first-playthrough evidence, a concrete per-mission intent-manipulation ledger, and only the smallest value/placement changes required to hit the MVP acceptance bar.

This is intentionally an evidence task. Do not invent tuning changes before playing the actual campaign.

- [ ] **Step 1: Create the validation ledger before the run**

Create `docs/validation/hpa-386.md` with this exact table header:

```markdown
# HPA-386 validation

## Full-campaign playthrough

| Mission | Minutes | Rounds | Restarts | Optional | Intent manipulation materially rewarded? | Credits after | Upgrades purchased | Notes / tuning |
| --- | ---: | ---: | ---: | --- | --- | ---: | --- | --- |
| 1 | | | | | | | | |
| 2 | | | | | | | | |
| 3 | | | | | | | | |
| 4 | | | | | | | | |
| 5 | | | | | | | | |
| 6 | | | | | | | | |
| 7 | | | | | | | | |

## Campaign totals

- Total first-playthrough minutes:
- Missions materially rewarding committed-intent reading/manipulation:
- Completed-save Continue result:
- Final ending result:
- Base-reward-only upgrade regression: automated test, 3300 -> three level-2 Weapon tracks -> 1500 credits remaining.

## Tuning changes

Record every authored value changed during this pass as `before -> after`, the observed problem, and the replay result. If no tuning is required, state `No authored tuning required after the measured run.`
```

The empty measurement cells are the form to fill from the manual run, not implementation placeholders; do not merge the final task until every row and campaign-total field is populated.

- [ ] **Step 2: Start from a clean save and run the real UI flow**

Delete/move the local development campaign save, launch:

```bash
cargo run --release
```

Use New Game and play through all seven missions via the real story, briefing, battle, aftermath, upgrade, save/Continue, and Ending flow. Do not seed a later mission for the primary timing run.

Use a wall-clock timer per mission from pre-mission story start to aftermath completion. Record rounds, restarts, optional completion, credits, and purchases immediately after each mission.

- [ ] **Step 3: Classify committed-intent payoff conservatively**

Mark `Yes` only when the completed run contained a concrete decision where reading a committed enemy footprint/target materially changed the player's move and produced an advantage such as:

```text
evading a committed hit while preserving an action;
redirecting enemy fire onto an enemy or explosive;
using push/collision/hazard positioning because a telegraphed attack was locked;
choosing Counter/Guard/Evade in response to a specific committed threat.
```

Pure focus fire or incidental enemy friendly fire does not count.

The final ledger must show at least 4 `Yes` missions. If fewer than 4 qualify, adjust opening placement/footprints or enemy placement in the weakest candidate mission and replay that mission; do not add a new mechanic.

- [ ] **Step 4: Apply tuning only when a measured problem exists**

Use this order for each observed issue:

```text
1. placement/opening geometry
2. enemy count
3. boss HP/half-HP timing
4. existing authored round pressure
5. weapon hit/damage/EN values
6. reward/upgrade values
```

Change one dimension at a time, add/update the focused authoring test that pins the new value, rerun the affected mission, and record `before -> after` plus the replay result in the ledger.

Do not respond to a long/short/noisy encounter by adding mechanics, AI layers, new objectives, dynamic difficulty, or telemetry.

- [ ] **Step 5: Enforce the campaign timing acceptance**

Sum the seven recorded mission times.

Preferred result:

```text
120–180 minutes
```

If the measured run is outside that range, first tune the missions that clearly account for the deviation and replay them. If the final total still lands slightly outside the target but further tuning would damage encounter quality, record the final measured total and a concrete justification in `Campaign totals` rather than inventing filler or cutting a good encounter blindly.

- [ ] **Step 6: Validate the final threshold/readability paths manually**

During Mission 6 and 7, explicitly record:

```text
Mission 6: current committed Graviton remains unchanged when Dreadnought crosses 21 -> 20; next planning uses Overload.
Mission 7: current committed Command Barrage remains unchanged when Regent crosses 27 -> 26; next planning uses Rupture Beam.
```

Also record whether the five opening Mission 7 telegraphs remain visually distinguishable. If they are not, remove/reposition one escort before adding UI filtering.

- [ ] **Step 7: Validate save/ending behavior from the real completed save**

After the ending:

1. Return to Title.
2. Choose Continue.
3. Confirm it opens Ending, not Upgrade/Battle.
4. Return to Title again.
5. Confirm the saved credits/upgrades remain unchanged and no second Mission 7 reward is granted.

Record the observed result in the ledger.

- [ ] **Step 8: Run focused/full tests after any tuning and commit the evidence**

If source values changed, run their focused tests first. Then run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

Commit the populated ledger and any evidence-backed value changes:

```bash
git add docs/validation/hpa-386.md src/mission src/campaign/progression.rs
git commit -m "test: validate and tune the seven-mission campaign"
```

If no source tuning was needed, stage only the validation document.

---

### Task 6: Close out MVP docs and run the repository gates

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `docs/validation/hpa-386.md`
- Verify: `.github/workflows/ci.yml` remains unchanged unless a real gate bug is discovered

**Interfaces:**
- Consumes: final authored/tuned campaign and validation evidence from Tasks 1–5.
- Produces: repository docs that describe the seven-mission completed MVP, immutable final gate evidence, and a PR ready for implementation review.

- [ ] **Step 1: Update only stale campaign documentation**

In `README.md` and `CLAUDE.md`, replace text that still describes Mission 7 as an unauthored handoff. Document:

```text
Missions 1–7 are playable in order.
Mission 6 Dreadnought: half-HP Graviton -> Overload behavior.
Mission 7 Regent: half-HP Command Barrage -> Rupture Beam behavior.
Campaign completion persists and Continue reopens the ending.
The game remains board-first; there is no separate battle-animation scene.
```

Keep existing controls unless Task 4 actually changed them; do not add a long implementation history.

- [ ] **Step 2: Add final gate section to `docs/validation/hpa-386.md`**

Append:

```markdown
## Final automated gates

- `cargo fmt --check`: PASS
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS
- `cargo test --all-targets`: PASS (<record final test count>)
- `cargo llvm-cov --all-targets --lcov --output-path lcov.info`: PASS
- `cargo build --release`: PASS
- PR `Build + lint`: PASS
- PR `Unit test`: PASS
```

Replace `<record final test count>` with the actual count printed by the final run before committing this section.

- [ ] **Step 3: Run the same local gate set CI expects**

Run exactly:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Expected: all commands succeed. Do not claim the test count or coverage until the commands have actually produced them.

- [ ] **Step 4: Re-check the HPA-386 acceptance contract against the final tree**

Verify from code/tests/ledger:

```text
Seven authored missions, six regular enemies, two bosses.
No parallel final-boss engine or foundational new combat subsystem.
Mission 7 completion persisted exactly once.
Completed Continue -> Ending.
Mission 7 synthesizes locked intent + displacement/environment + reactions + mixed escorts.
Board-first presentation remains in the normal battlefield; no dedicated animation scene.
Full campaign playthrough/timing/intent-majority evidence recorded.
Base-only upgrade affordability automated regression passes.
Regent/Dreadnought threshold regressions pass.
```

If an item is false, fix that concrete gap before marking the task complete; do not broaden scope beyond the ticket.

- [ ] **Step 5: Commit docs/gate evidence**

```bash
git add README.md CLAUDE.md docs/validation/hpa-386.md
git commit -m "docs: close out the Scorpius MVP"
```

- [ ] **Step 6: Push and verify PR CI on the final head**

```bash
git push
```

Wait only for the normal GitHub PR checks in the execution workflow; do not create a second PR. If `Build + lint` or `Unit test` fails, diagnose and fix the existing branch, rerun the corresponding local command, and push the fix.

---

## Plan self-review

### Spec coverage

- Final boss second threshold consumer: Task 1.
- Mission 7 encounter/objectives/story/reward and intent-manipulation centerpiece: Task 2.
- Stable once-only campaign-complete/ending state and save routing: Task 2.
- Final boss visual: Task 3.
- Existing board-first presentation retained; damage numbers/attack motion/modest boss emphasis added; audio intentionally skipped: Task 4.
- Seven-mission timing, intent-majority, progression/readability evidence and value-only tuning: Task 5.
- README/CLAUDE/validation + exact CI-equivalent gates: Task 6.

### Scope check

The six task groups still form one HPA-386 player-visible closeout and one PR. No task creates reusable infrastructure without an existing consumer; the only generalization is the half-HP boss selector now justified by its second concrete boss.

### Type/interface consistency

- `MissionDefinition.unlocks` is `Option<MissionId>` from Task 2 onward.
- `CampaignState.completed` is persisted as a required `bool`; no old-save compatibility path exists.
- `GameScreen::Ending` replaces `GameScreen::NextMission`; later tasks use only `Ending`.
- Regent uses authored IDs 71/209 in Mission 7; Task 1's 92/292 IDs are test-fixture-only and do not leak into authored content.
- Task 3 changes Regent scene mapping from temporary 13 to final 14.
