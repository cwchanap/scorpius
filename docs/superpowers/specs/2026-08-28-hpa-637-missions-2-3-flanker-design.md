# HPA-637 Missions 2–3 and Flanker Design

## Outcome

Extend Scorpius through Missions 2–3 and add Flanker without introducing a generic objective/AI framework. Keep one Rust 2024 / Bevy 0.19 application crate and one ticket/PR.

## Locked behavior

- Mission2 protects Gunner. Gunner KO fails. Clearing all attackers with Gunner alive wins immediately. Otherwise surviving through completed Round3 wins.
- Mission3 Courier starts `(0,6)`, extraction `(8,0)`, Move4, open distance14. Deadline is Round5: player Round4 exists; fourth later move can extract; Round5 is only blocked/stalled fallback.
- Flanker: HP8 Armor0 Move4 Acc82 Eva30; Skirmish Carbine range1–2 damage4 hit+5 crit10 EN0 no push/counter.
- Non-objective Flanker uses existing attack-band movement.
- Initiative becomes Striker30 / Flanker25 / Rifleman20 / Artillery10; no positional Rifleman hack and no new initiative system.
- Flanker gets checked-in glTF scene10, not friendly Interceptor scene2 plus runtime scaling/under-ring code.
- Campaign remains M1 → Upgrade → M2 → Upgrade → M3 → M4 handoff.

## Domain seam

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

BattleState stores rules. Rename objective/result `turnabout_complete` to generic `optional_complete`; Turnabout's damage trigger stays special.

Both new round conditions use only:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

## Mission2

Board **9×9**.

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
Reward 400+100; unlock Three
```

## Mission3

Board **9×9**.

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

```text
Mission 3 — Cut the Courier
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by end Round2.
Reward 500+150; unlock Four
```

Clock:

```text
round0 opening -> player1
round1 move1 -> player2
round2 move2 -> player3
round3 move3 (<=12 total) -> player4, cannot extract
round4 move4 can reach (8,0) -> extraction defeat
round5 only if blocked/delayed -> deadline before another move
```

Tests must prove Player4, live move4 extraction, blocked-exit Round5 fallback, and player push-to-extraction immediate failure. Do not pin exact intermediate distance.

## Openings/enemies/planner

Move Mission1 opening hardcodes to authored rows without behavior change. Create shared `mission::enemies` mirroring `squad.rs`.

Protect Flanker scores distance to protected-target weapon band, Manhattan, open-neighbor count, y,x and prefers Gunner when legal. Courier scores Manhattan to extraction, open-neighbor count, y,x. Other Flanker uses normal attack-band fallback. No RNG/policy object.

## Authoring validation

No runtime validation framework. Mission tests assert 9×9 dimensions, rule targets/opening refs/factions, legal opening destinations; Mission3 additionally asserts legal extraction and Manhattan14.

## Mission IDs/campaign

Add `MissionId { One, Two, Three, Four }` once in Mission2 work. Final definitions One/Two/Three authored, Four handoff. Continue One→story, Two/Three→Upgrade, Four→handoff; Proceed authored→story. Save shape unchanged.

## Presentation

M2 HUD Round n/3 + Gunner HP. M3 HUD Round n/5 + extraction distance. Generic bonus/result copy. Extraction ring uses existing white ring material.

Extend existing glTF with scene10 `Flanker`, nodes49–55, mesh10/material10 `Flanker Magenta`; set scene count11 and Flanker→10. Keep root scale0.72. Do not add `unit_scale`, child under-ring, inverse-scale compensation. Existing buffer/accessors are reused.

## Required coverage

- Mission1 opening/order unchanged.
- completed_enemy_round phase semantics.
- M2 KO fail, immediate-clear win, Round3 survival win with attacker alive, bonus boundary.
- M3 escort-clear non-win, Courier KO win, Player4, live move4 extraction, blocked Round5 fallback, push-to-exit loss, bonus boundary.
- Flanker protect/Courier/fallback/tie-break and initiative constants.
- Authoring reference legality.
- glTF scene10/count11.
- M2 HUD n/3, M3 HUD n/5, generic result/reward copy.
- One→Two→Three→Four save/upgrades, 1200 base credits.

## Scope guardrails

No new dependency, objective/AI framework, neutral role, status system, mission select, branching, difficulty, runtime asset pipeline, save migration, new VN asset, or second PR. Keep sync.rs, app.rs, save/session implementation, Cargo files unchanged unless tests expose a concrete need.