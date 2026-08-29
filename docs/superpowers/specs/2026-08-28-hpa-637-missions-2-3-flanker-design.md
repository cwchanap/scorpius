# HPA-637 Missions 2–3 and Flanker Design

## Context

HPA-637 extends the completed HPA-635 campaign loop through Missions 2 and 3 and adds the fourth regular enemy, Flanker. The implementation remains one Rust/Bevy application crate and one ticket/PR.

The design intentionally adds only what these two missions consume: three closed primary objective shapes, three closed bonus shapes, one authored opening slice, one shared regular-enemy catalog, and one explicit Flanker planner branch. No objective framework, behavior tree, neutral faction, scripting/data format, new status system, or new save layer is justified.

## Locked decisions

- Mission 2 protects the existing Gunner.
- Mission 2 wins when Gunner survives through full Round 3 **or** every attacker is eliminated while Gunner lives. Do not make the player click through empty rounds.
- Mission 3 Courier starts `(0,6)`, extraction `(8,0)`, Move4, open-path distance14.
- Mission 3 deadline is **Round5**. Player Round4 is guaranteed; open-route move4 can extract after player Round4. Round5 is only anti-stall/body-block fallback.
- Flanker stats: HP8, Armor0, Move4, Acc82, Eva30; Skirmish Carbine range1–2/damage4/hit+5/crit10/EN0/no push/no counter.
- Non-objective Flanker reuses normal attack-band movement.
- Existing initiative match becomes Striker30 / Flanker25 / Rifleman20 / Artillery10. No new initiative field/system.
- Flanker gets checked-in glTF scene10, not friendly Interceptor scene2 + runtime scale/under-ring compensation.
- Campaign/save/upgrade flow remains M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff.

## Closed domain rules

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

`BattleState` stores rules. Rename objective/result `turnabout_complete` to generic `optional_complete`; keep Turnabout's damage trigger special.

## One round-boundary helper

Both new objectives depend on the same `begin_round()` ordering. Name it once:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

At this boundary `round` is the count of completed player/enemy rounds. Do not duplicate this predicate.

## Terminal outcome

Global squad wipe remains defeat.

Protect:

```text
Gunner KO -> defeat
no living enemy + Gunner alive -> victory
completed_enemy_round(3) + Gunner alive -> victory
otherwise continue
```

Primary copy: `Protect Gunner through the end of Round 3, or eliminate all attackers.`

Intercept:

```text
Courier KO -> victory
Courier at (8,0) -> defeat
completed_enemy_round(5) with living Courier -> defeat
escort clear alone -> continue
```

Extraction is normal failure. Round5 is fallback.

## Mission 3 clock

Mission3 is 9×9.

```text
round0 begin: opening, Courier stays -> player1
round1 begin: move1 <=4 -> player2
round2 begin: move2 <=8 -> player3
round3 begin: move3 <=12 -> player4, cannot extract
round4 begin: deadline5 not reached; move4 can reach (8,0) -> extraction defeat
round5 begin: only if blocked/delayed; deadline fails before another move
```

Tests must prove player4 exists, open-route extraction on move4, blocked exit reaches Round5 deadline, and `resolve_push` onto extraction fails immediately. Do not pin exact intermediate distance.

## Bonus semantics

One `optional_complete` bit. Turnabout remains event-driven. Hold Fast checks Gunner ≥50% HP on victory. Swift Intercept checks victory by Round2. New terminal bonus event precedes MissionCompleted.

## Openings

Mission1 opening becomes authored data but remains exact:

```text
Rifleman L -> (2,5), Gunner
Rifleman R -> (6,5), Interceptor
Striker -> (4,6), Vanguard
Artillery -> (4,0), Vanguard
```

Round0 placement remains direct scripted movement; later movement remains planner-driven.

## Shared enemies and Flanker planner

Create `mission::enemies` mirroring `squad.rs`. Missions keep IDs/board/positions/openings/copy/rewards.

