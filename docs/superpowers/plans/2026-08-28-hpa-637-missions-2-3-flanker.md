# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius through Missions 2–3 with a Gunner-defense mission, a real Courier chase, and a distinct Flanker enemy while keeping one small typed mission/combat architecture.

**Architecture:** `BattleState` receives one closed `MissionRules` row. Mission modules author board/roster/openings/copy/rewards. A small shared enemy catalog mirrors `squad.rs`. Flanker is one explicit branch in the existing deterministic planner. Existing campaign/save/UI composition remains unchanged. Flanker gets one new scene in the existing checked-in glTF rather than runtime scale/marker compensation.

**Tech Stack:** Rust 2024, Bevy 0.19, serde/serde_json, headless Rust tests, checked-in glTF/VN assets.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Global Constraints

- One HPA-637 implementation PR.
- Domain stays Bevy-free; no new dependency/framework/scripting format/behavior tree/neutral faction/status system.
- Mission 2: protect Gunner; win at completed Round 3 or immediately when all attackers are gone.
- Mission 3: Courier `(0,6)` → extraction `(8,0)`, Move4, deadline Round5; player Round4 must exist; open route must actually extract; Round5 is blocked/stalled fallback.
- One optional-complete bit; bonuses affect credits only.
- Extend existing glTF with scene10; no new asset file/pipeline.
- Save shape unchanged; no migration path.
- Final gates: fmt, strict Clippy, all-target tests, release build.

## Task 1 — Objective rules and round boundary

- [ ] Add closed `PrimaryObjective`, `OptionalObjective`, `EnemyOpening`, `MissionRules` from spec.
- [ ] Store rules in `BattleState`; expose `rules()`; rename active `turnabout_complete` to `optional_complete`.
- [ ] Add `completed_enemy_round(round)` exactly as `EnemyPlanning && self.round >= round`; test Player phase false / EnemyPlanning true at same round.
- [ ] Protect match: target KO fail; no enemies win; otherwise completed Round3 win.
- [ ] Intercept match: target KO win; exact escape fail; completed Round5 fail; escort clear non-win.
- [ ] Keep Turnabout damage trigger special. Terminal half-HP/victory-by-round bonus uses one bit and emits bonus event immediately before MissionCompleted.
- [ ] Campaign reward checks only `optional_complete`.
- [ ] Run fmt/domain/all-target tests; commit `feat: add authored mission objective rules`.

## Task 2 — Openings, shared enemies, Flanker planner, initiative

- [ ] Strengthen Mission1 opening test before refactor: exact positions/order/intended occupants.
- [ ] Create `mission::enemies` factories:

```text
Rifleman HP9 Armor1 Move2 Acc72 Eva5
Striker HP12 Armor2 Move2 Acc78 Eva10
Artillery HP10 Armor1 Move1 Acc90 Eva0
Flanker HP8 Armor0 Move4 Acc82 Eva30
Skirmish Carbine range1-2 damage4 hit+5 crit10 EN0 no push/counter
```

- [ ] Replace Mission1 opening archetype/x-position logic with exact four `EnemyOpening` rows.
- [ ] Flanker tests: protect movement/target, Courier distance reduction, non-objective attack-band fallback, open-neighbor tie-break.
- [ ] Extract local attack-band helper; protect/Courier scoring per spec; no policy objects/RNG.
- [ ] Replace initiative position hack with Striker30/Flanker25/Rifleman20/Artillery10; pin values and Mission1 order.
- [ ] Run enemy/Mission1/all-target tests; commit `feat: add authored enemy openings and flanker`.

## Task 3 — Mission2 + authored references + MissionId One–Four once

- [ ] Pin 9×9 board, exact deployment/terrain/roster/opening from spec.
- [ ] Authoring tests validate protected Gunner, opening enemy/target IDs/factions, destinations legal, Gunner HP1 -> maxHP15.
- [ ] Definition: `Protect Gunner through the end of Round 3, or eliminate all attackers.` / Hold Fast / 400+100 / unlock Three.
- [ ] Immediate-clear test: last enemy KO in Round1 immediately wins.
- [ ] Survival test with durable Gunner: Round1→2 no result, Round2→3 no result, enemy alive, Round3 resolve wins.
- [ ] Gunner KO fail; half-HP bonus boundary/event order.
- [ ] Add `MissionId { One, Two, Three, Four }` now. One/Two authored; Three/Four handoff. Add number(); final routing One story / Two-Three upgrade / Four handoff; Proceed authored→story.
- [ ] Run Mission2/domain/all-target tests; commit `feat: add mission 2 gunner defense`.

## Task 4 — Mission3 extraction/deadline/push

