# HPA-523 Missions 4–5 and Regular Enemy Roster Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Missions 4–5, Bulwark, and Controller as one player-visible HPA-523 slice, completing the six-enemy regular roster and advancing the campaign to the Mission 6 handoff.

**Architecture:** Extend the existing closed Rust domain model with one exact target-elimination objective and two explicit enemy archetypes. Reuse the existing weapon reach/alignment rule, one-cell displacement, authored mission openings, campaign flow, Bevy UI, and checked-in glTF. Keep all HPA-523 work in one PR and fix the first enemy-push lifecycle locally instead of adding generic objective, AI, status, resistance, or battle-transaction frameworks.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, ordinary Cargo tests plus the existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-30-hpa-523-missions-4-5-regular-roster-design.md`

## Global Constraints

- One HPA-523 ticket = one PR. Continue implementation on this planning branch/PR.
- Seven task commits stay in this PR; do not split the ticket into prerequisite or follow-up PRs.
- No new dependencies, crates, objective framework, AI policy framework, generic statuses, displacement-resistance model, physics, scripting/data format, save migration, VN art, asset pipeline, or transactional battle framework.
- Add exactly two regular enemies: Bulwark and Controller. The final regular roster is exactly six.
- Add exactly one primary objective shape: `EliminateTarget { target: UnitId }`.
- Bulwark remains pushable through existing displacement; there is no resistance system on `main`.
- Controller uses existing one-cell push. Dynamic and authored push centers reuse `weapon_reaches` and cannot commit an illegal diagonal/out-of-range target.
- If player displacement breaks a committed enemy push's live alignment/range, the locked attack still resolves damage against the current footprint occupant but skips the push.
- Do not add generic `phase = Player` error recovery: resetting only phase after partial enemy-resolution mutations would permit replay/double damage.
- Mission 1–5 opening legality is checked by one shared `#[cfg(test)]` helper; mission-specific tests still pin exact rows.
- `choose_enemy_destination` and `initiative` use explicit archetype matches; no wildcard may silently idle a future enemy.
- Mission 4 uses only existing blocking, hazard, explosive, collision, and push rules.
- Mission 5 exploits already-locked Artillery footprints; do not special-case friendly fire.
- Mission IDs become One–Six; One–Five are authored and Six is the HPA-523 terminal handoff.
- Reuse `vn/relay_nine_bg.png`, `vn/control_alert.png`, `vn/control_neutral.png`, and `vn/vanguard_neutral.png`; add no VN files.
- Reuse `assets/models/mission_one.gltf`; final counts are 13 scenes, 70 nodes, 13 meshes, 13 materials, 1 buffer.
- CI gates remain `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo llvm-cov --all-targets --lcov --output-path lcov.info`, and `cargo build --release`.

## Risks

- **Displaced committed pusher — highest risk.** Controller is the first enemy push weapon while the player already has multiple push weapons. A perpendicular player displacement after intent commitment must not make `resolve_enemy_phase` return `PushTargetNotAligned` or leave the battle in `EnemyResolution`. Task 2 must drive the real commit -> player displacement -> enemy resolution sequence and prove damage-only fallback plus normal phase advancement.
- **Mission 5 opening geometry is load-bearing.** Both Artillery `Cross1` footprints must include `(3,7)`, Gunner `(3,8) -> (2,7)` and Vanguard `(4,7) -> (3,5)` must remain legal public movement paths, and Controller `(3,6) -> (3,7)` must remain a legal push. Task 4's real `begin_round` + public movement/displacement test is required coverage and must not be replaced with `move_unit_direct_for_test`.

---

