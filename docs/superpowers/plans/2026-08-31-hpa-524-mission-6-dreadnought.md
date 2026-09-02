# HPA-524 Mission 6 Dreadnought Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 6 and the first Dreadnought boss as one player-visible HPA-524 slice, with one derived half-HP behavior change on the existing locked-intent path and a persisted Mission 7 handoff.

**Architecture:** Add one concrete `Dreadnought` archetype. Extend the existing `unit_weapon` selector so Dreadnought uses slot 1 at/below half HP, make `build_intent` and the authored-opening validator reuse that selector, and keep committed intents immutable. Mission 6 owns boss values/content locally. Continue reuses `mission_definition` instead of growing another per-mission list. No boss phase runtime, threshold registry, resistance system, or second combat path.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md`

## Global Constraints

- One HPA-524 ticket = one PR; continue implementation on this planning branch/PR.
- One normal single-cell `UnitState`; no boss runtime, stored boss phase, threshold registry, scripting, or policy layer.
- Dreadnought HP 21–40 uses Graviton Salvo; HP 0–20 uses Overload Salvo.
- Graviton is range 3–6 Cross1; Overload is **range 2–4 Cross1**. Do not restore min range 1: Cross1 radius 1 would include the attacker at adjacent range and allow boss self-damage/free Turnabout.
- Current committed intent never changes after threshold crossing.
- `unit_weapon` is the one selector used by movement, intent construction, and the test-only authored-opening validator.
- Dreadnought initiative is 40, above Controller 35; Mission 6's centerpiece depends on that order.
- Boss remains pushable; no resistance system.
- Mission 6 owns boss factory/weapons locally; regular enemy factories remain shared.
- Mission 6: 9×9, existing blocking only, no hazards/explosives, `EliminateTarget`, `Turnabout`, 800 + 250 credits.
- Mission IDs become One–Seven; One–Six are authored, Seven is the terminal HPA-524 handoff.
- Continue: One -> story; later authored -> Upgrade; unauthored -> NextMission. Do not add another mission-ID list or routing helper.
- Final glTF counts: 14 scenes / 77 nodes / 14 meshes / 14 materials / 1 buffer.
- No new objective/status/AI/progression/save framework, dependency/crate, Mission 7 content, or second PR.

## Risks

- **Cross1 self-overlap — highest domain risk.** At min range 1 a Cross1 target footprint contains an orthogonally adjacent attacker. Enemy resolution does not exclude the attacker and enemy-on-enemy damage completes Turnabout. Keep Overload at 2–4 and pin the attacker is absent from its committed footprint.
- **Resolution ordering.** Dreadnought must resolve before Controller so the redirected Graviton deals normal damage, completes Turnabout, and leaves Controller alive to fire into the vacated target. Pin Dreadnought 40 > Controller 35 and run the centerpiece through `resolve_enemy_phase()`.
- **Mission registration blast radius.** Making Six authored invalidates old terminal assertions in mission modules plus `campaign_model`, `campaign_flow`, and `campaign_persistence`. Land all of those updates in Task 2 so every task remains green under `cargo test --all-targets`.
- **Asset append blast radius.** Existing glTF tests pin global counts and Controller's part loop is unbounded. Repair those tests before appending Dreadnought nodes.

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
- Consumes: `UnitState.weapons`, `unit_weapon`, `build_intent`, `attack_band_destination`, `initiative`, exhaustive presentation/interaction matches.
- Produces: `UnitArchetype::Dreadnought`; crate-visible half-HP `unit_weapon`; `build_intent` using that selector; initiative 40; Overload 2–4 contract; temporary scene mapping until Task 3.

- [ ] **Step 1: Write the failing threshold fixtures/tests**

Add test constants:

```rust
const DREADNOUGHT: UnitId = UnitId(90);
const TEST_PLAYER: UnitId = UnitId(91);
const GRAVITON: WeaponId = WeaponId(290);
const OVERLOAD: WeaponId = WeaponId(291);
```

Create a 7×7 fixture with boss `(3,1)`, player `(3,5)`, boss stats `40/3/1/90/5/0`, and:

```rust
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
)
squad::weapon(
    OVERLOAD,
    "Overload Salvo",
    2,
    4,
    WeaponShape::Cross1,
    10,
    10,
    10,
    0,
    false,
    false,
)
```

Pin the selector and immutable intent:

```rust
#[test]
fn dreadnought_switches_weapon_once_at_half_hp() {
    let mut battle = dreadnought_threshold_fixture();
    assert_eq!(
        unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap())
            .unwrap()
            .id,
        GRAVITON
    );

    battle
        .apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);
    assert_eq!(battle.unit(DREADNOUGHT).unwrap().hp, 20);
    assert_eq!(
        unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap())
            .unwrap()
            .id,
        OVERLOAD
    );

    battle
        .apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
    assert_eq!(
        unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap())
            .unwrap()
            .id,
        OVERLOAD
    );
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

