# HPA-637 Missions 2–3 and Flanker Design

## Context

HPA-637 extends the completed HPA-635 campaign loop through Missions 2 and 3 and adds the fourth regular enemy, Flanker. The implementation remains one Rust/Bevy application crate and one ticket/PR.

The design intentionally adds only what these two missions consume: three closed primary objective shapes, three closed bonus shapes, one authored opening slice, one shared regular-enemy catalog, and one explicit Flanker planner branch. No objective framework, behavior tree, neutral faction, scripting/data format, new status system, or new save layer is justified.

## Locked product decisions

- Mission 2 protects the existing Gunner.
- Mission 2 wins when Gunner survives through the full third enemy resolution **or** all attackers are eliminated while Gunner is alive. Empty-board reaction clicking is not gameplay.
- Mission 3 Courier starts `(0,6)`, extraction is `(8,0)`, movement is 4, open-path distance is 14.
- Mission 3 deadline is **Round 5**. Player Round 4 is guaranteed; on an open route the Courier's fourth later move can reach extraction after player Round 4. Round 5 exists only as the body-block/stall backstop.
- Flanker is HP8 / Armor0 / Move4 / Acc82 / Eva30 with one range1–2 Skirmish Carbine (damage4, hit+5, crit10, EN0, no push/counter).
- Flanker outside protect/intercept special cases reuses normal attack-band movement instead of standing still.
- Existing initiative ordering is simplified to Striker30 / Flanker25 / Rifleman20 / Artillery10. No new initiative system/field.
- Flanker gets its own checked-in glTF scene 10 rather than borrowing the player's cyan Interceptor scene.
- Existing campaign/save/upgrade screens remain the composition surface: M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff.

## Domain rules

```rust
pub enum PrimaryObjective {
    EliminateAllEnemies,
    ProtectThroughRound { target: UnitId, round: u16 },
    InterceptBeforeEscape { target: UnitId, escape: GridPos, deadline_round: u16 },
}

pub enum OptionalObjective {
    Turnabout,
    ProtectTargetAtHalfHp { target: UnitId },
    VictoryByRound { round: u16 },
}

pub struct EnemyOpening {
    pub unit: UnitId,
    pub destination: GridPos,
    pub target: Option<UnitId>,
}

pub struct MissionRules {
    pub primary: PrimaryObjective,
    pub optional: OptionalObjective,
    pub opening_plan: &'static [EnemyOpening],
}
```

`BattleState` stores `MissionRules`. `ObjectiveProgress` and `MissionResult` rename the Mission1-specific Turnabout bit to `optional_complete`.

### One named round-boundary predicate

`begin_round()` checks terminal state while `phase == EnemyPlanning` before movement/increment. Keep that subtle meaning in one helper:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

Both protect duration and interception deadline use this helper.

### Protect outcome

```text
Gunner KO -> defeat
no living enemies while Gunner alive -> victory
otherwise completed_enemy_round(3) while Gunner alive -> victory
```

Player-facing primary copy is explicit:

`Protect Gunner through the end of Round 3, or eliminate all attackers.`

### Intercept outcome

```text
Courier KO -> victory even with escorts alive
Courier at (8,0) -> defeat
completed_enemy_round(5) with living Courier -> deadline defeat
escort clear alone -> continue
```

Round5 is the anti-stall backstop; extraction is the normal failure.

## Mission 3 clock

Mission 3 is 9×9.

```text
round0 begin: opening; Courier stays (0,6)  -> player Round1
round1 begin: move1 <=4                       -> player Round2
round2 begin: move2 <=8 total                 -> player Round3
round3 begin: move3 <=12 total                -> player Round4; cannot extract
round4 begin: 4 < deadline5; move4 can reach (8,0) -> extraction defeat
round5 begin: only if exit blocked/delayed; deadline fires before another move
```

Automated coverage must prove Round4 exists, open-route extraction occurs after Round4, blocked exit reaches the Round5 backstop, and `resolve_push` can trigger an immediate player-caused loss by pushing Courier onto extraction.

Do not pin an exact intermediate remaining distance; that is tie-break detail.

## Authored openings

Mission1 hardcoded opening movement/targeting moves into `MissionRules::opening_plan` without changing behavior:

```text
Rifleman L -> (2,5), Gunner
Rifleman R -> (6,5), Interceptor
Striker -> (4,6), Vanguard
Artillery -> (4,0), Vanguard
```

Opening placement remains scripted direct movement; later rounds use the existing deterministic planner.

## Shared regular enemies

Create `mission::enemies` mirroring `mission::squad`. It owns fixed constructors/weapon specs for Rifleman, Striker, Artillery, Flanker. Missions keep IDs, board, positions, openings, dialogue, rewards.

## Flanker planner

Protect mission: score reachable cells by distance to protected-target weapon range band, Manhattan distance, more open neighbors, y, x. Prefer protected Gunner in target selection when legal.

Courier: score reachable cells by Manhattan distance to extraction, more open neighbors, y, x.

Fallback Flanker: reuse Rifleman/Striker attack-band movement.

No RNG or policy abstraction is added. Committed intents remain immutable during player phase.

## Mission 2 — Hold Relay Nine

Board **9×9**.

```text
Players: Vanguard (3,7), Gunner (4,6), Interceptor (5,7)
Blocking: (3,3), (5,3), (2,6), (6,6)
Hazards: (1,5), (7,5)
Explosive: (6,4), HP4

Rifleman21 start (2,2) -> opening (2,4), target Vanguard
Striker22 start (4,3) -> opening (4,5), target Gunner
Artillery23 start/stay (4,0), target Gunner
Flanker24 start (8,4) -> opening (5,5), target Interceptor
```

```text
Title: Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
Reward: 400 + 100
Unlock: Mission 3
```