### Task 1: Add the closed target-elimination objective and correct objective HUD copy

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/presentation/ui.rs`
- Test: `src/domain/battle.rs`
- Test: `src/presentation/ui.rs`

**Interfaces:**
- Consumes: current `PrimaryObjective`, `BattleState::check_terminal_state`, `HudSnapshot::from_battle`, `format_track`.
- Produces: `PrimaryObjective::EliminateTarget { target: UnitId }`; `ObjectiveTrackSnapshot::Target { name, hp, max_hp }`; exact `TARGET ... HP ...` tracker copy; corrected Mission 2/3 primary lines.

- [ ] **Step 1: Write failing domain tests for target-only victory/failure**

Add to `src/domain/battle.rs` tests:

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

- [ ] **Step 2: Run the domain tests and confirm red**

```bash
cargo test --lib eliminate_target -- --nocapture
```

Expected: compile failure because `PrimaryObjective::EliminateTarget` does not exist.

- [ ] **Step 3: Add the minimal objective variant and terminal rule**

In `src/domain/model.rs`:

```rust
EliminateTarget { target: UnitId },
```

In `BattleState::check_terminal_state`:

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

- [ ] **Step 4: Put the pre-Mission-4 HUD tests in the inline `ui.rs` test module**

`set_rules_for_test` is `#[cfg(test)] pub(crate)`, so do **not** try to construct this fixture from `tests/presentation_app.rs` yet.

Add an inline target fixture:

```rust
#[test]
fn target_objective_tracks_target_without_enemy_count_and_formats_as_target() {
    let mut battle = mission_one(7);
    battle.set_rules_for_test(MissionRules {
        primary: PrimaryObjective::EliminateTarget { target: ids::STRIKER },
        optional: OptionalObjective::Turnabout,
        opening_plan: &[],
    });
    let base = *mission_definition(MissionId::One).unwrap();
    let definition = MissionDefinition {
        primary_objective: "Destroy the Striker.",
        ..base
    };

    let hud = HudSnapshot::from_battle(&battle, None, &definition);
    assert_eq!(
        hud.objective_track,
        Some(ObjectiveTrackSnapshot::Target {
            name: "Striker",
            hp: 12,
            max_hp: 12,
        })
    );
    assert_eq!(hud.primary, "Destroy the Striker.");
    assert_eq!(
        format_track(hud.objective_track.as_ref().unwrap()),
        "TARGET Striker HP 12/12"
    );
}
```

Also add explicit Mission 2/3 regressions using their real definitions:

```rust
let m2 = mission_two(7);
let m2_hud = HudSnapshot::from_battle(&m2, None, mission_definition(MissionId::Two).unwrap());
assert!(!m2_hud.primary.contains("remaining"));

let m3 = mission_three(7);
let m3_hud = HudSnapshot::from_battle(&m3, None, mission_definition(MissionId::Three).unwrap());
assert!(!m3_hud.primary.contains("remaining"));
```

Keep an `EliminateAllEnemies` assertion that still includes the remaining-enemy count.

- [ ] **Step 5: Implement target projection, exact tracker copy, and objective-specific main copy**

Extend `ObjectiveTrackSnapshot`:

```rust
Target {
    name: &'static str,
    hp: i16,
    max_hp: i16,
},
```

Map `EliminateTarget` to target HP. Build the primary line as:

```rust
let primary = match battle.rules().primary {
    PrimaryObjective::EliminateAllEnemies => {
        format!("{} · {remaining} remaining", definition.primary_objective)
    }
    _ => definition.primary_objective.to_owned(),
};
```

Add the formatting arm:

```rust
ObjectiveTrackSnapshot::Target { name, hp, max_hp } => {
    format!("TARGET {name} HP {hp}/{max_hp}")
}
```

`round_cap` remains only Protect/Intercept; `EliminateTarget` has no round cap.

- [ ] **Step 6: Run focused and full tests**

```bash
cargo fmt --check
cargo test --lib eliminate_target
cargo test --lib presentation::ui::tests
cargo test --all-targets
```

Expected: target tests pass; Mission 2/3 intentionally lose the misleading enemy-count suffix; elimination missions retain it.

- [ ] **Step 7: Commit the objective/HUD slice**

```bash
git add src/domain/model.rs src/domain/battle.rs src/presentation/ui.rs
git commit -m "feat: add target elimination objective"
```

---

