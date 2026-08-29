# HPA-637 Missions 2–3 and Flanker Design

## Outcome

Extend the validated Scorpius campaign through Missions 2 and 3 and add Flanker without introducing a generic objective or AI framework. Keep one Rust/Bevy application crate and one ticket/PR.

## Locked product behavior

- Mission 2 protects existing Gunner.
- Mission 2 wins when Gunner survives through full Round 3 **or** every attacker is eliminated while Gunner lives. No empty-board busywork.
- Mission 3 Courier starts `(0,6)`, extraction `(8,0)`, Move4, open-path Manhattan distance14.
- Mission 3 deadline is **Round5**. Player Round4 is guaranteed; fourth later move can normally extract after player Round4; Round5 is only anti-stall/body-block fallback.
- Flanker: HP8 / Armor0 / Move4 / Acc82 / Eva30; one Skirmish Carbine, range1–2, damage4, hit+5, crit10, EN0, no push/counter.
- Non-objective Flanker reuses normal attack-band movement instead of standing still.
- Initiative is simplified in the existing match: Striker30 / Flanker25 / Rifleman20 / Artillery10. No new initiative system/field.
- Flanker gets checked-in glTF scene10 instead of reusing friendly Interceptor scene2 with runtime scaling/under-ring compensation.
- Existing campaign/save/upgrade composition stays M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff.

## Closed domain seams

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

`BattleState` stores `MissionRules`. Rename Mission1-specific `turnabout_complete` state/result bit to generic `optional_complete`; Turnabout's damage trigger remains special.

## One round-boundary helper

Both new objective arms use only:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

At that boundary `round` is the number of completed player/enemy rounds. Do not duplicate this predicate.

## Terminal rules

Global squad wipe remains defeat.

Mission2:

```text
Gunner KO -> defeat
no living enemies + Gunner alive -> immediate victory
completed_enemy_round(3) + Gunner alive -> victory
otherwise continue
```

Primary copy: `Protect Gunner through the end of Round 3, or eliminate all attackers.`

Mission3:

```text
Courier KO -> victory, escorts irrelevant
Courier at (8,0) -> defeat
completed_enemy_round(5) with living Courier -> deadline defeat
escort clear alone -> continue
```

Extraction is normal failure. Round5 is fallback.

## Mission 3 clock

Board **9×9**.

```text
round0 begin: opening, Courier stays -> player1
round1 begin: move1 <=4 -> player2
round2 begin: move2 <=8 -> player3
round3 begin: move3 <=12 -> player4, cannot extract
round4 begin: deadline5 not reached; move4 can reach (8,0) -> extraction defeat
round5 begin: only if blocked/delayed; deadline fires before another move
```

Required automated contracts: player4 exists; open-route move4 extraction; blocked exit reaches Round5 deadline; `resolve_push` onto extraction immediately fails. Do not pin an exact intermediate distance.

## Optional objectives

One `optional_complete` bit. Turnabout remains event-driven. Hold Fast checks Gunner ≥50% HP when M2 wins. Swift Intercept checks Courier defeated by end Round2. Newly earned terminal bonus event precedes MissionCompleted.

## Mission-authored openings

Mission1 opening becomes authored data, behavior unchanged:

```text
Rifleman L -> (2,5), Gunner
Rifleman R -> (6,5), Interceptor
Striker -> (4,6), Vanguard
Artillery -> (4,0), Vanguard
```

Round0 authored placement remains direct scripted movement; later movement remains planner-driven.

## Shared enemies and Flanker planner

Create `mission::enemies` mirroring `squad.rs`. Missions keep IDs/board/positions/openings/copy/rewards.

Protect Flanker: score weapon-band distance to Gunner, Manhattan, more open neighbors, y,x; prefer Gunner target when legal.

Courier: score Manhattan to extraction, more open neighbors, y,x.

Fallback Flanker: reuse Rifleman/Striker attack-band destination scoring. No policy objects or RNG.