Protect Flanker score: weapon-band distance to Gunner, Manhattan, more open neighbors, y,x; prefer Gunner target when legal.

Courier score: Manhattan to extraction, more open neighbors, y,x.

Fallback Flanker: current Rifleman/Striker attack-band scoring.

No RNG/policy object.

Initiative becomes:

```rust
Striker => 30,
Flanker => 25,
Rifleman => 20,
Artillery => 10,
_ => 0,
```

Equal Riflemen preserve left/right order through existing attacker-ID tie-break.

## Mission 2 content

Board **9×9**:

```text
Players V(3,7) G(4,6) I(5,7)
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
400 + 100; unlock Three
```

Reuse existing VN images. Copy explicitly mentions both three-round survival and eliminating all interrupting attackers.

## Mission 3 content

Board **9×9**:

```text
Players V(4,7) G(3,8) I(5,8)
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
500 + 150; unlock Four
```

VN line: `If it gets out, or Round 5 closes, the data is gone.`

## Authoring validation

Keep runtime `.expect` assumptions. Mission tests assert board9×9, rule targets exist/correct faction, opening references exist, destinations legal. Mission3 also asserts extraction in-bounds/non-blocking/non-hazard/no explosive and start→exit Manhattan14.

No validation framework.

## Mission IDs and routing

Add `MissionId { One, Two, Three, Four }` once in Mission2 task. Final authored: One/Two/Three; Four handoff.

```text
Continue One -> story
Continue Two/Three -> Upgrade
Continue Four -> handoff
Proceed authored -> story
Proceed unauthored -> handoff
```

Save shape unchanged.

## Presentation

HUD: M1 enemy count; M2 Round n/3 + Gunner HP; M3 Round n/5 + distance to extraction. Results use authored title/objectives + generic bonus status. Playback says `BONUS OBJECTIVE COMPLETE`; aftermath says `Bonus +...`.

Extraction ring uses existing white ring material at rule escape.

## Distinct Flanker scene

Extend existing glTF with scene10, mesh10/material10 `Flanker Magenta`; bump scene count to11; map Flanker→10. Keep root scale0.72. No `unit_scale`, child under-ring, inverse-scale math.

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

Scene nodes49–55 use mesh10 with slimmer fast-mech proportions specified in the implementation plan. Existing buffer/accessors stay unchanged.

## Required coverage

- Mission1 opening/order unchanged.
- One round-boundary helper semantics.
- M2 KO fail, immediate clear win, Round3 survival win with enemy alive, half-HP bonus.
- M3 escort-clear non-win, Courier KO win, player Round4, open move4 extraction, blocked Round5 deadline, push-to-exit loss, early bonus.
- Flanker protect/Courier/fallback movement + tie-break.
- Initiative constants 30/25/20/10.
- Authored-reference legality.
- glTF scene10 / scene count11.
- M2 HUD n/3; M3 HUD n/5; generic result/reward copy.
- One→Two→Three→Four save/upgrade progression; base total1200.

## Manual validation

M2: competing threats, immediate clear win, full Round3 win, Gunner KO, bonus states. M3: magenta Courier, extraction ring, player Round4, open extraction, blocked Round5 backstop, Courier-only win, bonus states. Verify save/Continue/upgrades and full CI-equivalent gates.

## File boundaries

New: enemies.rs, mission_two.rs, mission_three.rs, validation ledger.

Modified: domain model/battle/enemy; mission mod/Mission1; campaign progression; presentation assets/battlefield/interaction/ui/campaign_ui; existing glTF; campaign/persistence/presentation tests; README/CLAUDE.

Keep sync.rs, app.rs, save/session implementation, VN assets, Cargo files unchanged unless tests force a concrete correction.

## Decision summary

Mission2 does not punish success with empty rounds. Mission3 has a live extraction failure after player Round4 and a Round5 stall backstop. Flanker is a real checked-in enemy scene. Round-boundary semantics are named once, authoring assumptions are tested, positional initiative hack is removed, and the architecture remains deliberately small.