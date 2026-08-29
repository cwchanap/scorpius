# HPA-637 Missions 2–3 and Flanker Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or executing-plans. One HPA-637 ticket = one PR.

**Goal:** Ship Mission2 defense + Mission3 Courier chase + Flanker with the smallest typed extension of current battle/mission/campaign seams.

**Spec:** `docs/superpowers/specs/2026-08-28-hpa-637-missions-2-3-flanker-design.md`

## Constraints

- No new dependency/framework/neutral faction/scripting/save layer.
- M2 wins completed Round3 or earlier when all attackers gone.
- M3 `(0,6)`→`(8,0)`, Move4, deadline5; Player4 exists, move4 can extract, Round5 fallback only.
- One optional bit; unchanged save shape.
- Existing glTF gains scene10; no new asset file/pipeline or runtime scale workaround.

## Task 1 — Objective rules

- [ ] Add closed PrimaryObjective/OptionalObjective/EnemyOpening/MissionRules; store rules.
- [ ] Rename active progress/result to `optional_complete`.
- [ ] Add/test `completed_enemy_round(n) = EnemyPlanning && round>=n`.
- [ ] M2: KO fail, no attackers win, otherwise completed Round3 win.
- [ ] M3: Courier KO win, exit fail, completed Round5 fail, escort clear non-win.
- [ ] Keep Turnabout trigger special; terminal bonus event ordering; campaign optional reward generic.
- [ ] Run fmt/domain/all-target; commit.

## Task 2 — Openings/enemies/Flanker

- [ ] Strengthen M1 opening/order/intended-target regression.
- [ ] Shared enemy factories; Flanker HP8 Armor0 Move4 Acc82 Eva30, carbine range1–2 damage4 hit+5 crit10 EN0.
- [ ] Move M1 opening hardcodes to authored rows.
- [ ] Flanker protect/Courier/fallback/tie-break tests; local attack-band helper; no policy/RNG.
- [ ] Initiative Striker30/Flanker25/Rifleman20/Artillery10; remove x-position hack; retain M1 order regression.
- [ ] tests; commit.

## Task 3 — Mission2 + IDs once

- [ ] Pin M2 9×9 board/deployment/terrain/IDs/openings from spec; validate refs/factions/legal destinations; Gunner HP1→15.
- [ ] Primary protect through Round3 OR clear attackers; Hold Fast; 400+100; unlock3.
- [ ] Immediate-clear win test.
- [ ] Durable full Round3 win test with attacker alive.
- [ ] Gunner KO / half-HP bonus tests.
- [ ] Add MissionId One/Two/Three/Four once; final routing One story, Two/Three Upgrade, Four handoff; Proceed authored→story.
- [ ] tests; commit.

## Task 4 — Mission3 extraction/deadline/push

- [ ] Pin 9×9 board, Courier `(0,6)`, exit `(8,0)`, deadline5, Manhattan14; validate refs/exit legality.
- [ ] Primary intercept before extraction/end Round5; Swift Intercept; 500+150; unlock4.
- [ ] Escort clear non-win / Courier KO win / bonus boundary / exact-exit fail / Round5 fail.
- [ ] Durable timing helper.
- [ ] After three later moves Player4/no result/Courier not exit; no exact distance assertion.
- [ ] Fourth later move after Player4 reaches exit -> extraction defeat.
- [ ] Occupied exit -> Player5/no result; resolve Player5 -> deadline before another move.
- [ ] Push Vanguard `(6,0)` / Courier `(7,0)` -> exit immediate fail.
- [ ] Author Three; progress One-Two-Three -> Four+1200; save/load Four+upgrades.
- [ ] tests; commit.

## Task 5 — Flanker scene + UI

- [ ] JSON red test for glTF scene10/nodes49–55/mesh+material10 Flanker Magenta.
- [ ] Append scene using existing buffer/accessors; set scene count11, Flanker index10, root scale0.72. No unit_scale/under-ring.
- [ ] M2 HUD n/3 + GunnerHP; M3 HUD n/5 + distance.
- [ ] Generic bonus/result/event/reward copy; extraction white ring.
- [ ] tests; commit.

## Task 6 — Integration

- [ ] M2/M3 entry/restart definition-driven with upgrades/rules.
- [ ] Continue/Proceed routing One–Four.
- [ ] Save continuity through M3 -> Four, 800 after two 200 purchases.
- [ ] integration/all-target tests; stage only changed files; commit.

## Task 7 — Docs/validation

- [ ] README/CLAUDE current.
- [ ] fmt/strict Clippy/all-target/release.
- [ ] Manual M2: competing threats, clear win, Round3 win, KO fail, bonus.
- [ ] Manual M3: magenta Courier, ring, Player4, open extraction, blocked Round5 fallback, Courier-only win, bonus.
- [ ] save/Continue/upgrades/M4 handoff.
- [ ] validation ledger with exact SHA/test counts/lifecycle+push/glTF evidence/manual results; no placeholders.
- [ ] rerun gates; commit.

## Final Gate

- [ ] One small PR; no framework/dependency/runtime pipeline.
- [ ] M1 regression; one boundary helper.
- [ ] M2 immediate-clear + Round3 paths both correct.
- [ ] M3 Player4 + live extraction + Round5 fallback + push loss.
- [ ] Flanker fallback + 30/25/20/10 initiative; no x-position hack.
- [ ] Authoring legality; glTF scene10/count11; no scale workaround.
- [ ] M2 HUD n/3; M3 n/5; generic copy.
- [ ] One→Two→Three→Four; 1200 base credits; save/upgrades intact.
- [ ] docs/gates green.