- [ ] **Step 2: Pin Overload's self-overlap guard and close-pressure move**

Create one fixture with boss `(3,1)` and player `(3,3)` (distance 2), damage boss to 20, build the Overload intent, and pin both the authored range and footprint:

```rust
#[test]
fn overload_cross_never_contains_its_attacker() {
    let mut battle = dreadnought_range_two_fixture();
    battle
        .apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);

    let intent = build_intent(&battle, DREADNOUGHT, Some(GridPos::new(3, 3))).unwrap();
    let weapon = battle.weapon(OVERLOAD).unwrap();

    assert_eq!((weapon.min_range, weapon.max_range), (2, 4));
    assert_eq!(intent.profile.weapon, OVERLOAD);
    assert!(!intent.footprint.contains(&GridPos::new(3, 1)));
}
```

Create a second fixture with boss `(3,0)`, player `(3,5)`, same stats/weapons:

```rust
#[test]
fn dreadnought_overload_closes_from_range_five() {
    let mut battle = dreadnought_close_pressure_fixture();
    battle
        .apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);

    let destination = choose_enemy_destination(&battle, DREADNOUGHT).unwrap();

    assert_eq!(destination, GridPos::new(3, 1));
    assert_eq!(destination.manhattan(GridPos::new(3, 5)), 4);
}
```

- [ ] **Step 3: Confirm red**

```bash
cargo test --lib dreadnought -- --nocapture
```

Expected: compile failures because the archetype and selector behavior do not exist yet.

- [ ] **Step 4: Add the archetype and make `unit_weapon` the crate-visible selector**

Append `Dreadnought` to `UnitArchetype`.

Replace the selector with:

```rust
pub(crate) fn unit_weapon<'a>(
    battle: &'a BattleState,
    unit: &UnitState,
) -> Result<&'a WeaponSpec, BattleError> {
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

Do not add threshold state to `UnitState`.

- [ ] **Step 5: Make intent construction reuse `unit_weapon`**

At the start of `build_intent` replace the independent first-weapon lookup:

```rust
let attacker = battle
    .unit(attacker_id)
    .ok_or(BattleError::UnknownUnit(attacker_id))?;
let weapon = unit_weapon(battle, attacker)?;
let weapon_id = weapon.id;
```

The resulting `AttackProfile` remains snapshotted into `AttackIntent`; later HP changes do not mutate it.

- [ ] **Step 6: Add attack-band movement and initiative 40**

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

Extend `initiative_is_fixed_per_archetype_without_position` with an inline Dreadnought built through `squad::unit` and pin:

```rust
assert_eq!(initiative(&dreadnought), 40);
assert!(initiative(&dreadnought) > initiative(&controller));
```

Do not add a shared Dreadnought factory just for this test.

- [ ] **Step 7: Keep presentation/interaction exhaustive**

Temporarily map `UnitArchetype::Dreadnought => 11` in `scene_index`; Task 3 changes it to scene 13.

Add Dreadnought to the existing enemy-only branches in `ui.rs` and `interaction.rs`; do not give the boss a pilot command or special HUD.

- [ ] **Step 8: Verify and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib dreadnought
cargo test --all-targets
```

