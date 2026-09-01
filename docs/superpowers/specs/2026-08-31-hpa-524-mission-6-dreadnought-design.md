# HPA-524 Mission 6 Dreadnought Design

## Outcome

Ship Mission 6 as the first boss encounter while keeping the boss on the existing single-cell enemy/combat path. The fight should feel different because the Dreadnought commits large area attacks and switches to a stronger close-pressure salvo at half HP, not because Scorpius gains a boss engine.

Keep this as one HPA-524 PR. Reuse the deterministic battle state, locked enemy intents, `EliminateTarget`, `Turnabout`, authored mission openings, campaign save/progression flow, Bevy UI, and checked-in glTF. Do not add phase scripting, a boss component hierarchy, generic threshold registry, new objective shape, status framework, second battle runtime, or save migration.

## Existing seams

- Every enemy is a normal `UnitState` on one grid cell.
- Enemy movement already calls `unit_weapon`; `build_intent` must reuse the same selector instead of reading `.first()` independently.
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

No phase state is stored. The same selected weapon drives future intent commitment, opening validation, and ordinary attack-band movement. Mission 7 is the second concrete consumer and is the earliest point to consider sharing threshold structure.

Rejected: generic boss/phase data, boss runtime/controller, scripts, detachable parts, invulnerability, multi-tile occupancy, callbacks, resistance framework.

### Locked threshold contract

```text
Round N planning: 21 HP -> Graviton committed
Player phase: boss drops to 20 HP
Round N resolution: committed Graviton remains unchanged
Round N+1 planning: Overload selected
```

No phase-change event or boss banner. The next normal telegraph exposes the new weapon/range/damage.

### Movement and initiative

Dreadnought initiative is **40**, above Controller 35. That ordering is load-bearing for Mission 6's redirection line: the boss must damage the displaced Controller before the Controller resolves its own now-vacated committed target.

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
Range 2–4 / Cross1 / Damage 10 / Hit +10 / Crit 10% / EN 0 / no push / no counter
```

### Why Overload starts at range 2

`Cross1` includes the target cell and its four orthogonal neighbors. At Manhattan range 1, the attacker is itself an orthogonal neighbor of the target, so a range-1 Cross1 can include its own attacker. Enemy intent resolution attacks every occupant in the footprint and intentionally allows enemy-on-enemy damage; a range-1 Overload would therefore let the Dreadnought damage or KO itself and complete `Turnabout` for free.

Keep this an authored-value fix, not a new combat exception. **Range 2–4** preserves the intended close-pressure contrast with Graviton 3–6 while keeping the footprint radius strictly below the minimum range. A regression test pins that a committed Overload footprint never contains the Dreadnought's cell.

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

Reuse `assert_opening_plan_is_legal`; Mission 6 tests also pin every exact row. Make `unit_weapon` `pub(crate)` and let the shared opening validator use it, so movement, authored-opening validation, and intent construction all agree on the selected enemy weapon.

### Opening manipulation line

Dreadnought commits Graviton centered on Vanguard `(4,7)`; Cross1 contains `(5,7)`.

1. Vanguard `(4,7) -> (4,5)`.
2. Interceptor `(5,8) -> (7,7)`.
3. Vector Pulse geometry pushes Controller `(6,7) -> (5,7)`.
4. Finish all three player activations and call normal `resolve_enemy_phase()`.

With deterministic seed **1**, the Dreadnought's first roll is 66 (hit at 85%) and the critical roll is 20 (not critical at 5%). Graviton therefore deals normal 7 damage to Controller, leaving it alive at 2 HP; `OptionalObjectiveCompleted` is emitted through the existing `Turnabout` observer. Controller then resolves second at initiative 35 and its committed `(4,7)` target is empty, producing `AttackHitEmpty` instead of being canceled by a boss critical.

This pins the centerpiece using the real enemy-phase ordering rather than a private intent resolver or a seed sweep. If RNG call order changes, this test should fail visibly rather than silently finding another seed.

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

Mission 6 registration and the full old-terminal blast radius land in the same implementation task/commit so every task can remain green under `cargo test --all-targets`:

- Missions 2–5: terminal-definition pins move from Six to Seven.
- `tests/campaign_model.rs`: Six authored, Seven None, base rewards through Six = 3300.
- `tests/campaign_flow.rs`: Continue Six -> Upgrade, Continue Seven -> NextMission, Proceed Six -> PreMissionStory, Proceed Seven -> NextMission, Mission 6 completion through `complete_current_mission`.
- `tests/campaign_persistence.rs`: replace the old `mission_definition(Six).is_none()` assertion and round-trip Seven with upgrades/credits intact.

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

## Risks

- **Cross1 self-overlap:** Overload is the first proposed area weapon whose minimum range could overlap its own footprint radius. Lock it to 2–4 and test that the attacker cell is absent.
- **Resolution order:** the authored redirection line requires Dreadnought 40 > Controller 35. Pin both initiatives and the resulting event order.
- **Mission registration blast radius:** making Six authored invalidates old terminal assertions across mission, campaign-flow, campaign-model, and campaign-persistence tests. Update them in the same task as registration.
- **Asset append blast radius:** existing glTF tests pin global counts and an unbounded Controller loop would inspect the new nodes. Repair those assertions before appending the new scene.

## Testing contract

Automated coverage must prove:

1. Graviton at 21 HP, Overload at 20 HP, no oscillation after further damage.
2. Committed Graviton survives threshold crossing; future intent uses Overload.
3. Overload is exactly range 2–4, closes from range 5 -> 4, and no committed Overload footprint contains its attacker.
4. Initiative table includes Dreadnought 40 and explicitly proves it is above Controller 35.
5. Dreadnought remains normally pushable.
6. Mission 6 board/roster/opening/objective/rewards/stats/weapons/opening legality.
7. Public redirection line resolves through `resolve_enemy_phase()` at seed 1, damages Controller with Graviton, completes Turnabout, then resolves Controller into empty `(4,7)`.
8. Dreadnought KO wins with escorts alive.
9. All old Six-terminal pins move to Seven in the same task that registers Mission 6.
10. glTF old/new tests agree on final counts and Controller loop is bounded.
11. Every implementation task runs format, strict Clippy, and its focused/full relevant tests; final closeout runs the full CI gate set.

## Manual validation

Record in `docs/validation/hpa-524.md`:

- start Mission 6 through real campaign flow;
- confirm Graviton readability and escort redirection;
- cross 21+ -> <=20 after commitment and confirm current telegraph stays Graviton;
- confirm next planning shows Overload range 2–4 and closes from range 5 into range 4;
- confirm normal boss push;
- defeat Dreadnought with an escort alive;
- complete aftermath/reward/upgrade, return to title, Continue, confirm persisted Mission 7 handoff;
- record encounter length and tune authored values only if necessary.

## Scope guardrails

No multi-tile boss, parts, invulnerability, cinematic battle scene, unique boss runtime, threshold registry, phase scripting, generic behavior policies, displacement immunity, new status effect, new objective/optional shape, new hazard/prop type, new progression track, new save field, save migration, dependency/crate, second boss, Mission 7 content, new VN art, second glTF, or asset pipeline.