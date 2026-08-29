# HPA-637 Missions 2–3 and Flanker Design

## Outcome

Extend Scorpius through Missions 2–3 and add Flanker with the smallest typed extension of existing battle/mission/campaign seams. One ticket = one PR; no generic objective/AI framework.

## Locked rules

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

pub struct EnemyOpening { pub unit: UnitId, pub destination: GridPos, pub target: Option<UnitId> }
pub struct MissionRules { pub primary: PrimaryObjective, pub optional: OptionalObjective, pub opening_plan: &'static [EnemyOpening] }
```

BattleState stores rules. Rename active objective/result `turnabout_complete` to generic `optional_complete`; keep Turnabout's damage trigger special.

Use one boundary helper:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

## Mission2

Protect Gunner:

```text
Gunner KO -> defeat
all attackers gone + Gunner alive -> immediate victory
completed_enemy_round(3) + Gunner alive -> victory
```

Board 9×9:

```text
Players V(3,7) G(4,6) I(5,7)
Blocking (3,3),(5,3),(2,6),(6,6)
Hazards (1,5),(7,5)
Explosive (6,4) HP4
Rifleman21 (2,2)->(2,4), Vanguard
Striker22 (4,3)->(4,5), Gunner
Artillery23 (4,0), Gunner
Flanker24 (8,4)->(5,5), Interceptor
```

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
400+100; unlock Three
```

## Mission3

Intercept Courier:

```text
Courier KO -> victory
Courier at (8,0) -> defeat
completed_enemy_round(5) with Courier alive -> defeat
escort clear alone -> continue
```

Board 9×9:

```text
Players V(4,7) G(3,8) I(5,8)
Blocking (4,3),(4,4),(4,5)
Hazard (2,5)
Explosive (6,3) HP4
Extraction (8,0)
Courier31 Flanker start/stay(0,6)
Rifleman32 (3,2)->(3,4), Vanguard
Striker33 (6,6)->(5,7), Interceptor
```

Distance `(0,6)->(8,0)` is14; Move4; deadline5:

```text
move1 -> player2
move2 -> player3
move3 <=12 total -> player4, cannot extract
move4 after player4 can reach exit -> extraction defeat
deadline5 is only blocked/stalled fallback
```

Primary: `Intercept Courier before extraction or the end of Round 5.` Bonus Swift Intercept by Round2. Reward500+150; unlock Four.

Tests must prove Player4, live move4 extraction, blocked Round5 fallback, and `resolve_push` onto exit immediate failure. Do not pin exact intermediate distance.

## Flanker

HP8 Armor0 Move4 Acc82 Eva30. Skirmish Carbine range1–2 damage4 hit+5 crit10 EN0, no push/counter.

Protect planner: weapon-band distance to Gunner, Manhattan, open neighbors, y,x; prefer Gunner when legal. Courier planner: Manhattan to exit, open neighbors, y,x. Other Flanker: reuse normal attack-band movement. No RNG/policy object.

Initiative becomes Striker30 / Flanker25 / Rifleman20 / Artillery10; remove positional Rifleman hack, no new initiative field.

## Authored openings/enemies

Move Mission1 opening hardcodes to `EnemyOpening` data with exact behavior unchanged. Create `mission::enemies` mirroring `squad.rs` for fixed enemy factories/weapons.

Mission tests validate board dimensions, rule targets/opening refs/factions, legal opening destinations. Mission3 additionally validates legal extraction and Manhattan14. No runtime validation framework.

## Mission IDs/campaign

Add `MissionId { One, Two, Three, Four }` once in Mission2 task. Final authored One/Two/Three, Four handoff.

Continue One→story, Two/Three→Upgrade, Four→handoff. Proceed authored→story. Save shape unchanged.

## Presentation

M2 HUD Round n/3 + Gunner HP. M3 HUD Round n/5 + exit distance. Generic bonus/result copy; extraction uses existing white ring material.

Give Flanker checked-in glTF scene10 rather than Interceptor scene2 + scale workaround. Scene10 `Flanker`, nodes49–55, mesh/material10 `Flanker Magenta`; scene count11; root scale0.72. Reuse existing buffer/accessors. No `unit_scale`, child under-ring, inverse-scale math.

## Required coverage

- Mission1 opening/order unchanged; one boundary helper semantics.
- M2 KO fail, immediate clear win, Round3 win with attacker alive, bonus boundary.
- M3 escort-clear non-win, Courier KO win, Player4, move4 extraction, blocked Round5 fallback, push-to-exit fail, bonus boundary.
- Flanker protect/Courier/fallback/tie-break + initiative values.
- Authoring legality; glTF scene10/count11.
- M2 HUD n/3, M3 HUD n/5; generic result/reward.
- One→Two→Three→Four save/upgrades; base rewards total1200.

## Scope

No new dependency, framework, neutral role, status system, mission select, branching, difficulty, runtime asset pipeline, save migration, new VN asset, or second PR. Keep sync.rs, app.rs, save/session implementation and Cargo files unchanged unless concrete tests force a small correction.