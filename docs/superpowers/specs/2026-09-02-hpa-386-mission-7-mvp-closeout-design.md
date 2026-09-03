# HPA-386 Mission 7 and MVP Closeout Design

## Outcome

Complete the seven-mission Scorpius MVP with one final authored battle, the second and final boss, a stable persisted campaign-complete state, a simple ending, and one focused presentation/tuning pass.

Keep this as one HPA-386 PR. Mission 7 should synthesize the six missions already shipped: read locked telegraphs, vacate threatened cells, redirect committed attacks onto enemies or props, use displacement/environment positioning and reactions, and decide whether optional time pressure is worth pursuing.

Prefer authored values and small extensions of existing seams over new systems. Do not add a boss engine, phase scripting, multi-tile occupancy, destructible boss parts, invulnerability phases, new objective/status frameworks, a cinematic battle scene, an audio subsystem, analytics, save migration, New Game+, or another crate/dependency.

## Existing seams

The current code already provides nearly everything HPA-386 needs:

- `MissionId::Seven` exists as the current terminal handoff; Missions One–Six are authored definitions.
- `MissionDefinition` owns a mission's builder, objectives, rewards, pre-mission dialogue, and aftermath dialogue.
- `CampaignState` persists `next_mission`, credits, and upgrades through the existing `serde_json` save path.
- `CampaignState::complete_mission` and `complete_current_mission` already own save-backed mission completion.
- Mission 6 introduced Dreadnought as a normal one-cell `UnitState`; `unit_weapon` selects its future weapon slot by HP while committed `AttackIntent` values remain immutable.
- All six regular enemy factories already exist in `mission/enemies.rs`.
- Existing primary/optional objective variants already cover `EliminateTarget`, `VictoryByRound`, and `Turnabout`.
- Existing board vocabulary already includes blocking cells, hazards, explosives, collision, and one-cell push.
- `BattleState::is_open_for` deliberately treats live explosives as occupied terrain; pushes into them become collision instead of movement. Mission 7 must author around that rule rather than weaken it.
- Battlefield presentation already renders selected/reachable/attack-preview cells, intent footprints/edges/target guides, reaction markers, movement/push motion, impact meshes, hit/damage shake, and knockout shrink.
- Battle HUD copy already uses Bevy UI `Text` under the screen-space `HudRoot`; combat playback does not use `Text2d` or a `Camera2d`.
- `grid_to_world` is centered for the existing 9×9 board convention (`HALF = 4.0`). Mission 7 can fit on 9×9, so there is no reason to add board-size camera math for closeout.
- There is no audio path today. HPA-386 therefore skips sound rather than adding a subsystem for optional polish.
- The checked-in `assets/models/mission_one.gltf` remains the single combat visual asset.

## Selected approach

### Boss behavior: share only the second concrete seam

There are three plausible choices for the final boss threshold behavior:

1. Add a generic boss phase/threshold data model — rejected. Two bosses with one half-HP switch do not justify a registry, callbacks, or scripting.
2. Copy Dreadnought's threshold selector into Mission 7 — rejected. That would duplicate the same rule across movement, opening validation, and intent construction.
3. Extend the existing closed `unit_weapon` match to Dreadnought + Regent — selected. Mission 7 is the second concrete consumer and this is the smallest justified shared seam.

The same principle applies to campaign completion: represent terminal state directly rather than keeping Seven as an unauthored sentinel or inventing Mission Eight.

## Final boss — Regent

Add one final `UnitArchetype::Regent`. It remains a normal single-cell enemy using ordinary movement, targeting, damage, reactions, collision, displacement, and locked intent resolution.

Mission 7 owns the Regent factory and weapons locally; Regent does not join the six-archetype regular roster in `mission/enemies.rs`.

### Shared half-HP selector

Extend `unit_weapon` only as far as the two concrete bosses require:

```rust
let index = match unit.archetype {
    UnitArchetype::Dreadnought | UnitArchetype::Regent
        if unit.hp * 2 <= unit.stats.max_hp => 1,
    _ => 0,
};
```

