# HPA-386 Mission 7 and MVP Closeout Design

## Outcome

Complete the seven-mission Scorpius MVP with one final authored battle, the second and final boss, a stable persisted campaign-complete state, a simple ending, and a focused presentation/tuning pass.

Keep this as one HPA-386 PR. Mission 7 should feel like a synthesis of the six missions already shipped: read locked telegraphs, move out of threatened cells, redirect committed attacks onto enemies or props, use displacement and reactions, and decide whether the optional time pressure is worth pursuing. The closeout should prefer authored values and small presentation effects over new systems.

Do not add a boss engine, phase scripting, multi-tile occupancy, destructible boss parts, invulnerability phases, new objective/status frameworks, a cinematic battle scene, an audio subsystem, analytics, save migration, New Game+, or another crate/dependency.

## Existing seams

The current code already provides nearly everything HPA-386 needs:

- `MissionId::Seven` exists as the current terminal handoff; Missions One–Six are authored definitions.
- `MissionDefinition` owns a mission's builder, objectives, rewards, pre-mission dialogue, and aftermath dialogue.
- `CampaignState` persists `next_mission`, credits, and upgrades through the existing `serde_json` save path.
- `CampaignState::complete_mission` and `complete_current_mission` already enforce save-backed completion and reject replaying a mission after `next_mission` advances.
- Mission 6 introduced the Dreadnought as a normal one-cell `UnitState`. `unit_weapon` switches its future weapon selection at half HP while committed `AttackIntent` values remain immutable.
- All six regular enemy factories already exist in `mission/enemies.rs`; Mission 7 should compose those instead of adding a seventh regular archetype.
- Primary objectives already include `EliminateTarget`; optional objectives already include `VictoryByRound` and `Turnabout`.
- Existing board vocabulary already includes blocking cells, damaging hazards, explosives, collision, and one-cell push.
- Battlefield presentation already renders selected/reachable/attack-preview cells, intent footprints/edges/target guides, reaction markers, movement/push motion, hit/impact pulses, damage shake on units, and knockout shrink.
- There is no audio path today. HPA-386 therefore does not add sound merely to satisfy an optional polish bullet.
- The checked-in `assets/models/mission_one.gltf` contains all current unit/prop scenes and remains the single combat visual asset.

The closeout should extend those seams, not replace them.

## Selected approach

### Why this approach

There are three reasonable ways to implement the final boss threshold behavior:

1. **Add a generic boss phase/threshold data model.** Rejected. Two bosses with one half-HP switch do not justify a registry, callbacks, or scripting surface.
2. **Copy the Dreadnought selector into Mission 7-specific code.** Rejected. Mission 7 is now the second concrete consumer, and duplicating the same half-HP slot rule would make the existing selector disagree across movement, opening validation, and intent construction.
3. **Extend the existing closed `unit_weapon` match to both boss archetypes.** Selected. This is the smallest shared seam justified by the second consumer and keeps all future-intent selection on one path.

The same principle applies to campaign completion: represent completion directly rather than keeping `MissionId::Seven` as an unauthored sentinel or self-unlocking Mission 7.

## Final boss — Regent

Add one final `UnitArchetype::Regent`. It remains a normal single-cell enemy using ordinary movement, targeting, damage, reactions, collision, displacement, and locked intent resolution.

Mission 7 owns the Regent factory and weapons locally; it does not join the six-archetype regular roster in `mission/enemies.rs`.

### Shared half-HP selector

Extend `unit_weapon` only as far as the two concrete bosses require:

```rust
let index = match unit.archetype {
    UnitArchetype::Dreadnought | UnitArchetype::Regent
        if unit.hp * 2 <= unit.stats.max_hp => 1,
    _ => 0,
};
```

No phase field is stored in `UnitState`. No threshold event is added. Both bosses use weapon slot 0 above half HP and slot 1 at/below half HP.

The locked-intent contract remains unchanged:

```text
Round N planning: Regent above half HP -> Command Barrage committed
Player phase: Regent crosses to half HP or below
Round N resolution: committed Command Barrage is unchanged
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

The phase contrast is authored rather than systemic: the first weapon threatens a broad committed footprint, while the second closes the range band and concentrates more damage on one cell. `attack_band_destination` remains sufficient because neither Regent weapon pushes.

Regent initiative is **45**, above the Dreadnought's 40 and Controller's 35. This lets the final boss remain the first resolved threat when its telegraph is deliberately redirected onto an escort.

The Regent remains normally pushable. Do not add boss mass/resistance.

## Mission 7 — Last Command

### Board and deployment

Use a 10×10 board. It is slightly larger than prior encounters but remains within the locked compact-map range and gives the final mixed roster enough room without increasing system complexity.

```text
Players
Vanguard    (4,8)
Gunner      (3,9)
Interceptor (5,9)