### Task 2: Complete the regular roster, reuse reach validation, and make enemy push resolution safe

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/combat.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/mission/enemies.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/mission/mission_one.rs`
- Modify: `src/mission/mission_two.rs`
- Modify: `src/mission/mission_three.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/presentation/interaction.rs`
- Test: `src/domain/enemy.rs`
- Test: `src/mission/enemies.rs`
- Test: `src/mission/mission_one.rs`
- Test: `src/mission/mission_two.rs`
- Test: `src/mission/mission_three.rs`

**Interfaces:**
- Consumes: existing `weapon_reaches`, `attack_band_destination`, `distance_to_band`, `choose_target`, `BattleState::resolve_push`, authored `EnemyOpening`, and exhaustive presentation matches.
- Produces: `UnitArchetype::{Bulwark, Controller}`, `enemies::{bulwark, controller, bastion_cannon, impulse_projector}`, crate-visible `weapon_reaches`, shared opening validator, safe damage-only fallback after lost push alignment, exhaustive movement/initiative matches, and temporary scene mappings until Task 5.

- [ ] **Step 1: Write exact factory/weapon tests with the non-colliding enemy weapon name**

In `src/mission/enemies.rs` tests pin:

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
assert_eq!(controller.weapons, vec![ids::IMPULSE_PROJECTOR]);

let projector = impulse_projector();
assert_eq!(projector.id, ids::IMPULSE_PROJECTOR);
assert_eq!(projector.name, "Impulse Projector");
assert_eq!((projector.min_range, projector.max_range), (2, 4));
assert_eq!(projector.base_damage, 3);
assert_eq!(projector.hit_modifier, 10);
assert_eq!(projector.crit_chance, 0);
assert!(projector.push);
```

Keep Bastion Cannon pinned at range 1–3, damage 6, no push.

- [ ] **Step 2: Run factory tests and confirm red**

```bash
cargo test --lib mission::enemies::tests -- --nocapture
```

Expected: compile failures for new archetypes/factories/weapon IDs.

- [ ] **Step 3: Add the archetypes/factories and immediately keep exhaustive presentation matches compiling**

Add:

```rust
pub enum UnitArchetype {
    Vanguard,
    Gunner,
    Interceptor,
    Rifleman,
    Striker,
    Artillery,
    Flanker,
    Bulwark,
    Controller,
}
```

Enemy weapon IDs:

```rust
pub const BASTION_CANNON: WeaponId = WeaponId(205);
pub const IMPULSE_PROJECTOR: WeaponId = WeaponId(206);
```

Construct exact stats/weapons using existing `unit`, `stats`, and `weapon` helpers. Do not add fields to `UnitStats` or `WeaponSpec`.

Update `ui.rs` enemy-only pilot matches:

```rust
UnitArchetype::Rifleman
| UnitArchetype::Striker
| UnitArchetype::Artillery
| UnitArchetype::Flanker
| UnitArchetype::Bulwark
| UnitArchetype::Controller => false,
```

Use the same enemy set for `"[P] PILOT"` labels and `PilotSkillWrongUnit` in `interaction.rs`.

Use a temporary compile-safe visual mapping until Task 5:

```rust
UnitArchetype::Flanker
| UnitArchetype::Bulwark
| UnitArchetype::Controller => 10,
```

Do not change the glTF or scene count in Task 2.

- [ ] **Step 4: Reuse `weapon_reaches` instead of adding another alignment helper**

Change `src/domain/combat.rs`:

```rust
pub(crate) fn weapon_reaches(
    weapon: &WeaponSpec,
    attacker: GridPos,
    target: GridPos,
) -> bool {
    let distance = attacker.manhattan(target);
    distance >= weapon.min_range
        && distance <= weapon.max_range
        && (!weapon.push || attacker.x == target.x || attacker.y == target.y)
}
```

Import it in `enemy.rs` alongside `attack_footprint`/`preview_for_profile`.

Use it as the dynamic target filter:

```rust
.filter(|target| weapon_reaches(weapon, attacker.position, *target))
```

After `build_intent` chooses either dynamic or forced center, reject an authored/forced target that cannot be reached:

```rust
if !weapon_reaches(weapon, attacker.position, choice.center) {
    let distance = attacker.position.manhattan(choice.center);
    return Err(if distance < weapon.min_range || distance > weapon.max_range {
        BattleError::TargetOutOfRange {
            attacker: attacker_id,
            weapon: weapon_id,
            target: choice.center,
        }
    } else {
        BattleError::PushTargetNotAligned {
            attacker: attacker.position,
            target: choice.center,
        }
    });
}
```

No `push_target_aligned` helper is added.

- [ ] **Step 5: Extract one shared opening-legality assertion and delete three copies**

Add to `src/mission/mod.rs`:

```rust
#[cfg(test)]
pub(crate) fn assert_opening_plan_is_legal(battle: &BattleState) {
    let enemies: Vec<_> = battle
        .units()
        .filter(|unit| unit.faction == Faction::Enemy)
        .map(|unit| unit.id)
        .collect();
    assert_eq!(battle.rules().opening_plan.len(), enemies.len());

    for opening in battle.rules().opening_plan {
        let unit = battle.unit(opening.unit).expect("opening refs a real unit");
        assert_eq!(unit.faction, Faction::Enemy);
        assert!(opening.destination.manhattan(unit.position) <= unit.stats.movement);
        assert!(battle.board().contains(opening.destination));
        assert!(!battle.board().is_blocking(opening.destination));
        assert!(!battle.board().is_hazard(opening.destination));
        assert!(battle.units().all(|other| {
            other.id == opening.unit || other.position != opening.destination
        }));

        if let Some(target_id) = opening.target {
            let target = battle.unit(target_id).expect("opening target exists");
            assert_eq!(target.faction, Faction::Player);
            let weapon = unit
                .weapons
                .first()
                .and_then(|weapon| battle.weapon(*weapon))
                .expect("opening unit has first weapon");
            assert!(
                weapon_reaches(weapon, opening.destination, target.position),
                "opening target must be in range and push-aligned"
            );
        }
    }
}
```

Import the minimum test-only types/functions needed under `#[cfg(test)]`.

Replace the copied legality loops in Mission 1, 2, and 3 tests with:

```rust
assert_opening_plan_is_legal(&battle);
```

Keep each mission's separate exact opening-row assertions; only the duplicated generic legality body is removed.

- [ ] **Step 6: Write Controller/Bulwark planning tests and remove wildcard safety holes**

Inside `src/domain/enemy.rs` tests pin:

```text
A. Controller aligned lane exists -> chooses reachable aligned range-2..4 cell.
B. Controller no aligned lane -> deterministic attack-band fallback.
C. Dynamic push intent center satisfies weapon_reaches.
D. Forced diagonal push target is rejected before commitment.
E. Bulwark with a better Move-1 attack-band cell leaves origin.
F. Initiative is Controller35 / Striker30 / Flanker25 / Rifleman20 / Bulwark15 / Artillery10.
```

For the forced-target case:

```rust
let error = build_intent(&battle, controller_id, Some(GridPos::new(2, 2))).unwrap_err();
assert!(matches!(error, BattleError::PushTargetNotAligned { .. }));
```

Replace `choose_enemy_destination`'s wildcard with explicit arms:

```text
Flanker -> existing flanker_destination
Rifleman | Striker | Bulwark -> existing attack_band_destination
Artillery -> existing Artillery branch
Controller -> controller_destination
Vanguard | Gunner | Interceptor -> origin
```

Replace `initiative` wildcard with:

```rust
match unit.archetype {
    UnitArchetype::Controller => 35,
    UnitArchetype::Striker => 30,
    UnitArchetype::Flanker => 25,
    UnitArchetype::Rifleman => 20,
    UnitArchetype::Bulwark => 15,
    UnitArchetype::Artillery => 10,
    UnitArchetype::Vanguard | UnitArchetype::Gunner | UnitArchetype::Interceptor => 0,
}
```

- [ ] **Step 7: Add the displaced-Controller regression before changing resolution**

Build a small enemy-domain fixture with the player Vanguard on the Controller's committed vertical lane and Interceptor positioned to push the Controller horizontally. Use an authored opening so `begin_round()` commits the Controller intent first.

The test sequence must be:

```text
1. begin_round -> Controller commits push center on Vanguard.
2. player uses existing resolve_push test seam to move Controller perpendicular to the committed lane.
3. finish every living player activation with Guard so resolve_enemy_phase is legal.
4. call resolve_enemy_phase.
5. assert Ok, normal next Player phase, AttackRolled still targets Vanguard, and no enemy UnitPushed event occurs.
```

Use the same bounded seed-sweep pattern already used by the Aegis regression to choose one deterministic seed where the Controller hit lands, then assert the returned events contain enemy `DamageApplied` to Vanguard. The purpose is to prove damage-only fallback, not RNG behavior.

Before the fix, this test must fail with `PushTargetNotAligned` and leave the phase in `EnemyResolution`.

