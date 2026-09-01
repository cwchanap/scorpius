# HPA-524 Mission 6 Dreadnought Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Mission 6 and the first Dreadnought boss as one player-visible HPA-524 slice, with one half-HP behavior change on the existing locked-intent path and a persisted Mission 7 handoff.

**Architecture:** Add one concrete `Dreadnought` archetype. Extend `unit_weapon` so Dreadnought uses slot 1 at/below half HP, then make `build_intent` use that same selector. Mission 6 owns boss values/content locally. Keep Mission 1 special in Continue; all other routing derives from `mission_definition`, matching Proceed.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, checked-in glTF, Cargo tests plus existing Bevy `App` integration tests.

**Spec:** `docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md`

## Global Constraints

- One ticket = one PR; implement on this branch.
- One normal single-cell `UnitState`; no boss runtime/phase/threshold registry.
- HP 21–40 -> Graviton; HP 0–20 -> Overload.
- Current committed intent never changes after threshold crossing.
- Boss remains pushable; no resistance system.
- Mission 6 owns boss factory/weapons locally; regular enemy factories remain shared.
- Mission 6: 9×9, existing blocking only, no hazards/explosives, `EliminateTarget`, `Turnabout`, 800 + 250 credits.
- One–Six authored; Seven terminal.
- Continue: One -> story; later authored -> Upgrade; unauthored -> NextMission.
- Final glTF counts: 14 scenes / 77 nodes / 14 meshes / 14 materials / 1 buffer.
- No new objective/status/AI/progression/save framework, dependency/crate, Mission 7 content, or second PR.

---

### Task 1: Dreadnought threshold behavior

**Files:** `src/domain/model.rs`, `src/domain/enemy.rs`, `src/presentation/battlefield.rs`, `src/presentation/ui.rs`, `src/presentation/interaction.rs`.

**Produces:** `Dreadnought`, half-HP selector, locked-intent reuse, attack-band movement, initiative 40.

- [ ] **Step 1: Write failing threshold tests**

Use test constants `DREADNOUGHT=90`, `TEST_PLAYER=91`, `GRAVITON=290`, `OVERLOAD=291`. Create a 7×7 fixture with boss `(3,1)`, player `(3,5)`, stats `40/3/1/90/5/0`, and weapons:

```rust
squad::weapon(GRAVITON, "Graviton Salvo", 3, 6, WeaponShape::Cross1, 8, 10, 5, 0, false, false)
squad::weapon(OVERLOAD, "Overload Salvo", 1, 4, WeaponShape::Cross1, 10, 10, 10, 0, false, false)
```

Pin:

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

Create a second fixture with boss `(3,0)`, player `(3,5)`, same stats/weapons:

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

- [ ] **Step 2: Confirm red**

```bash
cargo test --lib dreadnought -- --nocapture
```

- [ ] **Step 3: Implement selector + intent reuse**

Append `Dreadnought` to `UnitArchetype`.

```rust
fn unit_weapon<'a>(battle: &'a BattleState, unit: &UnitState) -> Result<&'a WeaponSpec, BattleError> {
    let index = match unit.archetype {
        UnitArchetype::Dreadnought if unit.hp * 2 <= unit.stats.max_hp => 1,
        _ => 0,
    };
    let id = unit.weapons.get(index).copied().ok_or(BattleError::InvalidTarget(unit.position))?;
    battle.weapon(id).ok_or(BattleError::UnknownWeapon(id))
}
```

At `build_intent` start:

```rust
let attacker = battle.unit(attacker_id).ok_or(BattleError::UnknownUnit(attacker_id))?;
let weapon = unit_weapon(battle, attacker)?;
let weapon_id = weapon.id;
```

- [ ] **Step 4: Add movement/initiative and exhaustive presentation matches**

Group Dreadnought with Rifleman/Striker/Bulwark attack-band movement; initiative 40. Temporarily map scene index 11. Add Dreadnought to enemy-only branches in UI/interaction.

- [ ] **Step 5: Verify + commit**

```bash
cargo fmt --check
cargo test --lib dreadnought
cargo test --all-targets
git add src/domain/model.rs src/domain/enemy.rs src/presentation/battlefield.rs src/presentation/ui.rs src/presentation/interaction.rs
git commit -m "feat: add dreadnought threshold behavior"
```

