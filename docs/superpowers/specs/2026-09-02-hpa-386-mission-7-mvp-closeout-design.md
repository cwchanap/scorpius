# HPA-386 Mission 7 and MVP Closeout Design

## Outcome

Complete the seven-mission Scorpius MVP with one final authored battle, the second and final boss, a stable persisted campaign-complete state, a simple ending, and one evidence-driven presentation/tuning pass.

Keep this as one HPA-386 PR. Mission 7 should synthesize the six missions already shipped: read locked telegraphs, vacate threatened cells, redirect committed attacks onto enemies or props, use displacement/environment positioning and reactions, and decide whether optional time pressure is worth pursuing.

Prefer authored values and small extensions of existing seams over new systems. Do not add a boss engine, phase scripting, multi-tile occupancy, destructible boss parts, invulnerability phases, new objective/status frameworks, a cinematic battle scene, an audio subsystem, analytics, save migration, New Game+, or another crate/dependency.

## Existing seams to reuse

- `MissionId::Seven` already exists as the current terminal handoff; Missions One-Six are authored.
- `MissionDefinition` already owns a mission builder, objectives, rewards, pre-mission dialogue, and aftermath dialogue.
- `CampaignState` persists `next_mission`, credits, and upgrades through one `serde_json` save.
- `complete_current_mission` persists victory before entering Aftermath.
- Dreadnought is a normal one-cell `UnitState`; `unit_weapon` selects slot 1 at/below half HP while a committed `AttackIntent` remains immutable.
- All six regular enemy factories already exist in `mission/enemies.rs`.
- Existing objective vocabulary already includes `EliminateTarget`, `VictoryByRound`, and `Turnabout`.
- Existing board vocabulary already includes blocking cells, hazards, explosives, collision, and one-cell push.
- `BattleState::is_open_for` treats live explosives as occupied terrain; pushes into a live explosive become collision instead of movement.
- Existing combat playback owns one event lifecycle: start current event, animate while its timer runs, then clean up transient feedback in the finished branch.
- `EventEffect` is the current 3D impact-mesh component and `animate_effects` mutates its `Transform`.
- The battle HUD already uses screen-space Bevy UI `Text` under `HudRoot`; campaign screens are the places that spawn `Camera2d`.
- `grid_to_world` uses the current 9x9 convention (`HALF = 4.0`).
- There is no audio path.
- `assets/models/mission_one.gltf` remains the one combat visual asset.

## Selected approach

### Final boss behavior: extend only the second concrete consumer

Add `UnitArchetype::Regent` as a normal single-cell enemy. Mission 7 owns the Regent unit/weapons locally; Regent does not join the six-archetype regular roster in `mission/enemies.rs`.

Extend the existing selector:

```rust
let index = match unit.archetype {
    UnitArchetype::Dreadnought | UnitArchetype::Regent
        if unit.hp * 2 <= unit.stats.max_hp => 1,
    _ => 0,
};
```

No phase field, threshold registry, event, callback, or boss runtime is added.

Locked-intent behavior stays unchanged:

```text
Round N planning: Regent above half HP -> Command Barrage committed
Player phase: Regent crosses to half HP or below
Round N resolution: committed Command Barrage stays unchanged
Round N+1 planning: Rupture Beam is selected
```

### Regent values

```text
Regent
HP 52 / Armor 4 / Move 2 / Accuracy 92 / Evasion 8 / EN 0 / Initiative 45

Weapon 209 - Command Barrage
Range 3-6 / Cross1 / Damage 9 / Hit +10 / Crit 5% / EN 0 / no push / no counter

Weapon 210 - Rupture Beam
Range 2-4 / Single / Damage 12 / Hit +15 / Crit 10% / EN 0 / no push / no counter
```

At max HP 52:

```text
HP 27-52 -> Command Barrage
HP 0-26  -> Rupture Beam
```

Both weapons stay on existing `attack_band_destination`. Regent initiative 45 resolves ahead of Dreadnought 40 and Controller 35. Regent remains normally pushable.

## Mission 7 - Last Command

### 9x9 board

Keep the existing 9x9 board/camera convention.

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

The Controller push landing and explosive are deliberately different cells in the same Regent `Cross1` footprint:

```text
Regent barrage center: (4,7)
Cross1: (4,7) (4,6) (4,8) (3,7) (5,7)
Explosive: (3,7)
Controller landing: (5,7), empty before the push
```

Do not change `is_open_for` or `resolve_push` to allow a unit to occupy a live explosive.

### Enemy roster/opening

```text
Regent      71 start (4,1) -> (4,2), target Vanguard
Artillery   72 start (2,1) -> (2,2), target Gunner
Controller  73 start (8,7) -> (6,7), target Vanguard
Bulwark     74 start (1,6) -> (2,6), target Vanguard
Flanker     75 start (0,8) -> (1,8), target Gunner
```

