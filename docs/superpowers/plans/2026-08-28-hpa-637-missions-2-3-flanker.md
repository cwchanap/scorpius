# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend Scorpius through Missions 2–3 with a Gunner-defense mission, a real Courier chase, and a distinct Flanker enemy while keeping one small typed mission/combat architecture.

**Architecture:** `BattleState` receives one closed `MissionRules` row. Mission modules author board/roster/openings/copy/rewards. A shared enemy catalog mirrors `squad.rs`. Flanker is one explicit branch in the deterministic planner. Existing campaign/save/UI composition remains. Flanker gets one new scene in the existing checked-in glTF rather than runtime scale/marker compensation.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Constraints

- One HPA-637 PR, one Rust 2024 / Bevy 0.19 crate, no new dependency/framework/neutral faction/scripting/save layer.
- M2: Gunner survives completed Round3 OR all attackers cleared.
- M3: Courier `(0,6)` → `(8,0)`, Move4, deadline5; player Round4 exists; open move4 extracts; Round5 is blocked/stalled fallback.
- One bonus bit. Save shape unchanged.
- Existing glTF gains scene10. No new asset file/pipeline; no `unit_scale`/under-ring workaround.

## Task 1 — Objective rules

- [ ] Add closed primary/optional objective enums, `EnemyOpening`, `MissionRules`; store on BattleState.
- [ ] Rename active Turnabout-specific progress/result bit to `optional_complete`.
- [ ] Add/test one helper:

```rust
fn completed_enemy_round(&self, round: u16) -> bool {
    self.phase == BattlePhase::EnemyPlanning && self.round >= round
}
```

- [ ] Protect: Gunner KO fail; no attackers win; otherwise completed Round3 win.
- [ ] Intercept: Courier KO win; exact exit fail; completed Round5 fail; escort clear non-win.
- [ ] Keep Turnabout event trigger special; terminal bonuses use one bit and correct event ordering.
- [ ] Campaign reward uses only optional_complete.
- [ ] fmt/domain/all-target tests; commit objective rules.

## Task 2 — Openings/enemies/Flanker/initiative

- [ ] Strengthen M1 opening positions/order/intended-occupant regression before refactor.
- [ ] Create enemy factories: Rifleman 9/1/2/72/5, Striker 12/2/2/78/10, Artillery 10/1/1/90/0, Flanker 8/0/4/82/30; Skirmish Carbine range1–2 damage4 hit+5 crit10 EN0 no push/counter.
- [ ] Move M1 opening to four authored `EnemyOpening` rows; remove archetype/x-position opening hardcodes.
- [ ] Flanker tests: protected-target movement/intent, Courier distance reduction, non-objective attack-band fallback, open-neighbor tie-break.
- [ ] Extract local attack-band helper; no policy objects/RNG.
- [ ] Initiative becomes Striker30 / Flanker25 / Rifleman20 / Artillery10; remove positional Rifleman hack; keep M1 order regression.
- [ ] enemy/M1/all-target tests; commit.

## Task 3 — Mission2 + MissionId growth once

- [ ] Pin 9×9 M2 board, deployment, terrain, enemy IDs21–24/openings from spec.
- [ ] Authoring tests validate protected target, opening refs/factions/destinations; Gunner HP1 -> maxHP15.
- [ ] Copy: `Protect Gunner through the end of Round 3, or eliminate all attackers.` / Hold Fast / 400+100 / unlock3.
- [ ] Immediate-clear test: last enemy KO Round1 wins immediately.
- [ ] Durable survival test: Round1→2 no result, Round2→3 no result, enemy alive, Round3 resolution wins.
- [ ] Gunner KO and half-HP bonus boundaries.
- [ ] Add `MissionId { One, Two, Three, Four }` once. Task3 authors One/Two; Three/Four handoff. Final routing One story / Two-Three upgrade / Four handoff; Proceed authored→story.
- [ ] Mission2/domain/all-target tests; commit.

## Task 4 — Mission3 extraction/deadline/push

