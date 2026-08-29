# HPA-637 Missions 2–3 and Flanker Design

## Outcome

Extend the validated Scorpius campaign through Missions 2 and 3 and add Flanker without introducing a generic objective or AI framework.

Keep one ticket = one PR, one Rust 2024 / Bevy 0.19 application crate, plain-Rust domain rules, typed mission authoring, existing campaign/save/UI composition, and the committed-intent invariant.

## Closed mission rules

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

`BattleState` stores `MissionRules`. Rename the Mission1-only objective/result bit `turnabout_complete` to generic `optional_complete`. Turnabout's qualifying damage trigger stays special; no objective callback/trait registry.

## One named round boundary

Both protect duration and interception deadline use:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

At this state, `round` is the number of completed player/enemy rounds. Do not duplicate the phase/round predicate.

## Primary outcome semantics

Global squad wipe remains defeat.

### Mission 1

Eliminate all enemies, unchanged.

### Mission 2

`ProtectThroughRound { target: GUNNER, round: 3 }`:

```text
Gunner KO -> defeat
no living enemy and Gunner alive -> immediate victory
completed_enemy_round(3) and Gunner alive -> victory
otherwise continue
```

Player-facing primary copy is explicit:

`Protect Gunner through the end of Round 3, or eliminate all attackers.`

This preserves the defense lesson without forcing empty-board reaction clicks after every threat is gone.

### Mission 3

`InterceptBeforeEscape { target: COURIER, escape: (8,0), deadline_round: 5 }`:

```text
Courier KO -> victory, escorts irrelevant
Courier at (8,0) -> defeat
completed_enemy_round(5) with Courier alive -> defeat
escort clear alone -> continue
```

Extraction is the normal failure. Round 5 is only the anti-stall/body-block backstop.

## Mission 3 clock and geometry

Board: **9×9**. Courier start `(0,6)`, extraction `(8,0)`, Move4, open-path Manhattan distance14.

```text
round0 begin: opening, Courier stays -> player Round1
round1 begin: move1 <=4 -> player Round2
round2 begin: move2 <=8 -> player Round3
round3 begin: move3 <=12 -> player Round4, cannot extract
round4 begin: deadline5 not reached; move4 can reach extraction -> extraction defeat
round5 begin: only if blocked/delayed; deadline fires before another move
```

Required tests:

- player Round4 exists after three later moves;
- Courier is not on extraction then;
- open-route fourth later move reaches `(8,0)` after player Round4 and fails;
- occupied exit reaches player Round5, then deadline fails before another Courier move;
- player `resolve_push` onto `(8,0)` fails immediately.

Do not pin exact intermediate distance; it is tie-break detail.

## Bonus semantics

One `optional_complete` bit:

- Turnabout: existing event-driven damage trigger.
- Hold Fast: Gunner at or above 50% HP when M2 wins.
- Swift Intercept: Courier defeated by end Round2.

New terminal bonus event precedes `MissionCompleted`. Defeat does not newly earn a terminal-only bonus.

## Mission-authored openings

Replace Mission1 opening movement/targeting hardcodes with `EnemyOpening` data. Mission1 stays exact:

```text
Rifleman L -> (2,5), Gunner
Rifleman R -> (6,5), Interceptor
Striker -> (4,6), Vanguard
Artillery -> (4,0), Vanguard
```

Round0 authored placement remains direct scripted movement. Later movement remains deterministic planner logic.

## Shared enemies and Flanker behavior

Create `mission::enemies`, mirroring `squad.rs`.

```text
Rifleman HP9 Armor1 Move2 Acc72 Eva5
Striker HP12 Armor2 Move2 Acc78 Eva10
Artillery HP10 Armor1 Move1 Acc90 Eva0
Flanker HP8 Armor0 Move4 Acc82 Eva30
```

Flanker weapon: Skirmish Carbine, range1–2, Single, damage4, hit+5, crit10, EN0, no push/counter.

Planner:

- Protect Flanker: weapon-band distance to protected Gunner, Manhattan, more open neighbors, y,x; prefer Gunner target when legal.
- Courier: Manhattan to extraction, more open neighbors, y,x.
- Other Flanker: reuse Rifleman/Striker attack-band movement instead of standing still.

No policy objects or RNG.

Existing initiative match becomes:

```rust
Striker => 30,
Flanker => 25,
Rifleman => 20,
Artillery => 10,
_ => 0,
```

This removes the `Rifleman if unit.position.x < 4` hack. Mission1 left/right order remains through attacker-ID tie-break.