Every row must pass the existing `assert_opening_plan_is_legal` helper. The opening is legal against the live weapon bands: Artillery at `(2,2)` reaches Gunner `(3,8)` at Manhattan 7 with Siege Mortar range 3-8; Controller at `(6,7)` reaches Vanguard `(4,7)` at aligned range 2 with Impulse Projector range 2-4.

### Public-path centerpiece

Mirror Mission 6's existing `redirected_opening_ready_to_resolve` test helper rather than inventing another RNG sequence. Mission 7 copies the same activation order and Vector Pulse path, adding only the Gunner move required to clear the explosion footprint.

With deterministic seed 2:

1. `begin_round()` commits Regent `Command Barrage` centered on Vanguard `(4,7)`.
2. Vanguard moves `(4,7) -> (4,5)`, chooses Guard, and finishes.
3. Gunner moves `(3,8) -> (2,8)`, chooses Guard, and finishes. This clears the original Gunner cell from the explosive at `(3,7)`; `resolve_explosion` would include `(3,8)` but not `(2,8)`.
4. Interceptor moves `(5,8) -> (7,7)`.
5. Interceptor uses the real `BattleState::attack` with Vector Pulse against Controller `(6,7)`. The seed-2 player rolls remain the Mission 6 sequence: hit roll 11, non-critical roll 27, normal damage, then legal push `(6,7) -> (5,7)`.
6. Interceptor chooses Guard and finishes.
7. Normal `resolve_enemy_phase()` runs. Explosive damage consumes no RNG. Regent's Controller attack therefore keeps the existing next boss rolls: hit 52, critical roll 37, non-critical.
8. Command Barrage damages the displaced Controller at `(5,7)` and separately damages/triggers the explosive at `(3,7)`.
9. Controller is knocked out and its lower-initiative committed intent is canceled.

The regression must assert:

- `UnitPushed { unit: CONTROLLER, to: (5,7) }`.
- Regent's committed weapon/profile/footprint remain unchanged after player movement.
- `AttackRolled` is Regent + Command Barrage + Controller + roll 52 + non-critical.
- `ExplosiveDamaged { position: (3,7), .. }` exists.
- `ExplosionTriggered { position: (3,7), .. }` exists.
- `IntentCanceled { attacker: CONTROLLER }` exists.
- No seed sweep, direct `resolve_push`, or occupancy exception is introduced.

### Objectives/rewards/story

```text
Primary: EliminateTarget { target: REGENT }
Copy: Destroy the Regent and break the command net.

Optional: VictoryByRound { round: 6 }
Copy: Final Push: destroy the Regent by the end of Round 6.

Base reward: 1000 credits
Optional reward: 300 credits
```

No separate hard-failure turn limit. Tune HP/placement/enemy count before adding another rule if pacing is poor.

Reuse current VN art.

Pre-mission:

1. Control: "The last command node is ahead. The Regent is broadcasting firing solutions to everything still standing."
2. Vanguard: "Then we make its final order point the wrong way."
3. Control: "Break the Regent. Once the command net drops, Relay Nine is ours."

Aftermath:

1. Vanguard: "Regent down. The remaining signatures are scattering."
2. Control: "Relay Nine is secure. Bring everyone home."
3. Vanguard: "Copy. Mission complete."

## Campaign completion

### Terminal mission model

Change:

```rust
pub unlocks: MissionId,
```

to:

```rust
pub unlocks: Option<MissionId>,
```

Missions One-Six use `Some(next)`. Mission Seven uses `None`.

Add exactly one persisted field:

```rust
pub completed: bool,
```

`new_game()` sets `completed: false`. Do not add `#[serde(default)]`, save versioning, converters, or migration.

`complete_mission` checks `completed` before reward mutation. Mission Seven leaves `next_mission == Seven`, sets `completed = true`, and can never award final rewards twice.

### Central terminal routing guard

Once Seven is authored, `mission_definition(Seven).is_some()` can no longer distinguish an unfinished final mission from a completed save. Centralize this in one small pure helper keyed by the existing action enum rather than a new routing abstraction:

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

Use it from Continue, the final line of Aftermath, and Proceed. `Proceed` must never restart Mission 7 when `completed == true`, even if a completed state somehow reaches Upgrade through a test fixture or later UI change.

Rename `GameScreen::NextMission` to `Ending`.

```text
New Game -> Mission 1 story
Continue completed -> Ending
Continue Mission 1 -> Mission 1 story
Continue unfinished Missions 2-7 -> Upgrade
Aftermath unfinished -> Upgrade
Aftermath completed -> Ending
Proceed unfinished -> current mission story
Proceed completed -> Ending
Ending -> Return to Title
```

