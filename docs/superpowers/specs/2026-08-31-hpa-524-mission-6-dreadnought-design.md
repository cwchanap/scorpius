# HPA-524 Mission 6 Dreadnought Design

## Outcome

Ship Mission 6 as the first boss encounter while keeping the boss on the existing single-cell enemy/combat path. The fight should feel different because the Dreadnought commits large area attacks and switches to a stronger close-pressure salvo at half HP, not because Scorpius gains a boss engine.

Keep this as one HPA-524 PR. Reuse the deterministic battle state, locked enemy intents, `EliminateTarget`, `Turnabout`, authored mission openings, campaign save/progression flow, Bevy UI, and checked-in glTF. Do not add phase scripting, a boss component hierarchy, generic threshold registry, new objective shape, status framework, second battle runtime, or save migration.

## Existing seams

- Every enemy is a normal `UnitState` on one grid cell.
- Enemy movement calls `unit_weapon`; `build_intent` still reads `.first()` independently and must reuse the same selector.
- Committed attacks snapshot weapon/profile/origin/footprint and resolve against live occupants without retargeting.
- `EliminateTarget` + `Turnabout` already ship together in Mission 4.
- Friendly fire through committed footprints, collision, and one-cell push already exist.
- Missions own board/deployment/opening/dialogue/rewards/`MissionDefinition`.
- Mission 5 already persists `MissionId::Six`.
- `Proceed` already uses `mission_definition(next_mission).is_some()`; `Continue` is the leftover per-mission list.
- The checked-in glTF already carries all unit scenes.

## Selected design

Add exactly one `UnitArchetype::Dreadnought`. Mission 6 gives it two ordinary enemy weapons in fixed slots. For this archetype only, `unit_weapon` selects slot 1 when:

```rust
unit.hp * 2 <= unit.stats.max_hp
```

With max HP 40:

```text
HP 21–40 -> Graviton Salvo
HP 0–20  -> Overload Salvo
```

No phase state is stored. The same selected weapon drives future intent commitment and ordinary attack-band movement. Mission 7 is the second concrete consumer and is the earliest point to consider sharing threshold structure.

Rejected: generic boss/phase data, boss runtime/controller, scripts, detachable parts, invulnerability, multi-tile occupancy, callbacks, resistance framework.

### Locked threshold contract

```text
Round N planning: 21 HP -> Graviton committed
Player phase: boss drops below 20
Round N resolution: committed Graviton remains unchanged
Round N+1 planning: Overload selected
```

No phase-change event or boss banner. The next normal telegraph exposes the new weapon/range/damage.

### Movement and initiative

Dreadnought initiative: **40**.

Later-round movement uses the existing branch:

```text
Rifleman | Striker | Bulwark | Dreadnought -> attack_band_destination
```

Overload max range 4 is the close-pressure behavior. At 20 HP and distance 5, a focused test must prove the boss steps one cell closer.

Dreadnought remains a normal push target.

## Boss values

Mission 6 owns this factory locally in `mission_six.rs`; do not put a one-consumer boss in shared `mission/enemies.rs`.

```text
Dreadnought
HP 40 / Armor 3 / Move 1 / Accuracy 90 / Evasion 5 / EN 0 / Initiative 40

Weapon 207 — Graviton Salvo
Range 3–6 / Cross1 / Damage 8 / Hit +10 / Crit 5% / EN 0 / no push / no counter

Weapon 208 — Overload Salvo
Range 1–4 / Cross1 / Damage 10 / Hit +10 / Crit 10% / EN 0 / no push / no counter
```

## Mission 6 — Break the Dreadnought

9×9 board:

```text
Players
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking
(2,4) (6,4)
(2,5) (6,5)

No hazards or explosives

Enemies / opening
Dreadnought 61 start (4,1) -> (4,2), target Vanguard
Bulwark     62 start (0,7) -> (1,7), target Vanguard
Controller  63 start (8,7) -> (6,7), target Vanguard
Rifleman    64 start (8,6) -> (6,6), target Interceptor
```

Reuse `assert_opening_plan_is_legal`; Mission 6 tests also pin every exact row.

### Opening manipulation line

Dreadnought commits Graviton centered on Vanguard `(4,7)`; Cross1 contains `(5,7)`.

1. Vanguard `(4,7) -> (4,5)`.
2. Interceptor `(5,8) -> (7,7)`.
3. Vector Pulse Controller `(6,7) -> (5,7)`.
4. Resolve locked intents.

The boss can then roll against Controller at `(5,7)` without retargeting; Controller's committed `(4,7)` push resolves into the vacated cell.

```text
Controller HP 9 / Armor 1
Vector Pulse normal damage at weapon level 0: 3
Graviton normal damage against Controller: 7
```