- [ ] **Step 8: Implement damage-only fallback when live push reach is lost**

In `resolve_enemy_profile_against`, leave hit/damage resolution unchanged. Gate only the push:

```rust
let live_push_reaches = self
    .unit(attacker)
    .zip(self.unit(target))
    .and_then(|(attacker_state, target_state)| {
        self.weapon(profile.weapon).map(|weapon| {
            weapon_reaches(
                weapon,
                attacker_state.position,
                target_state.position,
            )
        })
    })
    .unwrap_or(false);

if profile.push
    && self.unit(attacker).is_some_and(|unit| !unit.is_knocked_out())
    && self.unit(target).is_some_and(|unit| !unit.is_knocked_out())
    && live_push_reaches
{
    events.extend(self.resolve_push(attacker, target)?);
}
```

Do not reset `BattlePhase` on arbitrary errors. The expected displaced-pusher path is now non-erroring; a generic phase reset after partial mutations is explicitly out of scope.

- [ ] **Step 9: Run roster, authoring, displacement, and all-target regressions**

```bash
cargo fmt --check
cargo test --lib mission::enemies::tests
cargo test --lib mission_one_opening_rows_reference_legal_units_and_destinations
cargo test --lib mission_two_opening_rows_reference_legal_units_and_destinations
cargo test --lib mission_three_opening_rows_reference_legal_units_and_destinations
cargo test --lib domain::enemy::tests
cargo test --lib domain::environment::tests
cargo test --all-targets
```

Expected: all pass, including the displaced Controller resolve path and Task 2's temporary scene-10 mapping.

- [ ] **Step 10: Commit the roster/domain slice**

```bash
git add src/domain/model.rs src/domain/combat.rs src/domain/enemy.rs \
  src/mission/enemies.rs src/mission/mod.rs src/mission/mission_one.rs \
  src/mission/mission_two.rs src/mission/mission_three.rs \
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
- Consumes: `build_player_squad`, shared enemy factories, `assert_opening_plan_is_legal`, `MissionDefinition`, `EliminateTarget`, `Turnabout`.
- Produces: `MISSION_FOUR_DEFINITION`, `mission_four_for_campaign`, `MissionId::{Five, Six}` with Six still a handoff.

- [ ] **Step 1: Add failing Mission 4 authoring tests**

Pin exact authored data:

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

After the exact row assertions, call:

```rust
assert_opening_plan_is_legal(&battle);
```

Do not add another hand-written legality loop.

- [ ] **Step 2: Add failing environmental geometry tests**

Drive the real opening:

```rust
let mut battle = mission_four(7);
battle.begin_round().unwrap();
```

Explosive path:

```rust
battle.begin_activation(ids::GUNNER).unwrap();
let preview = battle
    .preview_attack(ids::GUNNER, squad::ids::RAIL_RIFLE, GridPos::new(3, 4))
    .unwrap();
assert_eq!(preview.target, GridPos::new(3, 4));
```

After proving the shot is legal, call `damage_explosive` directly to remove RNG from the environment assertion and verify `ExplosionTriggered.footprint` contains `(4,4)` plus Bulwark receives `DamageSource::Explosion`.

Separate push geometry fixture:

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

Use only existing VN file paths and exact dialogue from the spec.

- [ ] **Step 6: Move Continue's temporary handoff forward**

```rust
Ok(MissionId::Two | MissionId::Three | MissionId::Four) => {
    next_state.set(GameScreen::Upgrade)
}
Ok(MissionId::Five | MissionId::Six) => next_state.set(GameScreen::NextMission),
```

Update stale comments that still call Four terminal. Task 4 authors Five and leaves only Six in the handoff branch.

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

### Task 4: Author Mission 5 and pin the load-bearing Controller crossfire setup

**Files:**
- Create: `src/mission/mission_five.rs`
- Modify: `src/mission/mod.rs`
- Modify: `src/presentation/campaign_ui.rs`
- Test: `src/mission/mission_five.rs`
- Test: `tests/campaign_flow.rs`
- Test: `tests/campaign_model.rs`
- Test: `tests/campaign_persistence.rs`

**Interfaces:**
- Consumes: locked `AttackIntent` footprints, public movement, current `resolve_push`, `EliminateAllEnemies`, `VictoryByRound`, shared opening validator.
- Produces: `MISSION_FIVE_DEFINITION`, `mission_five_for_campaign`, Five -> Six handoff, and durable evidence for the `(3,7)` crossfire line and 4/5 damage payoff.

- [ ] **Step 1: Add failing Mission 5 authoring tests for the revised formation**

Pin:

```text
Board 9×9
Players V(4,7) G(3,8) I(5,8)
Blocking (1,4),(7,4),(1,5),(7,5)
Artillery51 (3,0) stays, target Gunner
Artillery52 (7,2) stays, target Vanguard
Bulwark53 (0,7)->(1,7), target Vanguard
Controller54 (3,5)->(3,6), target Gunner
Flanker55 (8,7)->(6,7), target Interceptor
Primary EliminateAllEnemies
Optional VictoryByRound 4
Reward 700+200
Unlock Six
```

Pin exact opening rows, then call:

```rust
assert_opening_plan_is_legal(&battle);
```

The shared validator must prove Controller `(3,6)` -> Gunner `(3,8)` is range 2 and aligned, and Bulwark `(1,7)` -> Vanguard `(4,7)` is range 3.

- [ ] **Step 2: Write the required real-opening/public-movement crossfire regression**

Do not use `move_unit_direct_for_test`.

```rust
let mut battle = mission_five(7);
battle.begin_round().unwrap();