## Mission 2 — Hold Relay Nine

Board **9×9**.

```text
Players: V(3,7), G(4,6), I(5,7)
Blocking: (3,3),(5,3),(2,6),(6,6)
Hazards: (1,5),(7,5)
Explosive: (6,4), HP4

Rifleman21 start(2,2) -> opening(2,4), target Vanguard
Striker22 start(4,3) -> opening(4,5), target Gunner
Artillery23 start/stay(4,0), target Gunner
Flanker24 start(8,4) -> opening(5,5), target Interceptor
```

```text
Title: Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
Reward: 400 + 100
Unlock: Three
```

Reuse existing VN images. Copy states the three-round upload and the eliminate-all early-safe alternative.

## Mission 3 — Cut the Courier

Board **9×9**.

```text
Players: V(4,7), G(3,8), I(5,8)
Blocking: (4,3),(4,4),(4,5)
Hazard: (2,5)
Explosive: (6,3), HP4
Extraction: (8,0)

Courier31 Flanker start/stay(0,6)
Rifleman32 start(3,2) -> opening(3,4), target Vanguard
Striker33 start(6,6) -> opening(5,7), target Interceptor
```

```text
Title: Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by end Round2.
Reward: 500 + 150
Unlock: Four
```

VN line: `If it gets out, or Round 5 closes, the data is gone.`

## Authoring validation

Do not add runtime validation machinery. Mission tests assert:

- board 9×9;
- rule targets exist/correct faction;
- opening unit/target refs exist;
- opening destinations legal;
- Mission3 extraction in bounds, non-blocking, non-hazard, no live explosive;
- start→escape Manhattan14.

Runtime authored-data `expect` remains acceptable because CI pins constants.

## Mission IDs and campaign

Add `MissionId { One, Two, Three, Four }` once in the Mission2 task. Final authored: One/Two/Three; Four handoff.

```text
Continue One -> story
Continue Two/Three -> Upgrade
Continue Four -> handoff
Proceed authored -> story
Proceed unauthored -> handoff
```

Save shape remains next mission + credits + upgrades.

## HUD/results

- M1: enemy count.
- M2: Round n/3 + Gunner HP.
- M3: Round n/5 + Courier Manhattan distance to extraction.
- Generic terminal achieved/missed bonus.
- Playback: `BONUS OBJECTIVE COMPLETE`.
- Aftermath: `Bonus +...`.

Extraction ring uses existing white ring mesh/material at the authored escape cell.

## Distinct Flanker glTF scene

Extend existing `assets/models/mission_one.gltf` with scene10 rather than reusing friendly Interceptor scene2.

Scene `Flanker`, nodes49–55, mesh10/material10 `Flanker Magenta`.

Material:

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

Nodes49–55 use the existing cuboid accessors with slimmer proportions defined in the implementation plan. Set scene count11 and `scene_index(Flanker)=10`. Keep root scale0.72. Do not add `unit_scale`, child under-ring, or inverse-scale math.

## Required automated coverage

- Mission1 opening/order unchanged.
- completed_enemy_round phase semantics.
- M2 target KO fail, immediate clear win, Round3 survival win with enemy alive, bonus boundary.
- M3 escort-clear non-win, Courier KO win, player Round4, live move4 extraction, blocked Round5 deadline, push-to-exit loss, bonus boundary.
- Flanker protect/Courier/fallback/tie-break behavior.
- initiative 30/25/20/10.
- authoring reference legality.
- glTF scene10/count11.
- M2 HUD n/3, M3 HUD n/5, generic result/reward copy.
- One→Two→Three→Four save/upgrade progression; 1200 base credits.

## Manual validation

M2: competing threats, immediate-clear win, full Round3 win, Gunner KO, bonus states. M3: distinct magenta Courier, extraction ring, player Round4, open extraction, blocked Round5 fallback, Courier-only victory, bonus states. Verify save/Continue/upgrades and full local gates.

## Scope guardrails

No new dependency, objective/AI framework, neutral objective role, status system, new playable unit, mission select, branching, difficulty, runtime asset pipeline, save migration, or second PR.

Keep `sync.rs`, `app.rs`, save/session implementation, VN assets, Cargo files unchanged unless concrete tests force a small correction.

## Decision summary

Mission2 does not punish success with empty turns. Mission3 has a live extraction failure and Round5 anti-stall backstop. Flanker has a real enemy scene. One helper owns the subtle round-boundary meaning, authored references are tested, the positional initiative hack is removed, and the architecture stays deliberately small.