Expected: threshold, range, footprint, close-pressure, and initiative tests pass; all existing archetype matches compile.

```bash
git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add dreadnought threshold behavior"
```

---

### Task 2: Author Mission 6 and advance campaign/save routing to Seven

**Files:**
- Create: `src/mission/mission_six.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/mission/mission_two.rs`
- Modify: `src/mission/mission_three.rs`
- Modify: `src/mission/mission_four.rs`
- Modify: `src/mission/mission_five.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Modify: `tests/campaign_model.rs`
- Modify: `tests/campaign_flow.rs`
- Modify: `tests/campaign_persistence.rs`
- Test: all files above

**Interfaces:**
- Consumes: Task 1's `Dreadnought` and `unit_weapon`, `build_player_squad`, `squad::{unit, stats, weapon}`, regular enemy factories, `EliminateTarget`, `Turnabout`, `assert_opening_plan_is_legal`, `complete_current_mission`, current save/session flow.
- Produces: `MISSION_SIX_DEFINITION`; Six authored / Seven terminal; data-driven Continue; exact Mission 6 encounter; persisted 3300-base-credit path through Mission 6.

- [ ] **Step 1: Write the failing Mission 6 authoring tests**

Create `src/mission/mission_six.rs` and pin:

```rust
assert_eq!(battle.board().width(), 9);
assert_eq!(battle.board().height(), 9);
assert_eq!(battle.board().blocking_cells().collect::<Vec<_>>(), vec![
    GridPos::new(2, 4),
    GridPos::new(6, 4),
    GridPos::new(2, 5),
    GridPos::new(6, 5),
]);
assert_eq!(battle.board().hazard_cells().count(), 0);
assert_eq!(battle.board().explosives().count(), 0);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.max_hp, 40);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.armor, 3);
assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.movement, 1);
assert_eq!(
    battle.unit(ids::DREADNOUGHT).unwrap().weapons,
    vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO]
);
assert_eq!(
    battle.rules().primary,
    PrimaryObjective::EliminateTarget { target: ids::DREADNOUGHT }
);
assert_eq!(battle.rules().optional, OptionalObjective::Turnabout);
```

Pin exact opening rows:

```rust
[
    (ids::DREADNOUGHT, GridPos::new(4, 2), Some(ids::VANGUARD)),
    (ids::BULWARK, GridPos::new(1, 7), Some(ids::VANGUARD)),
    (ids::CONTROLLER, GridPos::new(6, 7), Some(ids::VANGUARD)),
    (ids::RIFLEMAN, GridPos::new(6, 6), Some(ids::INTERCEPTOR)),
]
```

- [ ] **Step 2: Confirm red**

```bash
cargo test --lib mission::mission_six -- --nocapture
```

Expected: compile failure because the module/definition/IDs do not exist.

- [ ] **Step 3: Register Six/Seven and update every old terminal pin immediately**

In `src/mission/mod.rs`:

```rust
pub mod mission_six;
```

Extend `MissionId`/Display with Seven and register:

```rust
MissionId::Six => Some(&mission_six::MISSION_SIX_DEFINITION),
MissionId::Seven => None,
```

In Missions 2–5, replace assertions that `mission_definition(MissionId::Six).is_none()` with Seven. In Mission 5 explicitly assert Six now resolves to `(MissionId::Six, MissionId::Seven)`.

Also change the test-only opening validator to use Task 1's selector instead of `.first()`:

```rust
let weapon = crate::domain::enemy::unit_weapon(battle, unit)
    .expect("opening unit has its selected weapon");