The hit is RNG-driven. Automated coverage uses the exact public movement/push line and a deterministic seed sweep to prove redirected Graviton enemy-weapon damage emits the existing `OptionalObjectiveCompleted` event for Turnabout. No friendly-fire special case.

### Objective/reward/story

```text
Primary: EliminateTarget { target: DREADNOUGHT }
Copy: Destroy the Dreadnought; escorts may be ignored.

Bonus: Turnabout
Copy: Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.

Base reward: 800
Bonus reward: 250
Unlocks: Mission 7 handoff
```

Do not add a turn limit. Tune authored values first if pacing is poor.

Reuse existing VN assets only.

Pre-mission:
1. Control: “A Dreadnought is anchoring the line. Its main battery commits before we move.”
2. Vanguard: “Then the escorts are ammunition.”
3. Control: “Exactly. Below half integrity the battery overloads and the Dreadnought will close in.”

Aftermath:
1. Vanguard: “Dreadnought down. Their line is collapsing.”
2. Control: “One command unit remains. Mission 7 is the final push.”

## Campaign handoff

Extend `MissionId` to Seven:

```text
One–Six -> authored definitions
Seven   -> terminal handoff
```

`MISSION_SIX_DEFINITION.unlocks = MissionId::Seven`. Existing completion logic remains unchanged.

Replace the hardcoded Continue list with:

```rust
Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
Ok(id) => next_state.set(if mission_definition(id).is_some() {
    GameScreen::Upgrade
} else {
    GameScreen::NextMission
}),
```

This reuses the same authored-data seam as `Proceed`. Add no new routing helper/table.

When Six becomes authored, existing library tests in Missions 2–5 that pin `mission_definition(Six).is_none()` move to Seven in that same task, so `cargo test --lib` stays green.

Campaign integration coverage then updates:

- `tests/campaign_model.rs`: Six authored, Seven None, base rewards through Six = 3300.
- `tests/campaign_flow.rs`: Continue Six -> Upgrade, Continue Seven -> NextMission, Proceed Six -> PreMissionStory, Proceed Seven -> NextMission, Mission 6 completion through `complete_current_mission`.
- `tests/campaign_persistence.rs`: Seven save round-trip preserves upgrades/credits.

No `GameScreen`, save-field, or migration changes.

## Presentation

No boss-only HUD or phase banner. Existing intent UI shows weapon/footprint/target/damage/hit; target-objective HUD shows Dreadnought HP.

Append one scene:

```text
Scene 13: Dreadnought
Root 70
Parts 71–76
Mesh/material 13
Scale 1.12
Material Dreadnought Crimson
Base color [0.55, 0.08, 0.12, 1.0]
```

Final counts: **14 scenes / 77 nodes / 14 meshes / 14 materials / 1 buffer**.

Existing Flanker and Bulwark/Controller tests pin old global counts and must move to final counts. Bound Controller's current `nodes.iter().enumerate().skip(64)` loop to `.skip(64).take(6)` before appending nodes 70–76.

Map `Dreadnought -> 13`; no second glTF, texture, animation, generator, under-ring, or inverse-scale compensation.

## Testing contract

Automated coverage must prove:

1. Graviton at 21 HP, Overload at 20 HP, no oscillation after further damage.
2. Committed Graviton survives threshold crossing; future intent uses Overload.
3. Overload close-pressure movement from range 5 -> 4.
4. Dreadnought remains normally pushable.
5. Mission 6 board/roster/opening/objective/rewards/stats/weapons/opening legality.
6. Public redirection line puts Controller on `(5,7)`, vacates `(4,7)`, and redirected boss damage can complete Turnabout.
7. Dreadnought KO wins with escorts alive.
8. Mission-local terminal pins move Six -> Seven when Six is registered.
9. Campaign model/flow/persistence blast radius above is updated.
10. glTF old/new tests agree on final counts and Controller loop is bounded.
11. Existing Missions 1–5 and all campaign/save/presentation tests remain green.

## Manual validation

Record in `docs/validation/hpa-524.md`:

- start Mission 6 through real campaign flow;
- confirm Graviton readability and escort redirection;
- cross 21+ -> <=20 after commitment and confirm current telegraph stays Graviton;
- confirm next planning shows Overload and closes from range 5 into range 4;
- confirm normal boss push;
- defeat Dreadnought with an escort alive;
- complete aftermath/reward/upgrade, return to title, Continue, confirm persisted Mission 7 handoff;
- record encounter length and tune authored values only if necessary.

## Scope guardrails

No multi-tile boss, parts, invulnerability, cinematic battle scene, unique boss runtime, threshold registry, phase scripting, generic behavior policies, displacement immunity, new status effect, new objective/optional shape, new hazard/prop type, new progression track, new save field, save migration, dependency/crate, second boss, Mission 7 content, new VN art, second glTF, or asset pipeline.