No phase field is stored in `UnitState`. No threshold event is added. Both bosses use slot 0 above half HP and slot 1 at/below half HP.

Locked-intent behavior remains unchanged:

```text
Round N planning: Regent above half HP -> Command Barrage committed
Player phase: Regent crosses to half HP or below
Round N resolution: committed Command Barrage stays unchanged
Round N+1 planning: Rupture Beam is selected
```

The next normal telegraph communicates the behavior change.

### Regent values

```text
Regent
HP 52 / Armor 4 / Move 2 / Accuracy 92 / Evasion 8 / EN 0 / Initiative 45

Weapon 209 — Command Barrage
Range 3–6 / Cross1 / Damage 9 / Hit +10 / Crit 5% / EN 0 / no push / no counter

Weapon 210 — Rupture Beam
Range 2–4 / Single / Damage 12 / Hit +15 / Crit 10% / EN 0 / no push / no counter
```

At max HP 52:

```text
HP 27–52 -> Command Barrage
HP 0–26  -> Rupture Beam
```

The contrast is authored rather than systemic: the first weapon threatens a broad committed footprint, while the second closes its range band and concentrates more damage on one cell. Existing `attack_band_destination` remains sufficient.

Regent initiative is **45**, above Dreadnought 40 and Controller 35. The Regent remains normally pushable; do not add mass/resistance.

## Mission 7 — Last Command

### Board and deployment

Use a **9×9** board. The final encounter does not need a tenth row, and keeping 9×9 preserves the current centered `grid_to_world`/camera composition without another presentation rule.

```text
Players
Vanguard    (4,7)
Gunner      (3,8)
Interceptor (5,8)

Blocking
(2,4) (6,4)
(2,5) (6,5)

Hazards
(3,5) (5,5)

Explosive
(3,7), HP 4
```

The explosive and the Controller's intended push landing are deliberately **different** cells. Both are in the Regent's opening `Cross1`, but `(5,7)` stays empty so Vector Pulse can legally push into it under the existing `is_open_for` rule.

Do not change `is_open_for` or `resolve_push` to let units overlap live explosives.

### Enemy roster and opening

Use Regent plus four existing regular archetypes. Do not put all six regular types on the board merely to showcase the roster.

```text
Regent      71 start (4,1) -> (4,2), target Vanguard
Artillery   72 start (2,1) -> (2,2), target Gunner
Controller  73 start (8,7) -> (6,7), target Vanguard
Bulwark     74 start (1,6) -> (2,6), target Vanguard
Flanker     75 start (0,8) -> (1,8), target Gunner
```

All opening rows must pass the existing shared `assert_opening_plan_is_legal` helper.

The opening layers earlier lessons without inventing a new mechanic:

- Regent commits `Command Barrage` centered on Vanguard `(4,7)`. Its `Cross1` footprint is `(4,7) (4,6) (4,8) (3,7) (5,7)`.
- `(3,7)` contains the live explosive.
- `(5,7)` is empty and is the legal Controller push landing.
- Controller creates a displacement threat in the lower-right lane.
- Artillery reinforces long-range locked-footprint reading.
- Bulwark occupies space through body blocking only.
- Flanker adds side pressure through its existing behavior.

### Authored opening manipulation line

Pin one public-path regression around the centerpiece using the same live action/resolution seams as Mission 6.

With deterministic seed **2**:

1. `begin_round()` applies the authored opening and commits Regent `Command Barrage` on Vanguard `(4,7)`.
2. Vanguard moves `(4,7) -> (4,5)` and finishes with a reaction.
3. Gunner moves `(3,8) -> (2,8)` so the explosive at `(3,7)` can detonate without clipping the original Gunner cell, then finishes with a reaction.
4. Interceptor moves `(5,8) -> (7,7)`.
5. Interceptor uses the real Vector Pulse `attack` on Controller `(6,7)`. Seed 2's first player rolls remain the existing deterministic sequence; the attack deals its normal player damage and legally pushes Controller `(6,7) -> (5,7)`.
6. Finish Interceptor with a reaction and call normal `resolve_enemy_phase()`.
7. Regent resolves first at initiative 45. Its still-committed `Command Barrage` damages Controller at `(5,7)` and separately damages the explosive at `(3,7)` enough to trigger the existing explosion path.
8. If the Regent hit knocks out Controller after Vector Pulse damage, Controller's lower-initiative committed intent is canceled through the existing knockout behavior.

Do not use a seed sweep. Do not bypass `BattleState::attack`, `resolve_push`, or `resolve_enemy_phase()` with test-only mutation shortcuts.

The regression pins tactical facts rather than incidental total event counts:

- `UnitPushed { unit: CONTROLLER, to: (5,7) }` is emitted; `(5,7)` is not a prop cell.
- Regent's committed weapon/profile/footprint are unchanged by player movement.
- Regent damages the displaced Controller before Controller's own intent can resolve.
- `ExplosiveDamaged` and `ExplosionTriggered` name explosive cell `(3,7)`, not Controller cell `(5,7)`.
- No retargeting occurs after Vanguard leaves `(4,7)`.
- The test never requires changing occupancy semantics.

### Objectives and rewards

```text
Primary: EliminateTarget { target: REGENT }
Copy: Destroy the Regent and break the command net.

Optional: VictoryByRound { round: 6 }
Copy: Final Push: destroy the Regent by the end of Round 6.

Base reward: 1000 credits
Optional reward: 300 credits
```

There is no separate hard-failure turn limit. If pacing is poor, tune HP/placement/enemy count before adding another rule.

### Story

Reuse existing VN backgrounds and portraits only.

Pre-mission:

1. Control: “The last command node is ahead. The Regent is broadcasting firing solutions to everything still standing.”
2. Vanguard: “Then we make its final order point the wrong way.”
3. Control: “Break the Regent. Once the command net drops, Relay Nine is ours.”

Aftermath/ending lead-in:

1. Vanguard: “Regent down. The remaining signatures are scattering.”
2. Control: “Relay Nine is secure. Bring everyone home.”
3. Vanguard: “Copy. Mission complete.”

No narrative engine or ending art is added.

## Campaign completion

### Author Seven and make terminality explicit

Register `MissionId::Seven` in `mission_definition` and add `mission_seven.rs`.

Change:

```rust
pub unlocks: MissionId,
```

to:

```rust
pub unlocks: Option<MissionId>,
```

Missions One–Six use `Some(next)`. Mission Seven uses `None`.

### Persist completion once

Add exactly one field:

```rust
pub completed: bool,
```

`CampaignState::new_game()` sets it to `false`.

Do not add `#[serde(default)]`, a version field, converter, or migration. Existing pre-HPA-386 saves may stop loading; pre-release backward compatibility is explicitly out of scope.

`CampaignState::complete_mission` becomes:

```rust
if self.completed {
    return Err(CampaignError::CampaignComplete);
}
if !result.victory {
    return Err(CampaignError::MissionNotWon);
}
if self.next_mission != definition.id {
    return Err(CampaignError::AlreadyAdvanced { ... });
}

let optional_reward = if result.optional_complete {
    definition.optional_reward
} else {
    0
};
let total_reward = definition.base_reward + optional_reward;
self.credits += total_reward;

match definition.unlocks {
    Some(next) => self.next_mission = next,
    None => self.completed = true,
}
```

For Mission 7, `next_mission` remains Seven while `completed` becomes true. A second completion attempt fails before reward mutation, proving exactly-once terminal completion.

### Ending flow

Rename the old sentinel-oriented `GameScreen::NextMission` to `GameScreen::Ending`.

```text
Title
PreMissionStory
Briefing
Battle
Aftermath
Upgrade
Ending
```

Routing:

```text
New Game -> Mission 1 story
Continue, completed == true -> Ending
Continue, Mission 1 -> Mission 1 story
Continue, unfinished Missions 2–7 -> Upgrade
Aftermath, completed == false -> Upgrade
Aftermath, completed == true -> Ending
Proceed from Upgrade -> current mission story
Ending -> Return to Title
```

Mission 7 intentionally skips a post-final upgrade screen. Its reward is persisted and appears in aftermath/ending totals, but there is no future battle to upgrade for.

Ending copy:

```text
CAMPAIGN COMPLETE
Relay Nine secured.

Final credits: <credits>

Vanguard    <HP ARMOR MOBILITY WEAPON levels>
Gunner      <levels>
Interceptor <levels>

RETURN TO TITLE
```

A completed save keeps Continue enabled and reopens Ending instead of attempting another battle.

## Board-first presentation finish

Do not replace presentation that already works. Keep the current selection/reachable tiles, telegraph overlays, intent guides, movement/push motion, 3D impact mesh, target shake, and KO shrink.

Add only the concrete gaps below.

### Attack motion

On `BattleEvent::AttackRolled`, pulse the attacker's existing `UnitVisual` forward/scale briefly using the current playback timer. No projectile entity or weapon-animation registry is required.

### Damage numbers — reuse existing battle UI text

Do **not** add `Text2d` or a `Camera2d`.

Keep `spawn_event_effect` unchanged for the existing 3D `DamageApplied` impact. In addition, spawn one short-lived Bevy UI `Text` child under the existing `HudRoot`:

```text
-7
-12
```

Use the event's `amount`; do not calculate damage again.

Position the UI node by projecting the target's world position through the existing 3D battle camera:

```rust
let viewport = camera.world_to_viewport(
    camera_transform,
    grid_to_world(target.position) + Vec3::Y * 0.8,
)?;
```

Set an absolute UI `Node.left/top` from that viewport position, then move it slightly upward during the same event fraction and despawn it when the event finishes.

This reuses the existing camera + Bevy UI text stack, preserves the current 3D impact mesh, and adds no second rendering pipeline.

### Modest boss camera emphasis

Tag the existing `Camera3d` with a small `BattleCamera { rest: Transform }` component. During `AttackRolled` from Dreadnought or Regent, apply a deterministic low-amplitude sinusoidal offset from `rest`; restore exactly to `rest` whenever that event finishes or no boss attack is active.

Do not add a camera controller, cut-in, zoom timeline, or battle-animation scene.

### Pilot skills

The three pilot skills already receive command/status feedback and normal attack events. Do not build a signature-animation framework solely to specialize them.

### Audio

Skip sound cues. There is no existing audio seam.

## Regent visual

Append one final scene to the existing checked-in glTF:

```text
Scene 14: Regent
Root node 77
Part nodes 78–83
Mesh/material 14
Root scale 1.20
Material: Regent Violet
Base color: [0.42, 0.14, 0.78, 1.0]
```

Final counts:

```text
15 scenes
84 nodes
15 meshes
15 materials
1 buffer
```

Map `UnitArchetype::Regent -> 14` and raise `MISSION_ONE_SCENE_COUNT` to 15. No new texture, animation, glTF file, generator, or asset pipeline.

## Whole-campaign tuning and validation

HPA-386 is the intentional end-to-end playtest ticket. Start from a clean New Game save and play Missions 1–7 through Ending.

Record one row per mission in `docs/validation/hpa-386.md`:

```text
mission | wall-clock minutes | rounds | restarts | optional complete? | committed-intent manipulation materially rewarded? | notes/tuning
```

Also record:

- total first-playthrough time;
- credits before/after each mission;
- upgrades purchased and when;
- telegraph/readability problems;
- whether each boss threshold occurs at a useful encounter point.

Acceptance for the ledger:

- Total first-playthrough time is roughly 120–180 minutes, or the ledger documents a small justified deviation.
- At least **4 of 7** authored encounters materially reward reading/manipulating committed intent rather than only maximizing damage.
- The base-reward-only path has **3300 credits before Mission 7** and can buy one chosen track to level 2 on each mech: 600 per mech, 1800 total, without optional rewards.
- Optional rewards accelerate/customize progression but are not required.

### Tuning policy

Only tune after recorded evidence. Prefer this order:

1. placement/opening geometry;
2. enemy count;
3. boss HP/threshold timing;
4. authored round pressure;
5. weapon values;
6. upgrade/reward values.

Do not build analytics, difficulty modes, or a tuning data framework.

## Testing contract

Automated coverage must prove:

1. Dreadnought still switches exactly at 21/20 HP and Regent at 27/26 HP.
2. Crossing either threshold never rewrites an already-committed intent.
3. Regent initiative is 45 and exceeds Dreadnought 40 / Controller 35.
4. Regent stays on ordinary single-cell attack-band movement and remains pushable.
5. Mission 7 board/deployment/roster/opening/objective/reward/story values are pinned and `assert_opening_plan_is_legal` passes.
6. The seed-2 public centerpiece performs a real Vector Pulse push `(6,7) -> (5,7)`, then the committed Regent barrage damages Controller and triggers the separate explosive at `(3,7)` through normal enemy resolution.
7. No test or implementation changes `is_open_for` to permit live-explosive occupancy.
8. `MissionDefinition.unlocks` is `Some(next)` for One–Six and `None` for Seven.
9. Final completion sets `completed = true`, awards Mission 7 once, and a second attempt is an atomic `CampaignComplete` error.
10. Completed save round-trip + Continue route to Ending; unfinished Seven resumes through Upgrade -> story -> battle.
11. All direct `CampaignState` fixtures explicitly set `completed`; no compatibility default hides stale test data.
12. Regent is included explicitly in every exhaustive `UnitArchetype` branch touched by the feature, including enemy movement/initiative, `HudSnapshot::can_pilot`, `HudSnapshot::pilot_label`, `CommandAction::PilotSkill`, and scene selection.
13. Existing 3D impact effects remain for `DamageApplied`; a separate HUD `Text` damage number is spawned/projected and removed without `Text2d` or `Camera2d`.
14. Boss camera emphasis always restores the exact rest transform.
15. glTF tests pin final 15/84/15/15/1 counts and scene 14 Regent values.
16. Base-only reward math proves 3300 credits before Seven can fund three level-2 tracks.
17. Concrete regressions discovered during the final manual pass receive targeted automated tests.
18. Repository format/lint/test/coverage/release-build gates pass at closeout.

## Risks

- **Illegal push/prop overlap:** highest authored-risk. The Controller landing `(5,7)` must stay empty; explosive is `(3,7)`. Do not alter occupancy rules.
- **Mission registration/terminal blast radius:** Seven changes from unauthored sentinel to playable terminal mission while `unlocks` and `CampaignState` change shape. Land those together.
- **Exhaustive Regent matches:** compiler-visible `UnitArchetype` arms must be updated in the same first task, especially HUD pilot affordance and pilot command rejection.
- **Damage-number rendering:** use existing UI `Text` + 3D camera projection. Do not introduce `Text2d`/`Camera2d`, and do not replace the current impact mesh.
- **Camera restore:** shake must be computed from an immutable rest transform and restored deterministically.
- **Asset append:** existing glTF tests pin global counts; update all relevant pins when scene 14 is appended.
- **Tuning scope creep:** authored values only, backed by the recorded playtest.

## Scope guardrails

No new playable mech, seventh regular enemy, boss engine, generic threshold registry, phase scripting, parts, invulnerability, resistance, multi-tile occupancy, objective/status framework, AI policy framework, new hazard/prop type, battle-animation scene, cut-in framework, audio subsystem, analytics/tuning framework, save migration/versioning, New Game+, new progression track, dependency/crate, second glTF, asset pipeline, or second PR.