assert!(weapon_reaches(weapon, opening.destination, target.position));
```

Do not add another selector helper.

- [ ] **Step 4: Make Continue data-driven in the same registration change**

Replace the per-ID Continue list with:

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

Leave `Proceed` on its existing `mission_definition` check. Add no routing helper/table.

- [ ] **Step 5: Implement local Dreadnought and Mission 6 content**

Local IDs:

```rust
pub const DREADNOUGHT: UnitId = UnitId(61);
pub const BULWARK: UnitId = UnitId(62);
pub const CONTROLLER: UnitId = UnitId(63);
pub const RIFLEMAN: UnitId = UnitId(64);
pub const GRAVITON_SALVO: WeaponId = WeaponId(207);
pub const OVERLOAD_SALVO: WeaponId = WeaponId(208);
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
weapon(
    ids::GRAVITON_SALVO,
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
)
weapon(
    ids::OVERLOAD_SALVO,
    "Overload Salvo",
    2,
    4,
    WeaponShape::Cross1,
    10,
    10,
    10,
    0,
    false,
    false,
)
```

Reuse `enemies::{bulwark, controller, rifleman}` and their existing weapons for escorts.

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

Use the exact dialogue from the spec; add no assets.

- [ ] **Step 6: Pin opening legality and build a resolution-ready redirection helper**

Add:

```rust
#[test]
fn mission_six_opening_rows_are_legal() {
    assert_opening_plan_is_legal(&mission_six(1));
}
```

Create a test helper that uses seed 2 and finishes all three player activations, firing the real Vector Pulse through `BattleState::attack` (damage + push) so the RNG call order matches a state the player can actually create:

```rust
fn redirected_opening_ready_to_resolve() -> BattleState {
    let mut battle = mission_six(2);
    battle.begin_round().unwrap();

    assert_eq!(battle.intents()[0].attacker, ids::DREADNOUGHT);
    assert_eq!(battle.intents()[1].attacker, ids::CONTROLLER);
    let boss_intent = battle.intent_for(ids::DREADNOUGHT).unwrap();
    assert_eq!(boss_intent.profile.weapon, ids::GRAVITON_SALVO);
    assert!(boss_intent.footprint.contains(&GridPos::new(5, 7)));

    battle.begin_activation(ids::VANGUARD).unwrap();
    battle.move_unit(ids::VANGUARD, GridPos::new(4, 5)).unwrap();
    battle.choose_reaction(ids::VANGUARD, Reaction::Guard).unwrap();
    battle.finish_activation(ids::VANGUARD).unwrap();

    battle.begin_activation(ids::INTERCEPTOR).unwrap();
    battle.move_unit(ids::INTERCEPTOR, GridPos::new(7, 7)).unwrap();
    // Real Vector Pulse: damage then push through `attack`, spending the
    // hit and crit rolls. Seed 2 pins a normal hit (roll 11) that deals 3
    // damage (9 -> 6) and pushes the Controller onto the boss footprint.
    let vp_events = battle
        .attack(ids::INTERCEPTOR, squad::ids::VECTOR_PULSE, GridPos::new(6, 7))
        .unwrap();
    assert!(vp_events.iter().any(|event| matches!(
        event,
        BattleEvent::AttackRolled { attacker, target, roll: 11, hit: true, critical: false, .. }
            if *attacker == ids::INTERCEPTOR && *target == ids::CONTROLLER
    )));
    assert!(vp_events.iter().any(|event| matches!(
        event,
        BattleEvent::UnitPushed { unit, to, .. } if *unit == ids::CONTROLLER && *to == GridPos::new(5, 7)
    )));
    battle.choose_reaction(ids::INTERCEPTOR, Reaction::Guard).unwrap();
    battle.finish_activation(ids::INTERCEPTOR).unwrap();

    battle.begin_activation(ids::GUNNER).unwrap();
    battle.choose_reaction(ids::GUNNER, Reaction::Guard).unwrap();
    battle.finish_activation(ids::GUNNER).unwrap();

    assert_eq!(battle.unit(ids::CONTROLLER).unwrap().position, GridPos::new(5, 7));
    assert_eq!(battle.unit(ids::VANGUARD).unwrap().position, GridPos::new(4, 5));
    // Vector Pulse damage applied: Controller is at 6 HP, not its start 9.
    assert_eq!(battle.unit(ids::CONTROLLER).unwrap().hp, 6);
    battle
}
```

This helper drives the real Vector Pulse action so the test proves a state the player can create; the earlier `preview_attack` + `resolve_push` shortcut skipped Vector Pulse damage and its two RNG rolls, which let the Controller survive at 2 HP and fire into the vacated cell — a state the live game never produces (the manual playtest observed the KO).

- [ ] **Step 7: Resolve the centerpiece through `resolve_enemy_phase()` with pinned seed semantics**

Use one deterministic seed, not a sweep:

```rust
#[test]
fn redirected_graviton_completes_turnabout_and_cancels_the_knocked_out_controller() {
    let mut battle = redirected_opening_ready_to_resolve();
    let events = battle.resolve_enemy_phase().unwrap();

    // Seed 2: the redirected boss Graviton hits the Controller (roll 52,
    // no crit). The Controller is at 6 HP from Vector Pulse, so the 7
    // Graviton damage knocks it out.
    let boss_hit = events.iter().position(|event| matches!(
        event,
        BattleEvent::AttackRolled {
            attacker,
            target,
            roll: 52,
            hit: true,
            critical_roll: Some(37),
            critical: false,
            ..
        } if *attacker == ids::DREADNOUGHT && *target == ids::CONTROLLER
    )).expect("seed 2 pins a normal Graviton hit on the redirected Controller");

    let turnabout = events.iter().position(|event| matches!(
        event,
        BattleEvent::OptionalObjectiveCompleted
    )).expect("redirected enemy fire completes Turnabout");

    let controller_canceled = events.iter().position(|event| matches!(
        event,
        BattleEvent::IntentCanceled { attacker } if *attacker == ids::CONTROLLER
    )).expect("knocked-out Controller intent is canceled");

    // The Controller was knocked out by the redirected Graviton, so it
    // never fires into the vacated cell.
    assert!(!events.iter().any(|event| matches!(
        event,
        BattleEvent::AttackHitEmpty { attacker, .. } if *attacker == ids::CONTROLLER
    )), "knocked-out Controller does not fire");

    assert!(boss_hit < turnabout);
    assert!(turnabout < controller_canceled);
    assert!(battle.unit(ids::CONTROLLER).unwrap().is_knocked_out());
    assert_eq!(battle.unit(ids::CONTROLLER).unwrap().hp, 0);
}
```

Seed 2 is intentional: Vector Pulse rolls 11 (hit, no crit at 5%) and the redirected Graviton rolls 52 (hit at 85%, no crit at 5%). Vector Pulse's 3 damage (9 → 6) plus the Graviton's 7 damage knocks the Controller out, so its committed intent is canceled rather than firing into the vacated cell. A changed RNG call order should fail this regression instead of silently finding another seed.

- [ ] **Step 8: Pin target victory and normal boss displacement**

Add a target-only victory test that directly KOs Dreadnought and asserts victory while Bulwark lives.

Add a pushability test that positions Vanguard `(3,3)` / Dreadnought `(4,3)`, calls existing `resolve_push`, and asserts Dreadnought moves to `(5,3)` with `UnitPushed`. Add no boss-resistance event/error.

- [ ] **Step 9: Update campaign-model/flow/persistence blast radius before the task gate**

In `tests/campaign_model.rs`:

```rust
let six = mission_definition(MissionId::Six).unwrap();
assert_eq!(six.unlocks, MissionId::Seven);
assert_eq!((six.base_reward, six.optional_reward), (800, 250));
assert_eq!(mission_definition(MissionId::Seven), None);
```

Update the base-reward sum through Six to **3300**.

In `tests/campaign_flow.rs` use the existing `complete_current_mission` API, not a new completion helper:

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

Also add the optional-complete 250 reward case, Mission 6 briefing/dialogue assertions, and update routing assertions:

```rust
assert_eq!(route_continue(MissionId::Six), Some(GameScreen::Upgrade));
assert_eq!(route_continue(MissionId::Seven), Some(GameScreen::NextMission));
```

Proceed Six -> `PreMissionStory`; Proceed Seven -> `NextMission`. Update the existing full-flow Continue-at-Six assertion from `NextMission` to `Upgrade`.

In `tests/campaign_persistence.rs`, update the existing comment/assertion that currently says Six is terminal:

```rust
assert!(mission_definition(MissionId::Six).is_some());
assert!(mission_definition(MissionId::Seven).is_none());
```

Then round-trip:

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

No migration/version conversion.

- [ ] **Step 10: Verify the entire task is green and commit**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib mission::mission_six
cargo test --test campaign_model
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --all-targets
```