- [ ] Pin 9×9 content, Courier Flanker `(0,6)`, extraction `(8,0)`, deadline5, Manhattan14, authored reference/extraction legality.
- [ ] Definition: `Intercept Courier before extraction or the end of Round 5.` / Swift Intercept / 500+150 / unlock Four.
- [ ] Focused outcomes: escort clear non-win; Courier KO with escort alive; Round2/3 bonus boundary; exact exit fail; Round5 deadline fail.
- [ ] Timing tests use durable players + Guard helper.
- [ ] Round4 test: remove escorts; resolve Rounds1–3 -> Player2/3/4, no result, Courier not exit. No exact intermediate distance assertion.
- [ ] Open route test: resolve Player4 -> fourth later move reaches `(8,0)` -> extraction defeat.
- [ ] Blocked exit test: player occupies `(8,0)`; resolve Player4 -> Player5/no result; resolve Player5 -> deadline fail before another move (Courier position unchanged).
- [ ] Push regression: Vanguard `(6,0)`, Courier `(7,0)`, `resolve_push` -> Courier `(8,0)` immediate fail.
- [ ] Author Three; Four handoff. Progress One-Two-Three no bonus -> Four +1200 credits; save/load Four+upgrades.
- [ ] Run Mission3/progression/persistence/all-target tests; commit `feat: add mission 3 courier interception`.

## Task 5 — Distinct Flanker glTF scene + objective UI

- [ ] Red JSON test: 11 scenes; scene10 Flanker nodes49–55; mesh10/material10 Flanker Magenta.
- [ ] Append scene10 with exact node transforms from spec; mesh10 reuses POSITION0/NORMAL1/indices2; material magenta `[0.78,0.08,0.46,1]`, metallic.25, roughness.62, emissive `[0.08,0,0.04]`; buffer/accessors unchanged.
- [ ] Set scene count11 and Flanker scene index10. Keep root scale0.72; no `unit_scale`, under-ring, inverse-scale math.
- [ ] HUD tests: M2 primary + Round1/3 + GunnerHP; M3 Round1/5 + cells-to-extraction; bonus state.
- [ ] Generic result/event/reward copy; extraction ring uses existing white ring material at rule escape.
- [ ] Genericize touched presentation root name.
- [ ] Run UI/campaign_flow/presentation/all-target tests; commit `feat: add flanker presentation and mission objective UI`.

## Task 6 — Campaign/restart/save integration

- [ ] Mission2 entry with GunnerHP1 -> ActiveMission Two, protect rules, round1, maxHP15.
- [ ] Mission3 entry/restart -> Three, CourierHP8, escape `(8,0)`, deadline5; definition-driven restart.
- [ ] Routing assertions: Continue One story, Two/Three upgrade, Four handoff; Proceed Two/Three story, Four handoff.
- [ ] Save continuity: M1 no bonus + VanguardHP1; M2 no bonus + GunnerHP1; M3 no bonus; reload -> Four, 800 credits, both upgrades.
- [ ] Run integration/all-target tests. Stage only changed tests by default; exact source file separately only if forced.
- [ ] Commit `test: cover campaign progression through mission 3`.

## Task 7 — Docs/manual/final evidence

- [ ] README: three-mission flow/rewards, M2 protect-or-clear, M3 extraction/Round5, Continue, distinct Flanker.
- [ ] CLAUDE.md: MissionRules, completed_enemy_round, shared enemies, Flanker planner/fallback, fixed initiative, glTF scene10, committed-intent invariant.
- [ ] Run fmt/strict Clippy/all-target/release.
- [ ] Manual M2: competing threats, reaction/Aegis, Gunner KO, immediate clear win, Round3 survival win, bonus.
- [ ] Manual M3: magenta Courier, extraction ring, Player4, open extraction, blocked Round5 fallback, Courier-only win, bonus.
- [ ] Manual save/Continue/upgrades and M4 handoff.
- [ ] Validation ledger: exact SHA/gate counts/named M2-M3 lifecycle+push tests/glTF evidence/manual/save verdict; no placeholders.
- [ ] Re-run gates; commit `docs: validate HPA-637 missions 2 and 3`.

## Final PR Gate

- [ ] One small HPA-637 PR; no framework/dependency/runtime pipeline.
- [ ] Mission1 regression green; one named round-boundary helper.
- [ ] M2: target KO fail, immediate clear win, real Round3 win with enemy alive, exact opening locks.
- [ ] M3: 14-step route, Player4, live fourth-move extraction, blocked Round5 fallback, push-to-exit loss.
- [ ] Courier KO with escort alive win; escort clear non-win.
- [ ] Flanker fallback + 30/25/20/10 initiative; no x-position Rifleman hack.
- [ ] Authored reference/extraction legality tests.
- [ ] glTF scene10/count11; no runtime scale workaround.
- [ ] M2 HUD n/3; M3 HUD n/5; generic bonus/result copy.
- [ ] One→Two→Three→Four, 1200 base credits, save/upgrades intact.
- [ ] Docs current; fmt/Clippy/tests/release green.

## Self-review

Every accepted review finding is a concrete implementation/test requirement. No placeholder or extra abstraction remains.