Blocking
(2,4) (7,4)
(2,5) (7,5)

Hazards
(3,5) (5,5)

Explosive
(5,8), HP 4
```

The single explosive is deliberately placed inside the opening Regent footprint so the board itself can become part of the solution without cluttering the final telegraphs. The two hazards preserve the earlier environmental vocabulary for later-round positioning.

### Enemy roster and opening

Use the Regent plus four existing regular archetypes. Do not put all six regular types on the board simply to showcase the roster.

```text
Regent      71 start (4,0) -> (4,2), target Vanguard
Artillery   72 start (2,2) -> (2,3), target Gunner
Controller  73 start (8,8) -> (6,8), target Vanguard
Bulwark     74 start (1,7) -> (2,7), target Vanguard
Flanker     75 start (0,9) -> (1,9), target Gunner
```

This opening deliberately layers earlier lessons:

- Regent commits a `Cross1` Command Barrage centered on Vanguard `(4,8)`; the footprint includes `(5,8)` and therefore the explosive.
- Controller creates a displacement threat in the same lower-right lane.
- Artillery reinforces the idea that long-range footprints stay locked.
- Bulwark occupies space rather than gaining a new aura/guard mechanic.
- Flanker creates fast side pressure without a new policy branch.

### Authored opening manipulation line

Pin one public-path regression around the centerpiece, using the same normal player action and enemy resolution seams that shipped in Mission 6:

1. Regent has already committed Command Barrage centered on Vanguard `(4,8)`.
2. Vanguard vacates `(4,8)` to `(4,6)`.
3. Interceptor moves `(5,9)` to `(7,8)`.
4. Interceptor uses the real Vector Pulse action on Controller `(6,8)`, pushing it to `(5,8)` — the Regent footprint and explosive cell.
5. Finish all player activations and call normal `resolve_enemy_phase()`.
6. Regent resolves first at initiative 45. A hit on the displaced Controller applies enemy damage; the same committed footprint damages the explosive and can trigger the existing explosion path.

Use a fixed deterministic seed in the test after confirming the actual current RNG sequence. Do not use a seed sweep and do not bypass `BattleState::attack` or `resolve_enemy_phase()` with test-only mutation shortcuts.

The test should pin the tactical facts rather than incidental event counts:

- Vector Pulse performs real player damage + push onto `(5,8)`.
- Regent still resolves the weapon/profile/footprint committed before player movement.
- The displaced Controller is damaged by the Regent before its own lower-initiative intent.
- The explosive at `(5,8)` is damaged/triggered by the same committed footprint.
- No retargeting occurs after the player vacates the original target cell.

### Objectives and rewards

```text
Primary: EliminateTarget { target: REGENT }
Copy: Destroy the Regent and break the command net.

Optional: VictoryByRound { round: 6 }
Copy: Final Push: destroy the Regent by the end of Round 6.

Base reward: 1000 credits
Optional reward: 300 credits
```

The optional objective adds pressure using an existing rule. It does not require a new score/rank system and is never required for progression.

There is no separate hard failure turn limit. If the encounter is too long, tune HP/placements/enemy count before adding another rule.

### Story

Reuse existing VN backgrounds and portraits only.

Pre-mission scene:

1. Control: “The last command node is ahead. The Regent is broadcasting firing solutions to everything still standing.”
2. Vanguard: “Then we make its final order point the wrong way.”
3. Control: “Break the Regent. Once the command net drops, Relay Nine is ours.”

Aftermath/ending lead-in:

1. Vanguard: “Regent down. The remaining signatures are scattering.”
2. Control: “Relay Nine is secure. Bring everyone home.”
3. Vanguard: “Copy. Mission complete.”

No new narrative engine or ending art is required.

## Campaign completion

### Replace the Seven handoff with an authored final mission

Register `MissionId::Seven` in `mission_definition` and add `mission_seven.rs`.

Change `MissionDefinition.unlocks` from `MissionId` to `Option<MissionId>`:

```rust
pub unlocks: Option<MissionId>,
```

Missions One–Six use `Some(next)`. Mission Seven uses:

```rust
unlocks: None
```

This is clearer than introducing a fake Mission Eight or having Mission Seven unlock itself.

### Persist completion explicitly

Add one field to `CampaignState`:

```rust
pub completed: bool,
```

`CampaignState::new_game()` sets it to `false`.

No `#[serde(default)]`, version field, converter, or migration is added. Existing pre-HPA-386 saves may stop loading; the project explicitly does not preserve pre-release save compatibility.