---

### Task 2: Author Mission 6 and register Six cleanly

**Files:** create `src/mission/mission_six.rs`; modify `src/mission/mod.rs`, `mission_two.rs`, `mission_three.rs`, `mission_four.rs`, `mission_five.rs`, `src/presentation/campaign_ui.rs`.

- [ ] **Step 1: Write failing authoring tests**

Pin 9×9; blocking `(2,4) (6,4) (2,5) (6,5)`; no hazards/explosives; exactly four enemies; boss stats `40/3/1`; weapons `[207,208]`; `EliminateTarget(DREADNOUGHT)`; `Turnabout`; exact opening rows:

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

- [ ] **Step 3: Register Seven and fix all library pins in the same change**

Add `pub mod mission_six`, `MissionId::Seven`, display `7`, `Six => Some(MISSION_SIX_DEFINITION)`, `Seven => None`.

In Missions 2–5, replace `mission_definition(Six).is_none()` with `mission_definition(Seven).is_none()`. In Mission 5 also assert Six is now authored.

- [ ] **Step 4: Make Continue data-driven**

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

- [ ] **Step 5: Implement local Mission 6 content**

IDs: Dreadnought 61, Bulwark 62, Controller 63, Rifleman 64, Graviton 207, Overload 208.

Boss factory:

```rust
unit(ids::DREADNOUGHT, "Dreadnought", UnitArchetype::Dreadnought, Faction::Enemy,
    stats(40, 3, 1, 90, 5, 0), GridPos::new(4, 1),
    vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO])
```

Weapons use the locked values from Task 1. Reuse regular escort factories.

Deployment `(4,7)/(3,8)/(5,8)`. Opening rows are the four pinned tuples above. Rules: target Dreadnought + Turnabout. Definition: title `Mission 6 — Break the Dreadnought`, rewards 800/250, unlock Seven.

Dialogue:

```text
Control: A Dreadnought is anchoring the line. Its main battery commits before we move.
Vanguard: Then the escorts are ammunition.
Control: Exactly. Below half integrity the battery overloads and the Dreadnought will close in.

Vanguard: Dreadnought down. Their line is collapsing.
Control: One command unit remains. Mission 7 is the final push.
```