let artillery_a = battle.intent_for(ids::ARTILLERY_A).unwrap();
let artillery_b = battle.intent_for(ids::ARTILLERY_B).unwrap();
assert!(artillery_a.footprint.contains(&GridPos::new(3, 7)));
assert!(artillery_b.footprint.contains(&GridPos::new(3, 7)));
assert_eq!(battle.unit(ids::CONTROLLER).unwrap().position, GridPos::new(3, 6));
```

Prove the exact public movement paths:

```rust
battle.begin_activation(ids::GUNNER).unwrap();
battle.move_unit(ids::GUNNER, GridPos::new(2, 7)).unwrap();
battle.choose_reaction(ids::GUNNER, Reaction::Guard).unwrap();
battle.finish_activation(ids::GUNNER).unwrap();

battle.begin_activation(ids::VANGUARD).unwrap();
battle.move_unit(ids::VANGUARD, GridPos::new(3, 5)).unwrap();
let ram = battle
    .preview_attack(ids::VANGUARD, squad::ids::REPULSOR_RAM, GridPos::new(3, 6))
    .unwrap();
assert_eq!(ram.normal_damage, 4);
assert_eq!(ram.push_destination, Some(GridPos::new(3, 7)));

let push_events = battle.resolve_push(ids::VANGUARD, ids::CONTROLLER).unwrap();
assert_eq!(battle.unit(ids::CONTROLLER).unwrap().position, GridPos::new(3, 7));
assert!(push_events.iter().any(|event| matches!(event, BattleEvent::UnitPushed { .. })));
```

The geometry test uses direct `resolve_push` after the real preview so RNG cannot make the authoring assertion flaky.

- [ ] **Step 3: Pin the Controller's vacated push intent and Mortar payoff**

After Gunner vacates `(3,8)` and Controller is at `(3,7)`:

```rust
let controller_events = battle.resolve_intent_for_test(ids::CONTROLLER).unwrap();
assert!(controller_events.iter().any(|event| matches!(
    event,
    BattleEvent::AttackHitEmpty { attacker, cell, .. }
        if *attacker == ids::CONTROLLER && *cell == GridPos::new(3, 8)
)));
```

Resolve Artillery A/B with deterministic seeds chosen by a bounded sweep when needed. For each battery assert `AttackRolled` targets Controller at the committed shared cell. On a hit assert:

```rust
BattleEvent::DamageApplied {
    target: ids::CONTROLLER,
    amount: 5,
    source: DamageSource::EnemyWeapon(_, _),
    ..
}
```

This pins the authored payoff:

```text
Repulsor Ram preview against Armor 1 = 4
Siege Mortar normal hit against Armor 1 = 5
Controller HP = 9
```

One Mortar hit after the real Ram would KO Controller; the test does not add a friendly-fire special case or force hit behavior in production.

- [ ] **Step 4: Add Round-4 bonus boundary tests**

Using the same durable round-step style as Mission 3:

```text
victory at round <= 4 -> optional_complete true
victory at round 5 -> optional_complete false
```

Knock out remaining enemies through the existing damage test seam so `check_terminal_state` evaluates the actual optional rule.

- [ ] **Step 5: Run Mission 5 tests and confirm red**

```bash
cargo test --lib mission::mission_five -- --nocapture
```

Expected: module/definition is missing.

- [ ] **Step 6: Implement Mission 5 exactly as revised**

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

Use shared enemy factories/weapons, no new hazards/props, no Artillery special case, and only existing VN assets/dialogue from the spec.

- [ ] **Step 7: Register Five and make Six the only handoff**

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

- [ ] **Step 8: Extend campaign/persistence tests through Six**

Pin:

```text
One -> Two -> Three -> Four -> Five -> Six
Base rewards through Five = 2500
Max optional through Five = 700
Max total through Five = 3200
```

Persist an upgrade before Mission 4, reload, construct Mission 4/5 with that state, finish Mission 5, save/reload Six, and assert upgrade levels plus intended credits survive.

- [ ] **Step 9: Run gates**

```bash
cargo fmt --check
cargo test --lib mission::mission_five
cargo test --test campaign_model
cargo test --test campaign_flow
cargo test --test campaign_persistence
cargo test --all-targets
```

- [ ] **Step 10: Commit Mission 5**

```bash
git add src/mission/mission_five.rs src/mission/mod.rs src/presentation/campaign_ui.rs \
  tests/campaign_flow.rs tests/campaign_model.rs tests/campaign_persistence.rs