Update mission completion as follows:

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

// award once
match definition.unlocks {
    Some(next) => self.next_mission = next,
    None => self.completed = true,
}
```

For Mission 7, `next_mission` remains `Seven` and `completed` becomes true after the reward is applied. A second completion attempt fails before awarding credits, so campaign completion is idempotent exactly once.

### Ending flow

The current `NextMission` screen exists only to represent the unauthored Mission 7 handoff. Once Seven is authored, rename it to `Ending` rather than keep a misleading terminal-sentinel name.

Game screens become:

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
Continue, Missions 2–7 -> Upgrade
Aftermath, completed == false -> Upgrade
Aftermath, completed == true -> Ending
Proceed from Upgrade -> current mission story
Ending -> Return to Title
```

The final mission intentionally skips the post-Mission-7 upgrade screen. Its reward is still persisted and displayed in the aftermath/ending totals, but there is no future mission to upgrade for.

Ending copy stays small:

```text
CAMPAIGN COMPLETE
Relay Nine secured.

Final credits: <credits>

Vanguard  <levels>
Gunner    <levels>
Interceptor <levels>

RETURN TO TITLE
```

A completed save keeps Continue enabled and reopens the ending rather than trying to construct another battle.

## Board-first presentation finish

HPA-386 should not reimplement presentation that already works. Keep the current selection/reachable tiles, telegraph overlays, intent guides, movement/push animation, impact pulse, target shake, and KO shrink.

Add only the concrete gaps visible in the current event playback path:

### Attack motion

When an `AttackRolled` event begins, give the attacker a short pulse/forward emphasis using the existing event timer. This is a presentation-only transform effect; no projectile entity is required for every weapon.

Existing impact effects remain the primary hit feedback.

### Damage numbers

On `BattleEvent::DamageApplied`, spawn one transient world-space `Text2d` child at the target cell:

```text
-7
-12
```

The number rises slightly during the existing event duration and despawns with the event effect. Use the event's `amount`; do not add a second damage-calculation path or a new domain event.

### Modest boss/signature camera emphasis

Mark the battle camera with a small component that retains its authored rest transform. During an `AttackRolled` from `Dreadnought` or `Regent`, apply a deterministic low-amplitude sinusoidal offset and restore the camera to the rest transform when the event ends.

Keep the amplitude small enough that the board and telegraphs remain readable. Do not create a cinematic camera controller, cut-in, zoom timeline, or battle-animation scene.

The three player pilot skills already receive command/status feedback and ordinary attack events. HPA-386 does not add a general signature-animation framework solely to specialize each skill.

### Audio

Skip sound cues in HPA-386. The repository has no existing audio path, and adding one would be a new subsystem for optional polish rather than MVP closeout.

## Regent visual

Append one final scene to the existing checked-in glTF.

```text
Scene 14: Regent
Root node 77
Part nodes 78–83
Mesh/material 14
Root scale 1.20
Material name: Regent Violet
Base color: [0.42, 0.14, 0.78, 1.0]
```

Final asset counts:

```text
15 scenes
84 nodes
15 meshes
15 materials
1 buffer
```

Map `UnitArchetype::Regent -> scene 14` and raise `MISSION_ONE_SCENE_COUNT` to 15.

Keep the same shared accessors/buffer, no new texture, animation, glTF file, generator, or asset pipeline.

## Whole-campaign tuning and validation

HPA-386 is the one intentional end-to-end playtest ticket. Use a clean New Game save and play Missions 1–7 in order through the ending.

Record one row per mission in `docs/validation/hpa-386.md`:

```text
mission | wall-clock minutes | rounds | restarts | optional complete? | committed-intent manipulation materially rewarded? | notes/tuning
```

Also record:

- total first-playthrough time;
- credits before/after each mission;
- upgrades purchased and when;
- any telegraph/readability issue;
- whether each boss threshold occurs at a useful point in the encounter.

Acceptance for the ledger:

- Total first-playthrough time is approximately 120–180 minutes. A small deviation is acceptable only if the ledger states the measured total and why further tuning would make the game worse.
- At least **4 of 7** missions materially reward reading/manipulating committed intent rather than only maximizing player damage. This is the strict majority required by the ticket; the ledger should identify the concrete moment in each qualifying mission.
- A base-reward-only campaign path must still afford meaningful upgrades. Automated coverage should prove the 3300 base credits available before Mission 7 can buy one chosen track to level 2 on each of the three mechs (600 credits per mech, 1800 total) without optional rewards.
- Optional rewards remain acceleration/customization, never a prerequisite.