VN reuses existing Relay Nine/Control/Vanguard images. Opening copy explicitly mentions the full three rounds and the alternative of eliminating all attackers.

## Mission 3 — Cut the Courier

Board **9×9**.

```text
Players: Vanguard (4,7), Gunner (3,8), Interceptor (5,8)
Blocking: (4,3), (4,4), (4,5)
Hazard: (2,5)
Explosive: (6,3), HP4
Extraction: (8,0)

Courier31 Flanker start/stay (0,6)
Rifleman32 start (3,2) -> opening (3,4), target Vanguard
Striker33 start (6,6) -> opening (5,7), target Interceptor
```

```text
Title: Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by the end of Round 2.
Reward: 500 + 150
Unlock: Mission 4
```

VN says `If it gets out, or Round 5 closes, the data is gone.`

## Authoring validation

Do not add runtime authoring-validation machinery. Mission tests assert:

- board dimensions 9×9;
- rule target exists with correct faction;
- opening enemy/target IDs exist;
- opening destinations are in bounds/non-blocking;
- Mission3 extraction is in bounds, non-blocking, non-hazard, no live explosive;
- Courier start→escape Manhattan distance is 14.

Runtime authored-data `expect(...)` remains acceptable because CI pins constants.

## Mission IDs/campaign routing

Add `MissionId { One, Two, Three, Four }` once when Mission2 lands. One/Two authored first, Three/Four handoffs; then Task4 authors Three. Final dispatch: One/Two/Three Some, Four None.

```text
Continue One -> story
Continue Two/Three -> Upgrade
Continue Four -> handoff

Upgrade Proceed:
  authored next -> story
  otherwise -> handoff
```

Save shape remains next mission + credits + upgrades.

## Presentation

HUD derives progress from rules:

- M1 enemy count;
- M2 Round n/3 + Gunner HP;
- M3 Round n/5 + Courier Manhattan distance to extraction.

Results use authored mission title/objective copy + generic achieved/missed bonus. `OptionalObjectiveCompleted` playback becomes `BONUS OBJECTIVE COMPLETE`; aftermath uses `Bonus +...`.

Extraction ring uses the existing white `ring_mesh + intended_target` material at `(8,0)`.

## Distinct Flanker glTF scene

Extend `assets/models/mission_one.gltf` with scene index **10**. Do not reuse Interceptor scene2 and do not add `unit_scale`/child under-ring compensation.

Scene `Flanker`: nodes 49–55, mesh10/material10 `Flanker Magenta`.

```json
{
  "name": "Flanker Magenta",
  "pbrMetallicRoughness": {
    "baseColorFactor": [0.78, 0.08, 0.46, 1.0],
    "metallicFactor": 0.25,
    "roughnessFactor": 0.62
  },
  "emissiveFactor": [0.08, 0.0, 0.04]
}
```

Nodes use shared cuboid mesh accessors:

```text
49 Left Leg      [-0.16,0.18, 0.00] [0.12,0.36,0.16]
50 Right Leg     [ 0.16,0.18, 0.00] [0.12,0.36,0.16]
51 Torso         [ 0.00,0.62, 0.00] [0.36,0.42,0.28]
52 Head          [ 0.00,0.95, 0.00] [0.20,0.20,0.20]
53 Left Fin      [-0.42,0.67,-0.10] [0.42,0.08,0.28]
54 Right Fin     [ 0.42,0.67,-0.10] [0.42,0.08,0.28]
55 Rear Thruster [ 0.00,0.52,-0.34] [0.20,0.16,0.34]
```

Set `MISSION_ONE_SCENE_COUNT=11`, `scene_index(Flanker)=10`, keep root scale0.72.

## Rewards

Base-only total through Mission3 is **1200 credits** (300+400+500). Bonuses are 100/100/150. No bonus/grind is required for useful upgrades.

## Required automated coverage

- Mission1 opening/intent order unchanged.
- `completed_enemy_round` phase semantics.
- M2 target KO fail, immediate enemy-clear win, full Round3 survival win with enemies alive, half-HP bonus boundary.
- M3 Courier KO/escort-clear semantics, Round4 availability, open extraction after Round4, blocked-exit Round5 deadline, push-to-extraction fail, early bonus.
- Flanker protect/Courier/fallback movement and open-neighbor tie-break.
- Initiative constants Striker30/Flanker25/Rifleman20/Artillery10.
- M2/M3 authored-reference legality.
- glTF scene10 exists; scene count11.
- M2 HUD n/3; M3 HUD n/5; objective/result/reward copy generic.
- One→Two→Three→Four save/upgrade progression; 1200 base credits.

## Manual validation

Validate M2 competing threats, early-clear win, Round3 survival win, Gunner KO, bonus states. Validate distinct magenta Courier, extraction ring, player Round4, open extraction after Round4, blocked Round5 backstop, Courier-only win, bonus states, save/Continue continuity.

## File boundaries

New: `mission/enemies.rs`, `mission_two.rs`, `mission_three.rs`, validation ledger.

Modified: domain model/battle/enemy; mission mod/mission_one; campaign progression; presentation assets/battlefield/interaction/ui/campaign_ui; existing glTF; campaign flow/persistence/presentation tests; README; CLAUDE.md.

Keep `presentation/sync.rs`, `app.rs`, save/session implementation, VN assets, Cargo files unchanged unless concrete tests force a small correction.

## Decision summary

Keep the architecture minimal while making both encounters honest: Mission2 does not punish success with empty turns; Mission3's extraction is a live failure and Round5 is only the stall backstop. Flanker gets a proper checked-in enemy scene. Round-boundary semantics are named once, authored references are tested, the remaining positional initiative hack is removed, and no new framework is introduced.