- [ ] Pin 9×9 board; Courier31 Flanker `(0,6)`; extraction `(8,0)`; deadline5; Manhattan14; validate authored refs/escape legality.
- [ ] Copy: `Intercept Courier before extraction or the end of Round 5.` / Swift Intercept / 500+150 / unlock4.
- [ ] Outcomes: escort clear non-win; Courier KO with escort alive; Round2/3 bonus boundary; exact exit fail; Round5 deadline fail.
- [ ] Durable timing helper for player squad.
- [ ] Round4 test: after three later moves Player4/no result/Courier not exit; no exact distance assertion.
- [ ] Open-route test: resolve Player4 -> move4 reaches exit -> extraction defeat.
- [ ] Blocked-exit test: occupy exit -> Player5/no result, then Round5 deadline before another move (position unchanged).
- [ ] Push regression: Vanguard `(6,0)`, Courier `(7,0)`, resolve_push -> exit/immediate fail.
- [ ] Author Three; Four handoff. One/Two/Three no bonus -> Four +1200; save/load Four+upgrades.
- [ ] Mission3/progression/persistence/all-target tests; commit.

## Task 5 — Flanker scene + objective UI

- [ ] JSON red test: glTF 11 scenes; scene10 Flanker nodes49–55; mesh10/material10 `Flanker Magenta`.
- [ ] Add scene10 using existing cuboid accessors/buffer. Node transforms and magenta material are pinned in spec.
- [ ] `MISSION_ONE_SCENE_COUNT=11`, Flanker scene index10, root scale0.72. No unit_scale/under-ring/inverse compensation.
- [ ] HUD tests: M2 Round n/3 + Gunner HP; M3 Round n/5 + extraction distance.
- [ ] Generic result/event/reward copy (`BONUS OBJECTIVE COMPLETE`, `Bonus +...`).
- [ ] Extraction ring uses existing white ring material at rule escape.
- [ ] UI/campaign_flow/presentation/all-target tests; commit.

## Task 6 — Campaign/restart/save integration

- [ ] M2 entry with GunnerHP1 -> ActiveMission2/protect rules/round1/maxHP15.
- [ ] M3 entry/restart -> ActiveMission3/CourierHP8/escape(8,0)/deadline5; definition-driven restart.
- [ ] Continue One story; Two/Three upgrade; Four handoff. Proceed Two/Three story; Four handoff.
- [ ] Save continuity: M1 no bonus + VanguardHP1; M2 no bonus + GunnerHP1; M3 no bonus; reload Four/800 credits/both upgrades.
- [ ] Run integration/all-target tests. Stage only files actually changed; commit.

## Task 7 — Docs/validation

- [ ] README current three-mission flow/rewards, M2 protect-or-clear, M3 extraction/Round5, distinct Flanker.
- [ ] CLAUDE.md current rules/helper/shared enemies/initiative/glTF scene/intent invariant.
- [ ] Run fmt, strict Clippy, all-target tests, release build.
- [ ] Manual M2: competing threats, immediate clear win, full Round3 win, Gunner KO, bonus.
- [ ] Manual M3: magenta Courier, extraction ring, Player4, open extraction, blocked Round5 fallback, Courier-only win, bonus.
- [ ] Save/Continue/upgrades/M4 handoff.
- [ ] Validation ledger with exact SHA/gate counts/named lifecycle+push tests/glTF evidence/manual/save verdict, no placeholders.
- [ ] Rerun gates and commit validation.

## Final Gate

- [ ] One small HPA-637 PR; no framework/dependency/runtime pipeline.
- [ ] M1 opening/order unchanged; one named boundary helper.
- [ ] M2 KO fail + immediate clear win + real Round3 win with attacker alive.
- [ ] M3 14-step route + Player4 + live move4 extraction + blocked Round5 fallback + push-to-exit fail.
- [ ] Courier/escort semantics correct.
- [ ] Flanker fallback and 30/25/20/10 initiative; no x-position hack.
- [ ] Authored refs/extraction legal.
- [ ] glTF scene10/count11; no runtime scale workaround.
- [ ] M2 HUD n/3; M3 HUD n/5; generic bonus/result copy.
- [ ] One→Two→Three→Four, 1200 base credits, save/upgrades intact.
- [ ] Docs current; fmt/Clippy/tests/release green.

## Self-review

Every accepted review finding is a concrete implementation/test requirement; no placeholder or extra abstraction remains.