git commit -m "feat: add Mission 5 artillery assault"
```

---

### Task 5: Append Bulwark/Controller glTF scenes and replace temporary scene mappings

**Files:**
- Modify: `assets/models/mission_one.gltf`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Test: `src/presentation/assets.rs`
- Test: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: Task 2's temporary scene-10 mapping, `MissionAssets`, real Mission 4/5 definitions, current single embedded glTF buffer/accessors.
- Produces: scene 11 Bulwark, scene 12 Controller, scene count 13, permanent `scene_index` mappings, real Mission 4 target-HUD integration assertion.

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

- [ ] **Step 3: Append two scenes without another buffer/file**

```text
Bulwark root 56 + children 57–62 -> mesh 11
Controller root 63 + children 64–69 -> mesh 12
```

Copy exact Flanker child transforms 50–55, changing only mesh indices. Add meshes/materials 11 and 12 using the same cube accessors. Keep the existing single embedded buffer and accessor arrays.

- [ ] **Step 4: Replace only the temporary visual mappings**

Change:

```rust
pub const MISSION_ONE_SCENE_COUNT: usize = 13;
```

Replace Task 2's combined scene-10 arm with:

```rust
UnitArchetype::Flanker => 10,
UnitArchetype::Bulwark => 11,
UnitArchetype::Controller => 12,
```

The `ui.rs` and `interaction.rs` exhaustive enemy arms were completed in Task 2 and are not Task 5 work. Do not add a per-archetype scale table.

- [ ] **Step 5: Add real Mission 4/5 presentation assertions**

In `tests/presentation_app.rs`, now that Mission 4 exists, pin the real target HUD:

```text
TARGET Gate Bulwark HP 16/16
```

Also assert:

```rust
assert_eq!(scene_index(UnitArchetype::Bulwark), 11);
assert_eq!(scene_index(UnitArchetype::Controller), 12);
```

For Mission 5, assert both Artillery intents appear in the threat list and the primary line includes remaining-enemy count because its primary is `EliminateAllEnemies`.

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

Expected: definition-driven code should already handle Four/Five. If compilation exposes a remaining exhaustive MissionId match or literal Four handoff, update only that concrete branch; do not introduce a router/registry abstraction.

- [ ] **Step 3: Apply only bounded corrections named by failing tests**

Allowed production corrections are limited to:

```text
- an exhaustive MissionId match that lacks Five/Six
- stale literal Four-terminal handoff logic/copy
- a hard-coded mission lookup where mission_definition(next_mission) should already be used
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

