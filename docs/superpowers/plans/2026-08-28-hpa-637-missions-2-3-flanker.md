# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. One ticket = one PR.

**Goal:** Ship Mission2 defense + Mission3 Courier chase + Flanker with the smallest typed extension of existing battle/mission/campaign seams.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Task 1 — Closed objectives

- [ ] Add closed primary/optional objective enums, `EnemyOpening`, `MissionRules`; store rules on BattleState.
- [ ] Rename active Turnabout-specific progress/result bit to `optional_complete`.
- [ ] Add/test single boundary helper `completed_enemy_round(n) = EnemyPlanning && round >= n`.
- [ ] M2 protect: target KO fail; no attackers win immediately; otherwise completed Round3 win.
- [ ] M3 intercept: Courier KO win; exact exit fail; completed Round5 fail; escort clear non-win.
- [ ] One bonus bit; terminal bonus event precedes MissionCompleted; campaign optional reward checks only bit.
- [ ] fmt/domain/all-target tests; commit.

## Task 2 — Openings/enemies/Flanker/initiative

- [ ] Strengthen Mission1 exact opening/order/intended-occupant regression.
- [ ] Create shared Rifleman/Striker/Artillery/Flanker factories; Flanker HP8 Armor0 Move4 Acc82 Eva30 + Skirmish Carbine range1–2 damage4 hit+5 crit10 EN0.
- [ ] Replace Mission1 opening hardcodes with four authored rows.
- [ ] Flanker planner tests: protect movement/target; Courier distance; fallback attack-band; open-neighbor tie-break.
- [ ] Reuse local attack-band helper; no policy objects/RNG.
- [ ] Initiative Striker30/Flanker25/Rifleman20/Artillery10; remove x-position hack; retain M1 order regression.
- [ ] enemy/M1/all-target tests; commit.

## Task 3 — Mission2 + MissionId growth once

- [ ] Pin 9×9 board: players V(3,7) G(4,6) I(5,7), blockers `(3,3),(5,3),(2,6),(6,6)`, hazards `(1,5),(7,5)`, explosive `(6,4)` HP4.
- [ ] Pin openings: Rifleman21→Vanguard, Striker22→Gunner, Artillery23→Gunner, Flanker24→Interceptor.
- [ ] Authoring tests validate target/opening IDs/factions/legal destinations; Gunner HP1 -> maxHP15.
- [ ] Primary `Protect Gunner through the end of Round 3, or eliminate all attackers.` Bonus Hold Fast. Reward400+100, unlock3.
- [ ] Immediate-clear lifecycle test.
- [ ] Durable full Round3 lifecycle test with attacker alive.
- [ ] Gunner KO / half-HP bonus tests.
- [ ] Add `MissionId { One, Two, Three, Four }` once; final routing One story, Two/Three Upgrade, Four handoff; Proceed authored→story.
- [ ] tests; commit.

## Task 4 — Mission3 extraction/deadline/push

- [ ] Pin 9×9 board: players V(4,7) G(3,8) I(5,8), blockers `(4,3),(4,4),(4,5)`, hazard `(2,5)`, explosive `(6,3)` HP4, exit `(8,0)`.
- [ ] Courier31 Flanker `(0,6)`, Rifleman32→Vanguard, Striker33→Interceptor; deadline5; Manhattan14; authored ref/exit legality tests.
- [ ] Primary `Intercept Courier before extraction or the end of Round 5.` Bonus Swift Intercept. Reward500+150, unlock4.
- [ ] Escort-clear non-win / Courier-KO win / bonus boundary / exact-exit fail / Round5 fail.
- [ ] Durable timing tests: after three later moves Player4 exists and Courier not exit; open fourth move after Player4 extracts/fails; blocked exit reaches Player5 then deadline before another move.
- [ ] Push-to-exit regression using `resolve_push`.
- [ ] Author Three; progression to Four=1200 base credits; save/load Four+upgrades.
- [ ] tests; commit.

## Task 5 — Flanker scene + objective presentation

- [ ] JSON test: existing glTF has 11 scenes after change, scene10 `Flanker`, nodes49–55, mesh/material10 `Flanker Magenta`.
- [ ] Append scene10 using existing buffer/accessors; magenta material and slim node transforms from spec.
- [ ] `MISSION_ONE_SCENE_COUNT=11`; `scene_index(Flanker)=10`; root scale0.72. No `unit_scale`, under-ring, inverse compensation.
- [ ] M2 HUD Round n/3 + GunnerHP; M3 HUD Round n/5 + distance.
- [ ] Generic bonus/result/event/reward copy; extraction white ring at rule escape.
- [ ] tests; commit.

## Task 6 — Campaign/restart/save integration

- [ ] M2 entry with upgrades; M3 entry/restart with escape/deadline5; definition-driven restart.
- [ ] Routing: Continue One story, Two/Three Upgrade, Four handoff; Proceed Two/Three story, Four handoff.
- [ ] Save continuity after M1/M2 purchases + M3 no bonus -> Four, 800 credits, upgrades retained.
- [ ] Run integration/all-target tests; stage only files actually changed; commit.

## Task 7 — Docs/final validation

- [ ] README/CLAUDE current.
- [ ] fmt/strict Clippy/all-target/release.
- [ ] Manual M2: competing threats, immediate clear, Round3 survival, KO fail, bonus.
- [ ] Manual M3: magenta Courier, exit ring, Player4, open extraction, blocked Round5 fallback, Courier-only win, bonus.
- [ ] Save/Continue/upgrades/M4 handoff.
- [ ] Validation ledger with exact SHA, gate counts, lifecycle/push/glTF evidence, manual results; no placeholders.
- [ ] rerun gates; commit.

## Final Gate

- [ ] M1 regression green; one round-boundary helper.
- [ ] M2 immediate clear + Round3 path both correct.
- [ ] M3 Player4 + live extraction + Round5 fallback + push loss correct.
- [ ] Flanker fallback + 30/25/20/10 initiative; no x-position hack.
- [ ] Authoring legality tests; glTF scene10/count11; no runtime scale workaround.
- [ ] M2 HUD n/3, M3 HUD n/5, generic bonus/result copy.
- [ ] One→Two→Three→Four, 1200 base credits, save/upgrades intact.
- [ ] No new framework/dependency/runtime pipeline; docs and all gates green.