Expected: Six is authored everywhere, Seven is the only handoff, the real enemy phase proves the redirection/Turnabout ordering, and no old terminal assertion remains red.

```bash
git add src/mission/mod.rs src/mission/mission_six.rs src/mission/mission_two.rs src/mission/mission_three.rs src/mission/mission_four.rs src/mission/mission_five.rs src/presentation/campaign_ui.rs tests/campaign_model.rs tests/campaign_flow.rs tests/campaign_persistence.rs
git commit -m "feat: author Mission 6 Dreadnought encounter"
```

---

### Task 3: Append the Dreadnought visual and repair the glTF test blast radius

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Test: `src/presentation/assets.rs`
- Test: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: existing one-buffer glTF, Bulwark scene/root/part structure, `MISSION_ONE_SCENE_COUNT`, `scene_index`.
- Produces: scene 13 Dreadnought; counts 14/77/14/14/1; permanent `Dreadnought -> 13` mapping.

- [ ] **Step 1: Update the old test blast radius first**

In `flanker_scene_is_authored_with_own_mesh_material_and_root_scale` and `bulwark_and_controller_scenes_are_authored_with_own_meshes_and_roots`, update global counts to the final expected values where they are intended as whole-file assertions:

```text
scenes 14
nodes 77
meshes 14
materials 14
```