Keep the commit even when production code needs no correction; the integration tests are the Task 6 deliverable.

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
- Mission 5: locked-artillery crossfire exploitation against the displaced Controller
- Bulwark has no displacement immunity
- Controller uses Impulse Projector push-only behavior, no status system
- displaced committed Controller push becomes damage-only if live push reach is lost
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
- Bulwark reads visually heavier than regular enemies
- Controller Impulse Projector telegraph is legible
- Gunner can use explosive (3,4) to splash Bulwark
- Vanguard can push Bulwark (4,4)->(4,3) onto hazard
- Bulwark KO wins with escorts alive
- Chain Reaction reward appears for qualifying enemy/environment damage
- encounter remains a short tactical session
```

- [ ] **Step 4: Manually validate Mission 5**

Record:

```text
- both Artillery Cross1 footprints include (3,7)
- Gunner (3,8)->(2,7) is legal
- Vanguard (4,7)->(3,5) is legal
- Controller (3,6)->(3,7) push is legal
- Ram preview against Controller is 4 normal damage
- each Mortar normal hit against Controller is 5 damage
- Controller's committed (3,8) attack is harmless after Gunner vacates
- committed Artillery crossfire materially damages/usually finishes the displaced Controller
- Rapid Break rewards <= Round 4 but is not a failure deadline
- mixed telegraphs remain readable and encounter stays short
```

Also manually displace a Controller perpendicular to a committed push lane in either Mission 4 or a debug/test setup and confirm the enemy attack resolves without a stuck `EnemyResolution` phase.

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
- displaced committed-pusher automated/manual evidence
- shared Mission 1-5 opening-validator evidence
- Mission 4 automated geometry + manual evidence
- Mission 5 real begin_round `(3,7)` crossfire + public movement/displacement + 4/5 damage evidence
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

- [ ] HPA-523 is one PR with seven implementation task commits.
- [ ] Only one new primary objective exists: `EliminateTarget`.
- [ ] No displacement-resistance/status/AI/objective/transaction framework was added.
- [ ] `weapon_reaches` is the shared range/push-alignment predicate for player/enemy reach validation rather than a second helper.
- [ ] Mission 1–5 tests call one shared opening-legality validator; the copied Mission 1/2/3 generic legality bodies are removed.
- [ ] Bulwark is HP16 / Armor4 / Move1, pushable, initiative15, and has a tested later-round attack-band movement path.
- [ ] Controller is HP9 / Armor1 / Move2 with `Impulse Projector` range2–4 damage3 Push1, initiative35.
- [ ] Dynamic and authored Controller intents cannot commit an illegal push target.
- [ ] Player displacement that breaks a committed Controller push's live reach produces damage-only resolution, no enemy push, no error, and no stuck `EnemyResolution` phase.
- [ ] `choose_enemy_destination` and `initiative` have no wildcard archetype fallthrough; only Vanguard/Gunner/Interceptor explicitly use neutral fallback values.
- [ ] Task 2's `cargo test --all-targets` is green before real Bulwark/Controller glTF scenes exist; temporary scene 10 mapping is replaced in Task 5.
- [ ] Mission 2/3 main HUD lines intentionally no longer append `N remaining`; their objective trackers still communicate HP/distance.
- [ ] Target tracker renders `TARGET {name} HP {hp}/{max_hp}` and Mission 4 uses the real Gate Bulwark value.
- [ ] Mission 4 preserves both authored environmental solutions and wins on Bulwark KO with escorts alive.
- [ ] Mission 5's real opening preserves shared `(3,7)` dual-Artillery footprint, exact-fit public movement paths, and Controller displacement line.
- [ ] Mission 5 pins 4 normal Ram damage and 5 normal damage per Mortar against Controller; its own vacated `(3,8)` push intent is harmless.
- [ ] Mission 5's Round-4 condition is optional pressure, not a primary deadline.
- [ ] Regular roster totals exactly six archetypes.
- [ ] glTF final structure is 13 scenes / 70 nodes / 13 meshes / 13 materials / 1 buffer.
- [ ] One–Five are authored; Six is the only terminal handoff.
- [ ] Base rewards through Mission 5 total exactly 2500.
- [ ] Save, upgrades, Continue, restart, VN, briefing, results, and upgrade flow remain continuous through Mission 5.
- [ ] README/CLAUDE/validation ledger match shipped behavior.
- [ ] fmt, strict Clippy, all-target tests/coverage, and release build are green.