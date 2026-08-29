# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius through Missions 2–3 with a Gunner-defense mission, a real Courier chase, and a distinct Flanker enemy while keeping one small typed mission/combat architecture.

**Architecture:** `BattleState` receives one closed `MissionRules` row. Mission modules author board/roster/openings/copy/rewards. A small shared enemy catalog mirrors `squad.rs`. Flanker is one explicit branch in the existing deterministic planner. Existing campaign/save/UI composition remains unchanged. Flanker gets one new scene in the existing checked-in glTF rather than runtime scale/marker compensation.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, headless Rust tests, checked-in glTF/VN assets.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Global Constraints

- One HPA-637 implementation PR.
- Domain stays Bevy-free; no new dependency/framework/scripting format/behavior tree/neutral faction/status system.
- Mission2: protect Gunner; win at completed Round3 or immediately when no attackers remain.
- Mission3: Courier `(0,6)`, extraction `(8,0)`, Move4, deadline Round5; player Round4 must exist; open route must actually extract; Round5 is blocked/stalled fallback.
- One optional-complete bit; bonuses affect credits only.
- Extend existing glTF with scene10; no new asset file/pipeline.
- Save shape unchanged; no migration path.
- Final gates: fmt, strict Clippy, all-target tests, release build.

---

## Task 1 — Closed objectives and one round-boundary helper

**Files:** domain model/battle, campaign progression, Mission1 call sites, UI/test renames.

- [ ] Add `PrimaryObjective::{EliminateAllEnemies, ProtectThroughRound, InterceptBeforeEscape}`, `OptionalObjective::{Turnabout, ProtectTargetAtHalfHp, VictoryByRound}`, `EnemyOpening`, `MissionRules` exactly as specified.
- [ ] Rename `turnabout_complete` state/result fields to `optional_complete`; keep `BattleEvent::OptionalObjectiveCompleted`.
- [ ] Change `BattleState::new(..., rules, seed)`, store rules, expose `rules()`.
- [ ] Add and test:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

Test same numeric round in Player phase returns false.

- [ ] Implement protect outcome:

```rust
if target.is_knocked_out() { Some(false) }
else if !any_living_enemy || self.completed_enemy_round(round) { Some(true) }
else { None }
```

Test target KO fail, last-enemy KO immediate win, and focused Round3-boundary win.

- [ ] Implement intercept outcome: target KO win; exact escape fail; `completed_enemy_round(deadline)` fail; escort clear does not win.
- [ ] Keep Turnabout damage trigger special. At terminal victory, half-HP / victory-by-round can set the one bonus bit; newly-earned bonus event immediately precedes MissionCompleted.
- [ ] Campaign completion checks only `result.optional_complete` for optional reward.
- [ ] Run `cargo fmt --check`, domain battle tests, all-target tests.
- [ ] Commit `feat: add authored mission objective rules`.

---

## Task 2 — Authored openings, shared enemies, Flanker planner, initiative cleanup

**Files:** create `mission/enemies.rs`; modify mission mod/Mission1, domain model/enemy, exhaustive presentation matches.

- [ ] Before refactor, strengthen Mission1 opening test: exact positions, intent order, and intended occupants L Rifleman→Gunner, R Rifleman→Interceptor, Striker→Vanguard, Artillery→Vanguard.
- [ ] Create fixed enemy factories:

```text
Rifleman  HP9  Armor1 Move2 Acc72 Eva5  Service Rifle
Striker   HP12 Armor2 Move2 Acc78 Eva10 Shock Claw
Artillery HP10 Armor1 Move1 Acc90 Eva0  Siege Mortar
Flanker   HP8  Armor0 Move4 Acc82 Eva30 Skirmish Carbine
```

Skirmish Carbine: range1–2, Single, damage4, hit+5, crit10, EN0, no push/counter.

- [ ] Move Mission1 opening to `EnemyOpening[4]`; delete archetype/x-position opening movement + `opening_target`.
- [ ] Write Flanker planner tests: protect movement into Gunner range band; protect targeting Gunner; Courier reduces distance to `(8,0)`; non-objective Flanker uses normal attack-band fallback; open-neighbor tie-break.
- [ ] Extract local attack-band helper reused by Rifleman/Striker/fallback Flanker. Protect sort: band distance, Manhattan, more open neighbors, y,x. Courier sort: distance to escape, more open neighbors, y,x.
- [ ] Replace positional initiative hack with exact archetype values:

```text
Striker 30, Flanker 25, Rifleman 20, Artillery 10, other 0
```

Pin values and retain Mission1 order test.
- [ ] Run enemy/Mission1/all-target tests.
- [ ] Commit `feat: add authored enemy openings and flanker`.