Bound the existing Controller loop:

```rust
for (index, part) in nodes.iter().enumerate().skip(64).take(6) {
    assert_eq!(part["mesh"], 12, "node {index} must use mesh 12");
}
```

This prevents new nodes 70–76 from being incorrectly treated as Controller parts.

- [ ] **Step 2: Add the failing Dreadnought structure test**

Add:

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

Expected: scene/root/mesh/material 13 do not exist yet.

- [ ] **Step 4: Append the exact Dreadnought glTF entries**

In `assets/models/mission_one.gltf`:

- append scene `{ "name": "Dreadnought", "nodes": [70] }`;
- copy Bulwark root/part transform structure from nodes 56–62 into nodes 70–76;
- set root 70 name `Dreadnought Root`, children `[71,72,73,74,75,76]`, scale `[1.12,1.12,1.12]`;
- keep the copied part transforms, rename with Dreadnought prefixes, and point each part to mesh 13;
- append mesh 13 `Dreadnought Crimson` using existing cube POSITION/NORMAL accessors and material 13;
- append material 13 `Dreadnought Crimson` with base color `[0.55,0.08,0.12,1.0]` and the same metallic/roughness shape as existing unit materials;
- do not add or alter buffer/accessor binary data.

- [ ] **Step 5: Update Bevy loading and permanent mapping**

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 14;
```

Replace Task 1's temporary scene with:

```rust
UnitArchetype::Dreadnought => 13,
```

- [ ] **Step 6: Verify and commit**

```bash
python -m json.tool assets/models/mission_one.gltf >/dev/null
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib presentation::assets::tests
cargo test --test presentation_app
cargo test --all-targets
```

Expected: glTF parses, old/new asset tests agree on final counts, Controller remains bounded to nodes 64–69, and the presentation suite remains green.

```bash
git add assets/models/mission_one.gltf src/presentation/assets.rs src/presentation/battlefield.rs
git commit -m "feat: present the Dreadnought boss"
```

---

### Task 4: Close HPA-524 with validation, documentation, and full gates

**Files:**
- Create: `docs/validation/hpa-524.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify the spec/plan only if manual tuning changes locked authored values.