- [ ] **Step 6: Add opening validator + public manipulation helper**

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
    let preview = battle.preview_attack(ids::INTERCEPTOR, squad::ids::VECTOR_PULSE, GridPos::new(6, 7)).unwrap();
    assert_eq!(preview.push_destination, Some(GridPos::new(5, 7)));
    battle.resolve_push(ids::INTERCEPTOR, ids::CONTROLLER).unwrap();
    assert_eq!(battle.unit(ids::CONTROLLER).unwrap().position, GridPos::new(5, 7));
}
```

Also call `assert_opening_plan_is_legal(&mission_six(7))`.

- [ ] **Step 7: Pin geometry, Turnabout, target victory, and pushability**

Geometry test resolves Controller's intent after the helper and requires `AttackHitEmpty` at `(4,7)`.

Turnabout test loops seeds `0..256`, runs the helper, resolves Dreadnought intent, and succeeds only when events include both `AttackRolled` against Controller and `OptionalObjectiveCompleted`.

Target-victory test directly KOs Dreadnought and asserts victory with Bulwark alive.

Push test moves Vanguard `(3,3)` / Dreadnought `(4,3)`, calls `resolve_push`, and asserts Dreadnought `(5,3)` plus `UnitPushed`.

- [ ] **Step 8: Verify library + commit**

```bash
cargo fmt --check
cargo test --lib mission::mission_six
cargo test --lib
git add src/mission/mod.rs src/mission/mission_six.rs src/mission/mission_two.rs src/mission/mission_three.rs src/mission/mission_four.rs src/mission/mission_five.rs src/presentation/campaign_ui.rs
git commit -m "feat: author Mission 6 Dreadnought encounter"
```

---

### Task 3: Campaign/save integration through Six

**Files:** `tests/campaign_model.rs`, `tests/campaign_flow.rs`, `tests/campaign_persistence.rs`.

- [ ] **Step 1: Update campaign model**

Pin Six authored, unlock Seven, title/rewards 800/250, Seven None, and base rewards through Six = 3300.

- [ ] **Step 2: Complete Mission 6 with the real API**

```rust
let receipt = complete_current_mission(
    &mut session,
    mission_definition(MissionId::Six).unwrap(),
    MissionResult { victory: true, optional_complete: false, rounds: 4 },
).unwrap();
assert_eq!((receipt.base_reward, receipt.optional_reward), (800, 0));
assert_eq!(session.state.as_ref().unwrap().next_mission, MissionId::Seven);
assert_eq!(session.state.as_ref().unwrap().credits, 3300);
```

Add an optional-complete case asserting total reward 1050.

- [ ] **Step 3: Update routing blast radius**

```rust
assert_eq!(route_continue(MissionId::Six), Some(GameScreen::Upgrade));
assert_eq!(route_continue(MissionId::Seven), Some(GameScreen::NextMission));
```

Proceed at Six -> PreMissionStory. Proceed at Seven -> NextMission. Update the existing full-flow Continue-at-Six assertion to Upgrade.

- [ ] **Step 4: Persist Seven**

Round-trip a `CampaignState { next_mission: Seven, credits: 1234, ...non-default upgrades... }`; assert exact equality after load. No migration.

- [ ] **Step 5: Pin Mission 6 briefing/dialogue**

Assert title, 800/+250 copy, first pre-mission speaker Control, second aftermath speaker Control.

- [ ] **Step 6: Verify + commit**

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

### Task 4: Append Dreadnought visual and repair glTF test blast radius

**Files:** `assets/models/mission_one.gltf`, `src/presentation/assets.rs`, `src/presentation/battlefield.rs`.

- [ ] **Step 1: Update old tests first**

Change existing global counts to 14/77/14/14 and bound Controller loop:

```rust
for (index, part) in nodes.iter().enumerate().skip(64).take(6) {
    assert_eq!(part["mesh"], 12, "node {index} must use mesh 12");
}
```

- [ ] **Step 2: Add Dreadnought structure test**

Pin scene 13, root 70, children 71–76, scale 1.12, mesh/material 13, crimson color `[0.55,0.08,0.12,1.0]`, final counts, one buffer.

- [ ] **Step 3: Confirm red**

```bash
cargo test --lib presentation::assets::tests -- --nocapture
```

- [ ] **Step 4: Append scene/root/parts/mesh/material**

Copy Bulwark's part-transform structure into root 70 + parts 71–76, point parts to mesh 13, append mesh/material 13, leave accessors/buffer untouched.

- [ ] **Step 5: Update loading/mapping**

`MISSION_ONE_SCENE_COUNT = 14`; `Dreadnought => 13`.

- [ ] **Step 6: Verify + commit**

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

### Task 5: Closeout

**Files:** create `docs/validation/hpa-524.md`; update `README.md`, `CLAUDE.md`; update spec/plan only if tuning changes locked values.

- [ ] **Step 1: Update shipped facts**

Document Missions 1–6 authored, Seven handoff, one Dreadnought, Graviton/Overload threshold, locked intent semantics, pushable boss, save/upgrade flow through Six.

- [ ] **Step 2: Record full gates**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo build --release
```

Record final test count.

- [ ] **Step 3: Manual campaign validation**

Verify real Mission 5 -> Mission 6 flow; readable Graviton; Controller redirection/Turnabout; immutable committed intent across threshold; next Overload + range-5 close step; boss pushability; target-only victory; aftermath/reward/upgrade; persisted Seven Continue handoff; encounter duration.

Tune only authored values if pacing is poor.

- [ ] **Step 4: Re-run full gates after tuning**

Run the same five commands from Step 2.

- [ ] **Step 5: Scope self-review**

Verify one boss archetype, no threshold framework, no resistance, no Mission 7 content, reviewed regressions covered, Six authored/Seven terminal, data-driven Continue, glTF old tests updated, one PR.

- [ ] **Step 6: Commit closeout**

```bash
git add README.md CLAUDE.md docs/validation/hpa-524.md docs/superpowers/specs/2026-08-31-hpa-524-mission-6-dreadnought-design.md docs/superpowers/plans/2026-08-31-hpa-524-mission-6-dreadnought.md
git commit -m "docs: validate HPA-524 Mission 6"
```

- [ ] **Step 7: Keep implementation on this PR**

Mark ready only after final gates/manual evidence; do not open another implementation PR.