---

## Task 3 — Mission2, authored-reference tests, MissionId One–Four once

**Files:** create `mission_two.rs`; modify mission mod, campaign UI, enemy tests.

- [ ] Pin **9×9** board and exact content:

```text
Players V(3,7), G(4,6), I(5,7)
Blocking (3,3),(5,3),(2,6),(6,6)
Hazards (1,5),(7,5)
Explosive (6,4) HP4
Rifleman21 start(2,2) -> (2,4), Vanguard
Striker22 start(4,3) -> (4,5), Gunner
Artillery23 (4,0) -> Gunner
Flanker24 start(8,4) -> (5,5), Interceptor
```

- [ ] Authoring tests: protected Gunner exists/player; every opening enemy/target exists/correct faction; destinations in-bounds/non-blocking; Gunner HP1 projects maxHP15.
- [ ] Definition:

```text
Mission 2 — Hold Relay Nine
Primary: Protect Gunner through the end of Round 3, or eliminate all attackers.
Bonus: Hold Fast: finish with Gunner at or above 50% HP.
400 + 100, unlock Three
```

- [ ] Immediate-clear lifecycle: begin Round1, KO all enemies, final KO immediately yields victory/MissionCompleted.
- [ ] Survival lifecycle: build durable Gunner HP3/Armor3; Guard all living players; resolve Round1→player2 no result, Round2→player3 no result, assert enemy still alive, resolve Round3→victory.
- [ ] Gunner KO fail; half-HP bonus boundary; bonus event ordering.
- [ ] Add `MissionId { One, Two, Three, Four }` once. This task authors One/Two; Three/Four handoffs. Add `number()`.
- [ ] Final routing already: Continue One→story, Two/Three→Upgrade, Four→handoff. Proceed authored→story, unauthored→handoff.
- [ ] Run Mission2/domain/all-target tests.
- [ ] Commit `feat: add mission 2 gunner defense`.

---

## Task 4 — Mission3: live extraction, Round5 backstop, push-to-exit regression

**Files:** create `mission_three.rs`; modify mission dispatch/campaign UI/progression tests/persistence tests.

- [ ] Pin **9×9** content:

```text
Players V(4,7), G(3,8), I(5,8)
Blocking (4,3),(4,4),(4,5)
Hazard (2,5)
Explosive (6,3) HP4
Extraction (8,0)
Courier31 Flanker start(0,6)
Rifleman32 start(3,2) -> (3,4), Vanguard
Striker33 start(6,6) -> (5,7), Interceptor
```

- [ ] Authoring tests: Courier exists/Flanker; opening references valid; extraction in-bounds/non-blocking/non-hazard/no live explosive; Manhattan `(0,6)->(8,0)==14`.
- [ ] Rules/copy:

```text
Primary: Intercept Courier before extraction or the end of Round 5.
Bonus: Swift Intercept: defeat Courier by end Round2.
500 + 150, unlock Four
```

- [ ] Focused outcomes: escort clear non-win; Courier KO with escort alive win; Round2 bonus true / Round3 false; Courier already at exit fail; EnemyPlanning Round5 deadline fail.
- [ ] Timing tests use durable HP3/Armor3 player squad + Guard helper.
- [ ] **Round4 test:** remove escorts; resolve Rounds1–3; assert Player rounds2/3/4, result none; Courier not at exit. Do not assert exact remaining distance.
- [ ] **Open extraction test:** from equivalent fixture resolve player Round4; fourth later move reaches `(8,0)` and mission fails from extraction.
- [ ] **Blocked-exit test:** place durable Interceptor at `(8,0)`; after player Round4 resolve assert Player Round5/no result/Courier not exit; resolve Round5 and assert deadline fail before another Courier move (position unchanged).
- [ ] **Push test:** direct-place Vanguard `(6,0)`, Courier `(7,0)`, call `resolve_push`; Courier becomes `(8,0)` and mission immediately fails.
- [ ] Make Three authored; Four remains handoff. No enum churn.
- [ ] Progression One/Two/Three no bonus -> Four +1200 credits. Bonus credits only. Save/load Four + upgrades exactly.
- [ ] Run Mission3/progression/persistence/all-target tests.
- [ ] Commit `feat: add mission 3 courier interception`.

---

## Task 5 — Distinct Flanker scene + objective-generic presentation

**Files:** existing glTF, presentation assets/battlefield/ui/campaign UI, relevant tests.

- [ ] Red JSON test parses existing glTF: 11 scenes; scene10 `Flanker`; nodes49–55; mesh10/material10 `Flanker Magenta`.
- [ ] Append scene10 with mesh10 nodes:

```text
49 Left Leg      [-0.16,0.18, 0.00] [0.12,0.36,0.16]
50 Right Leg     [ 0.16,0.18, 0.00] [0.12,0.36,0.16]
51 Torso         [ 0.00,0.62, 0.00] [0.36,0.42,0.28]
52 Head          [ 0.00,0.95, 0.00] [0.20,0.20,0.20]
53 Left Fin      [-0.42,0.67,-0.10] [0.42,0.08,0.28]
54 Right Fin     [ 0.42,0.67,-0.10] [0.42,0.08,0.28]
55 Rear Thruster [ 0.00,0.52,-0.34] [0.20,0.16,0.34]
```

Mesh uses existing POSITION0/NORMAL1/indices2. Material: magenta base `[0.78,0.08,0.46,1]`, metallic.25, roughness.62, emissive `[0.08,0,0.04]`. Buffer/accessors unchanged.
- [ ] Set scene count11 and Flanker scene index10. Keep root scale0.72; **no `unit_scale`, under-ring, inverse-scale math**.
- [ ] HUD tests: M2 primary + Round1/3 + GunnerHP; M3 Round1/5 + distance; bonus state.
- [ ] Result/event/reward copy generic: `BONUS OBJECTIVE COMPLETE`, `Bonus +...`.
- [ ] Spawn extraction ring at rule escape using existing white ring material.
- [ ] Genericize touched presentation root name.
- [ ] Run UI/campaign_flow/presentation/all-target tests.
- [ ] Commit `feat: add flanker presentation and mission objective UI`.

---

## Task 6 — Campaign/restart/save integration

- [ ] Mission2 entry with GunnerHP1 -> ActiveMission Two, protect rule, round1, maxHP15.
- [ ] Mission3 entry/restart -> ActiveMission Three, CourierHP8, escape `(8,0)`, deadline5; restart remains definition-driven.
- [ ] Routing:

```text
Continue One -> Story
Continue Two/Three -> Upgrade
Continue Four -> Handoff
Proceed Two/Three -> Story
Proceed Four -> Handoff
```

- [ ] Save continuity: M1 no bonus + VanguardHP1 purchase; M2 no bonus + GunnerHP1 purchase; M3 no bonus; reload -> Four, 800 credits, both upgrades.
- [ ] Run campaign_flow, campaign_persistence, presentation_app, all-target tests.
- [ ] Stage only test files by default; stage exact source path separately only if tests force a source fix.
- [ ] Commit `test: cover campaign progression through mission 3`.

---

## Task 7 — Docs and final validation

- [ ] README: three-mission flow, rewards, M2 protect-or-clear, M3 extraction/Round5, Continue, distinct Flanker, controls.
- [ ] CLAUDE.md: MissionRules, `completed_enemy_round`, shared enemies, Flanker planner/fallback, fixed initiative, glTF scene10, committed-intent invariant.
- [ ] Run fmt, strict Clippy, all-target tests, release build.
- [ ] Manual M2: competing threats; reaction/Aegis usefulness; Gunner KO; immediate clear win; Round3 survival win; bonus states.
- [ ] Manual M3: magenta Courier; extraction ring; player Round4; open extraction after Round4; blocked exit Round5 backstop; Courier-only win; bonus states.
- [ ] Manual save/Continue/upgrades after M1/M2 and M4 handoff after M3.
- [ ] Write validation ledger with exact SHA, gate/test counts, named lifecycle/push tests, glTF evidence, manual observations, save continuity, short-session verdict. No placeholders.
- [ ] Re-run all gates; commit `docs: validate HPA-637 missions 2 and 3`.

---

## Final PR Gate

- [ ] One HPA-637 PR; no new framework/dependency/runtime asset pipeline.
- [ ] Mission1 opening regression green.
- [ ] One named objective round-boundary predicate.
- [ ] M2 target-KO fail, immediate clear win, real Round3 win with enemy alive, exact competing openings.
- [ ] M3 14-step route, player Round4, live fourth-move extraction, blocked Round5 backstop, push-to-exit loss.
- [ ] Courier KO with escort alive win; escort clear non-win.
- [ ] Flanker fallback works; initiative 30/25/20/10; no positional Rifleman hack.
- [ ] Authored reference/extraction legality tests.
- [ ] Flanker scene10 / scene count11; no runtime scale workaround.
- [ ] M2 HUD n/3; M3 HUD n/5; generic result/bonus copy.
- [ ] One→Two→Three→Four flow; base 1200 credits.
- [ ] Docs/validation current; fmt/Clippy/tests/release green.

## Self-review

All accepted review findings are implementation requirements rather than deferred playtest choices. No placeholder or extra abstraction remains.