### Tuning policy

Only tune values when the playtest shows a concrete problem. Prefer this order:

1. placement/opening geometry;
2. enemy count;
3. boss HP/threshold timing;
4. authored round pressure;
5. weapon hit/damage/EN values;
6. rewards/upgrade costs or effects.

Change one dimension at a time and replay the affected mission. Do not add mechanics to solve a tuning problem.

## Test contract

Automated coverage must prove at least:

1. Regent uses Command Barrage above half HP and Rupture Beam at/below half HP.
2. Dreadnought retains its existing 21/20 behavior after the shared selector is extended.
3. Crossing the Regent threshold does not rewrite a committed intent; the next intent uses Rupture Beam.
4. Regent initiative is 45 and is above Dreadnought 40 / Controller 35.
5. Regent remains normally pushable.
6. Mission 7 board dimensions, blockers, hazards, explosive, roster, exact opening rows, objective, rewards, boss stats/weapons, and opening legality are pinned.
7. The authored opening manipulation line uses the real Vector Pulse action and normal `resolve_enemy_phase()`, demonstrating committed Regent fire damaging the displaced Controller/explosive without retargeting.
8. Regent KO completes Mission 7 even if escorts remain alive.
9. Mission Seven is authored and has `unlocks == None`; Missions One–Six have the expected `Some(next)` chain.
10. Mission 7 completion sets `CampaignState.completed` and awards its reward exactly once; a second completion call is an atomic error/no-op.
11. Save round-trip preserves `completed`, credits, and upgrades; no compatibility migration is expected.
12. Continue on a completed save routes to Ending; Mission Seven before completion routes through Upgrade -> story -> battle.
13. Final aftermath routes directly to Ending, while earlier aftermath still routes to Upgrade.
14. The base-only 3300-credit pre-Mission-7 path can buy a level-2 track for all three mechs without optional rewards.
15. The Regent glTF scene and final global asset counts are pinned.
16. Focused presentation tests cover damage-number copy/lifetime behavior and boss-camera emphasis selection without snapshotting an entire rendered frame.
17. Existing repository format, strict Clippy, coverage-backed tests, and release build gates stay green.

## Documentation and closeout

Update `README.md` and `CLAUDE.md` only where their current campaign description stops at Mission 6/Seven handoff or describes the old terminal screen. Do not turn HPA-386 into a documentation rewrite.

The validation ledger is the source of truth for the final playthrough timing and any evidence-driven tuning changes.

## Risks

- **Final completion idempotence.** A self-unlocking Mission 7 would allow duplicate rewards. Use `completed` + `unlocks: None` and test the second completion attempt.
- **Save blast radius.** Adding `completed` intentionally breaks old JSON saves and every direct `CampaignState` fixture must be updated in the same implementation task. Do not add migration code to hide that fact.
- **Mission registration blast radius.** `MissionId::Seven` changes from terminal/unauthored to authored. Existing Continue/Proceed/terminal assertions across campaign tests must move in the same task so no intermediate commit knowingly leaves `cargo test --all-targets` red.
- **Boss selector generalization.** Generalize only the exact shared half-HP slot rule. Do not grow `unit_weapon` into threshold tables or callbacks.
- **Opening density.** Five enemy telegraphs plus the explosive must remain readable. If the playtest is noisy, remove or reposition an escort before inventing overlay filtering.
- **Presentation transform drift.** Attack pulses/camera shake must restore authored transforms after each event. Pin helper behavior and keep the effect deterministic.
- **Asset append blast radius.** Existing glTF tests pin global counts; update all relevant count assertions together with the scene append.
- **Closeout scope creep.** Whole-campaign playtesting may reveal ideas that are improvements rather than blockers. Record them outside this MVP rather than adding systems to HPA-386.

## Scope guardrails

No extra playable mech, seventh regular enemy, deployment/team composition, pilot leveling, Spirit Commands/SP, morale, parts/equipment, ammo, weapon-specific upgrades, elements, generic statuses, overwatch, permadeath, repair costs, procedural maps, branching routes, difficulty modes, rank/score system, achievements, New Game+, multiplayer, generic boss/threshold scripting, multi-tile boss, boss parts, invulnerability, cinematic battle scene, cut-in framework, audio subsystem, analytics/tuning framework, save migration, new dependency/crate, second glTF, or second PR.