Initiative match:

```rust
Striker => 30,
Flanker => 25,
Rifleman => 20,
Artillery => 10,
_ => 0,
```

This removes the positional Rifleman hack; equal Riflemen retain left/right order via existing attacker-ID tie-break.

## Mission 2 — Hold Relay Nine

Board **9×9**.

```text
Players V(3,7), G(4,6), I(5,7)
Blocking (3,3),(5,3),(2,6),(6,6)
Hazards (1,5),(7,5)
Explosive (6,4) HP4
Rifleman21 start(2,2)->(2,4), Vanguard
Striker22 start(4,3)->(4,5), Gunner
Artillery23 (4,0), Gunner
Flanker24 start(8,4)->(5,5), Interceptor
```

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
Reward 400 + 100; unlock Three
```

Reuse existing VN images; copy explicitly mentions both survival and eliminating all interrupting attackers.

## Mission 3 — Cut the Courier

Board **9×9**.

```text
Players V(4,7), G(3,8), I(5,8)
Blocking (4,3),(4,4),(4,5)
Hazard (2,5)
Explosive (6,3) HP4
Extraction (8,0)
Courier31 Flanker start/stay(0,6)
Rifleman32 start(3,2)->(3,4), Vanguard
Striker33 start(6,6)->(5,7), Interceptor
```

```text
Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by end Round2.
Reward 500 + 150; unlock Four
```

VN says `If it gets out, or Round 5 closes, the data is gone.`

## Authoring validation

No runtime validation framework. Mission tests assert board dimensions, rule targets and opening IDs/factions, legal opening destinations. Mission3 also asserts extraction in bounds/non-blocking/non-hazard/no explosive and start→escape Manhattan14.

## Mission IDs and campaign

Add `MissionId { One, Two, Three, Four }` once in Mission2 task. Final authored One/Two/Three; Four handoff.

```text
Continue One -> story
Continue Two/Three -> Upgrade
Continue Four -> handoff
Proceed authored -> story
Proceed unauthored -> handoff
```

Save shape unchanged.

## Presentation

HUD: M1 enemy count; M2 Round n/3 + Gunner HP; M3 Round n/5 + Courier distance to extraction. Generic result/bonus copy. Playback `BONUS OBJECTIVE COMPLETE`; aftermath `Bonus +...`. Extraction ring uses existing white ring material at authored escape.

## Distinct Flanker scene

Extend `assets/models/mission_one.gltf` with scene10, mesh10/material10 `Flanker Magenta`; scene count11; `scene_index(Flanker)=10`; root scale remains0.72. No `unit_scale`, child under-ring, inverse-scale math.

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

Nodes49–55 use the existing cuboid accessors with slimmer proportions specified by the implementation plan; existing buffer/accessors unchanged.

## Required coverage

- Mission1 opening/order unchanged.
- completed_enemy_round phase semantics.
- M2 KO fail, immediate clear win, Round3 survival win with enemy alive, half-HP bonus.
- M3 escort-clear non-win, Courier KO win, player Round4, live move4 extraction, blocked Round5 deadline, push-to-exit loss, bonus boundary.
- Flanker protect/Courier/fallback/tie-break and initiative values.
- authored-reference legality.
- glTF scene10/count11.
- M2 HUD n/3, M3 HUD n/5, generic result/reward copy.
- One→Two→Three→Four save/upgrade flow; 1200 base credits.

## Scope

No new dependency, objective/AI framework, neutral objective role, status system, mission select, branching, difficulty, runtime asset pipeline, save migration, second PR, or new VN asset. Keep sync.rs, app.rs, save/session implementation, Cargo files unchanged unless concrete tests force a small correction.

## Decision summary

Mission2 does not punish success with empty turns. Mission3 has a live extraction failure and Round5 anti-stall fallback. Flanker gets a real enemy scene. One helper owns round-boundary semantics, authoring assumptions are tested, the positional initiative hack is removed, and the architecture remains deliberately small.