Mission 7 skips a post-final upgrade screen. Final reward remains persisted and is shown in aftermath/ending totals.

## Board-first presentation finish

Keep current selection/reachable cells, telegraphs, intent guides, movement/push animation, 3D `EventEffect` impact meshes, damage shake, and KO shrink.

### Attack motion

On `AttackRolled`, briefly emphasize the attacker's existing `UnitVisual` using the current event timer. No projectile registry or animation system is added.

### Damage numbers: parallel feedback inside EventPlayback

Do not add `Text2d` or `Camera2d`, and do not tag a UI damage number as `EventEffect`.

`EventEffect` remains 3D-only because `animate_effects` mutates its `Transform`. Add a sibling `DamageNumberEffect { origin: Vec2 }` component/query inside `play_battle_events`.

Lifecycle:

1. When a `DamageApplied` event starts, keep the existing `spawn_event_effect` call unchanged so the 3D impact still appears.
2. Resolve the target's current grid cell.
3. Call `Camera::world_to_viewport` using the existing battle `Camera3d` and `GlobalTransform`.
4. If projection returns `Ok(viewport)`, spawn one UI `Text` child under `HudRoot`; if projection returns `Err`, skip only the number. `play_battle_events` does not become a `Result` function.
5. Reuse the existing presentation `text_font` helper by making it `pub(crate)`; do not duplicate a new font-construction path.
6. During the current event, animate only `DamageNumberEffect`'s UI `Node.top` from `origin.y` to `origin.y - 24.0`.
7. In the same `finished` branch that currently despawns all 3D `EventEffect` entities, separately despawn all `DamageNumberEffect` entities, then clear `playback.current`.

Tests must cover the actual lifecycle: number entity appears for `DamageApplied`, moves upward, and is gone after the shared finished cleanup. A string-format-only test is insufficient.

### Boss camera emphasis

Tag the existing battle `Camera3d` with `BattleCamera { rest: Transform }`. During Dreadnought/Regent `AttackRolled`, derive a small deterministic sinusoidal offset from `rest`. Restore exactly to `rest` after the event and for all non-boss events. No second camera/controller/cut-in/zoom timeline.

### Audio

Skip sound cues because the repo has no audio seam.

## Regent visual

Append one final scene to the same glTF:

```text
Scene 14: Regent
Root node 77
Part nodes 78-83
Mesh/material 14
Root scale 1.20
Material: Regent Violet
Base color: [0.42, 0.14, 0.78, 1.0]
```

Final counts are one coordinated pin:

```text
15 scenes
84 nodes
15 meshes
15 materials
1 buffer
```

Update every existing test in `src/presentation/assets.rs` that hard-codes old global counts in the same commit as the glTF append:

- `flanker_scene_is_authored_with_own_mesh_material_and_root_scale`
- `bulwark_and_controller_scenes_are_authored_with_own_meshes_and_roots`
- `dreadnought_scene_is_authored_as_a_larger_crimson_unit`
- the new Regent scene test

Do not refactor these into a registry/helper merely to deduplicate four assertions.

## Whole-campaign tuning and validation

Play a clean New Game through Ending and record one row per mission:

```text
mission | wall-clock minutes | rounds | restarts | optional complete? | committed-intent manipulation materially rewarded? | notes/tuning
```

Also record credits, purchases, boss-threshold timing, telegraph readability, and presentation observations.

Acceptance:

- first playthrough roughly 120-180 minutes, or document a small justified deviation;
- at least 4 of 7 missions materially reward reading/manipulating committed intent;
- base rewards before Mission 7 remain 3300;
- a base-only path can buy one chosen track to level 2 on each mech: `(200 + 400) * 3 = 1800`, so optional rewards are acceleration rather than a prerequisite;
- final Dreadnought 21/20 and Regent 27/26 threshold regressions pass;
- the Mission 7 seed-2 public-path regression passes without occupancy-rule changes;
- completed Continue/Aftermath/Proceed all route to Ending;
- damage numbers share EventPlayback cleanup and never accumulate.

Tune only when the playtest records a concrete problem. Prefer placement/opening geometry, enemy count, boss HP/threshold timing, authored round pressure, weapon values, then progression values. Add focused mechanical regressions for every tuned value.

## Scope guardrails

No new playable mech, seventh regular enemy, boss engine, phase/threshold registry, objective/status framework, AI policy layer, occupancy exception, new hazard/prop type, battle-animation scene, `Camera2d`/`Text2d` battle pipeline, audio subsystem, analytics/tuning framework, save migration, New Game+, dependency/crate, second glTF, asset pipeline, or second PR.