**Interfaces:**
- Consumes: completed Mission 6 implementation and existing validation-ledger style.
- Produces: reproducible HPA-524 evidence and truthful shipped documentation.

- [ ] **Step 1: Update shipped facts**

Document only concrete shipped behavior:

```text
- Missions 1–6 are authored; Seven is the handoff.
- Mission 6 adds one single-cell Dreadnought boss.
- Graviton Salvo is range 3–6 above half HP.
- Overload Salvo is range 2–4 at/below half HP.
- Threshold affects future planning only; committed intents stay locked.
- Dreadnought remains pushable.
- Continue derives authored-vs-handoff routing from mission_definition.
- Save/upgrade flow advances through Mission 6.
```

Do not document Mission 7 content, generic boss systems, or resistance as shipped.

- [ ] **Step 2: Record the full automated gates**

Create `docs/validation/hpa-524.md` and record pass/fail summaries plus final test count for:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

- [ ] **Step 3: Perform the real campaign/manual Mission 6 playthrough**

Run:

```bash
cargo run
```

Validate from the real campaign flow:

1. reach/start Mission 6 after Mission 5;
2. confirm opening Graviton Cross1 is readable;
3. execute the actual Vanguard/Interceptor/Vector Pulse redirection line and confirm Turnabout can be earned from boss friendly fire;
4. cross Dreadnought from 21+ HP to 20-or-less after an intent is committed and confirm the current telegraph remains Graviton;
5. confirm the next planning pass commits Overload range 2–4 and closes from range 5 into range 4;
6. confirm Overload never visibly hits the Dreadnought itself;
7. push Dreadnought once and confirm normal one-cell displacement;
8. defeat Dreadnought with at least one escort alive and confirm immediate victory;
9. finish aftermath/reward/upgrade, return to title, Continue, and confirm `MISSION 7 UNLOCKED` from persisted state;
10. record approximate encounter duration and any authored HP/damage/position tuning.

If pacing is poor, tune only Mission 6 authored values and update the spec/plan locked values in this same PR. Do not add systems as a tuning response.

- [ ] **Step 4: Re-run full gates after any tuning**

Run the same five commands from Step 2. Every gate must be green on the final product commit.

- [ ] **Step 5: Scope self-review**

Verify:

```text
- exactly one new boss archetype
- no threshold/phase registry or stored boss phase
- no new objective/optional-objective shape
- no displacement resistance
- no Mission 7 content
- Overload remains 2–4 and cannot self-overlap
- current-round intent remains immutable across threshold crossing
- Dreadnought 40 resolves before Controller 35
- Six is authored; Seven terminal
- Continue is data-driven
- spec/plan values match final tuned implementation
- one ticket / one PR preserved
```

- [ ] **Step 6: Commit closeout evidence**

```bash
git add README.md CLAUDE.md docs/validation/hpa-524.md docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md docs/superpowers/plans/2026-08-31-hpa-524-mission-6-dreadnought.md
git commit -m "docs: validate HPA-524 Mission 6"
```

- [ ] **Step 7: Keep implementation on this PR**

Do not open a second implementation PR. Mark this draft ready only after the final gates and manual ledger are complete.