# HPA-480 — Complete native UI visual parity

**Date:** 2026-09-05 (America/Vancouver)
**Issue:** [HPA-480](https://linear.app/cwchanap/issue/HPA-480)
**Branch:** `hpa-480-ui-visual-parity`
**Status:** Design for implementation; native UI parity is not yet implemented or verified.
**Baseline:** `d981682840eb9147ba9eb7f7c56b2ceae88a3aed` on `main`.
**Plan:** [Implementation plan](../plans/2026-09-05-hpa-480-ui-visual-parity.md)
**Reference record:** [Source identity and capture inventory](../../references/hpa-480/reference-manifest.json)

## 1. Delivery decision

Deliver the complete previously scoped visual overhaul as **one ticket, one branch, one PR**. The initial draft contains this design and its implementation plan. Continue implementation, tests, reference extensions, visual fixes, and acceptance evidence on this same branch and PR. Do not merge the planning documents first and open an implementation PR. Do not turn plan phases into sub-issues, a separate closeout ticket, or dependent PRs.

Replace the presentation of all eight screen families: Title, Story, Briefing, Battle, Result, Aftermath, Hangar, and Ending. Include the missing interactions needed to make the design work with the existing seven-mission campaign. Completion means full visual parity plus functioning gameplay, not merely adopting the palette.

This supersedes the earlier three-ticket delivery proposal. It also supersedes the old angled battlefield presentation constraint: the supplied design uses a flat tactical grid and geometric tokens. Keep Bevy; do not retain a selectable old/new renderer or introduce a WebView.

### Source precedence

The supplied `Scorpius UI (offline).html` determines appearance and displayed interaction affordances. The prior `scorpius-ui-scope.md` determines the agreed scope, except its three-ticket section is replaced by this delivery decision. Existing Rust domain/campaign rules determine live values and game outcomes. New decisions below resolve implementation details; they are not claims that the prototype already implements them.

The HTML's simplified JavaScript simulation, hardcoded credits, prototype screen rail, and forced result previews are not game rules. A discrepancy in sample data does not justify changing combat rules or hardcoding production UI. Later-mission states without original designs require explicitly identified same-style extensions.

## 2. Verified codebase and reuse

The current game already has Missions 1–7, three fixed player mechs, six regular enemies, two bosses, committed intents, reactions, environmental interactions, pilot skills, rewards, upgrades, saves, and the completed-campaign route. HPA-632/635/637/523/524/386 are complete. This task is a presentation replacement, not another campaign implementation.

| Existing seam | Use in this change |
| --- | --- |
| `src/app.rs`: `GameScreen`, enter/exit systems, `enter_battle`, `teardown_battle_screen` | Keep the screen flow; replace renderer wiring and cleanup. Result stays an overlay within Battle. |
| `src/domain/battle.rs`, `combat.rs`, `enemy.rs`, `environment.rs` | Keep authoritative rules, queries, previews, event generation, and RNG order. |
| `src/mission/mod.rs`, mission definitions and squad/enemy factories | Read current mission metadata and stable IDs; do not author game rules in UI files. |
| `src/campaign/session.rs`, `progression.rs`, `save.rs` | Reuse New Game, Continue, completion, purchases, and persistence. |
| `src/presentation/campaign_ui.rs` | Reuse `apply_campaign_action`, `dialogue_snapshot`, `DialogueCursor`, `campaign_destination`, and receipt semantics; replace layouts. |
| `src/presentation/interaction.rs` | Extend `route_cell_click`, `execute_command`, keyboard/observer adapters, and transient reset. |
| `src/presentation/ui.rs`: `HudSnapshot` | Extend typed view data instead of parsing display strings or maintaining a second game state. |
| `src/presentation/playback.rs`, `sync.rs` | Keep ordered event consumption/input locks; replace mesh/transform effects with token/grid effects. |
| `tests/presentation_app.rs`, `campaign_flow.rs`, `campaign_model.rs`, `campaign_persistence.rs` | Preserve behavioral coverage while adapting renderer-specific assertions. |

The old `MissionAssets` loads 15 glTF scenes, and `AssetLoadStatus` gates battle input on those scenes. `playback.rs` directly depends on those assets, `Camera3d`, and world transforms. Removing only visible meshes is insufficient: replace those load gates, effects, and schedule dependencies together. Historical glTF scene-count tests may be replaced with equivalent current visual-catalog coverage; gameplay tests must remain.

The native game was not built or run during this planning change. Repository contents were inspected through GitHub. A direct local clone was unavailable because the container could not resolve the GitHub host.

## 3. Fixed constraints

- One ticket, one branch, one PR; implement and accept on this existing draft PR.
- Rust 2024, Bevy 0.19, one application crate, committed Cargo.lock.
- Native Bevy presentation; no second UI runtime, physics engine, networking, or generic ability/UI framework.
- `src/domain/` remains Bevy-free; `BattleRuntime` and `CampaignRuntime` remain authoritative.
- One 1920 × 1080 logical canvas; uniform scale-to-fit with letterboxing, not responsive mobile reflow.
- Preserve combat balance, authored missions, committed intents, pilot restrictions, RNG order, and current save semantics.
- Full native visual parity is the final gate; unit tests alone cannot satisfy it.
- Existing normal tests remain headless; windowed visual capture is a separate opt-in command.
- No save migration or backward-compatibility layer, new save slots, checkpoints, undo, new missions, inventory, or settings screen.

## 4. Visual contract and assets

### Geometry and styling

Use source measurements rather than approximate recreation. At the logical canvas size, the battle uses 22 px outer padding, a 78 px header, 14 px major gaps, 352 px sidebars, and a 912 × 912 board. Its nine 96 px cells have 6 px gaps; each token is 86 × 86 with a 5 px inset. The contextual menu is 236 px wide and clamps inside the board. Preserve the source's surrounding flex spacing rather than stretching the board to fill the central column.

Interface type is Chakra Petch; technical/numeric type is IBM Plex Mono. Preserve the source's weights, letter spacing, line heights, alignment and wrapping. Main colors include cyan `#3ec7db`, background `#05080f`, panel `#0a1420`, red `#ff6b5c`, amber `#ffd175`, and green `#7fdc9a`. The source supplies the remaining alpha, gradient, stroke and glow values.

Match image crops, frame corners, icon paths, pips, scrims, shadows, hazard stripes, selection rings and pulses. Do not replace vector icons with emoji or artwork with generic placeholders. Do not add a runtime SVG/HTML framework: static decorative images can be exported at adequate resolution; dynamic bars, text, highlights and indicators remain native components. Verify title typography and one complex battle card early, rather than discovering approximation problems at closeout.

Fit scale is `min(window_logical_width / 1920, window_logical_height / 1080)`. Center the canvas. Rendering, hit targets, token/menu anchors and screenshot cropping share one transform. Handle the window's physical/logical DPI distinction once; never multiply pointer coordinates by DPI twice. Letterbox space is not a playable cell.

### Art inventory

Reuse `assets/vn/control_alert.png`, `control_neutral.png`, `vanguard_neutral.png`, and `relay_nine_bg.png`. The prior source review found matching embedded artwork. Import the five missing supplied images: title key art, briefing illustration, and Vanguard/Gunner/Interceptor mech illustrations. Record their source resource IDs and checksums during intake. Record the exact font family/weight versions used for native rendering.

A single small presentation asset catalog replaces the glTF-only readiness gate. Required images, fonts and decorative atlases must be loaded before accepting a capture or enabling interaction on the affected screen. Missing required assets surface a visible error; a blank image or fallback font is not a baseline. Returning between screens does not trigger an unrelated glTF dependency.

The 58 px source rail is a prototype tool, not shipped navigation. Its accent/number knobs do not create product settings. Default number visibility and accent follow the original reference.

### Reference availability

The original HTML and reference ZIP remain in the originating conversation. The ZIP contains ten source screenshots, an overview and metadata, not native-game evidence. Their exact hashes are in the reference record. An attempted Linear ZIP upload failed during PUT because the upload host could not resolve; no successful remote binary attachment is claimed.

The first implementation phase must put source screenshots/art references in durable project storage, confirm the checksums, and link them from this PR. Do not start pixel-matching from memory or fabricate a source image. The capture inventory is enough to identify the correct bundle, not a replacement for its bytes. This draft intentionally contains documentation and provenance metadata, not reference binaries or font files.

## 5. Screen requirements

| Screen | Required appearance | Live behavior/data |
| --- | --- | --- |
| Title | Key art, shading, spaced wordmark, paired hex emblems, decorative glyphs, reference buttons, seven save pips | Existing New Game/Continue; missing/unreadable save disabled; completed Continue goes to Ending. Derive completed pips from loaded progress, never from a fixed demo count. |
| Story | Background treatment, 300 × 300 lower-left portrait, frame corners, speaker, 212 px dialogue band, line pips, next arrow, Skip | Read the current pre-mission definition and `DialogueCursor`; advance each authored line; Skip goes once to Briefing with no progress/reward mutation. |
| Briefing | Mission header/pips, credits badge, illustration, map/enemy/bonus metadata, primary/bonus cards, reward tiles, Deploy | Derive current mission number/title/objectives/rewards, board dimensions and enemy count. Illustration is not collision data. Deploy enters the actual current mission. |
| Battle | Flat grid/tokens/terrain, header, inspector/log, anchored menus, threat/preview column, Resolve | Canonical state, availability and previews; complete interaction contract in section 6. |
| Result | Full-canvas scrim, 760 px card, 168 px ring, outcome icon/color, heading, primary badge, bonus state, Continue/Retry | Real terminal result and objective-specific copy. Continue persists exactly once; failure stays visible and retryable without leaving Battle. Retry rebuilds without rewards. |
| Aftermath | Story framing plus upper-right Base/Bonus/Total/Credits receipt | Dialogue comes from the just-completed `ActiveMission`; receipt comes from persisted completion. It must not read the next mission's dialogue after advancement. Last line routes to Upgrade or final Ending. |
| Hangar | Three illustrated mech columns, four tracks, three level pips per track, next effect, cost/currency, affordable/unaffordable/MAX states, Next Drop | Existing purchase validation/persistence. Failed purchases leave credit/level values unchanged. Next Drop preserves the real next-mission story/briefing flow. |
| Ending | Completion emblem, campaign heading, seven complete pips, three upgrade summaries, Title | Completed saved state; Return to Title; completed Continue reopens Ending. |

Pre-mission Skip is new. Do not add an aftermath skip where the source has only Next. Repeated clicks during a queued screen transition must not skip the next screen or grant another reward. Existing flow guards and a small pending-transition input check are sufficient; no navigation framework.

Story, briefing, hangar and ending copy must remain truthful. Preserve upgrade tracks and prices: HP increases maximum HP; Armor increases armor; Mobility increases evasion; Weapon increases base damage. Skills remain Aegis/Focus/Overdrive, not an invented pilot level system.

## 6. Battle interaction contract

### Inspection and activation

Keep one presentation-local inspected unit ID and use `battle.active_unit()` as the only activation authority. Rename/update the existing selected-unit field and call sites where appropriate; do not maintain a second independently mutable active-unit ID.

Clicking an enemy or finished player in Inspect mode opens its inspector only. Clicking a ready player begins activation only when no activation is active; otherwise it inspects that unit without abandoning the active one. Only the inspected, active, living player receives actionable commands. Other cards explain that the current activation must finish.

Next-ready uses a stable squad order (Vanguard, Gunner, Interceptor). When an activation exists, it can re-focus that unit, not start a different one; disable the action if that same unit is already inspected. With no activation it selects/starts the first living unfinished squad unit. After successful Wait, automatically focus/start the next eligible unit, unless the battle ended or playback is locked. All finished means no next unit and Resolve available. At battle entry keep the reference's unselected view until selection.

### Menu and targeting

Use a closed menu state: Hidden, Root, Weapons, Stances. Keep the existing Inspect/Move/Attack/AegisTarget modes. Root contains Move, Attack, Stance, Skill, Wait; Wait invokes `FinishUnit` and still requires a chosen reaction. Weapon submenus show shape/range/EN/push/counter information and retain readable weapon names in detail/focus treatment. Stances are Counter, Guard, Evade with current selection shown.

Back from Weapons/Stances returns to Root. Cancel/Escape during targeting clears tentative targeting/preview and returns to Root for the active unit. Escape at Root closes the menu. Board-background deselection closes inspection/menu without canceling an already-started activation or undoing committed actions; next-ready can re-focus it. An invalid target keeps targeting open with feedback and does not spend resources.

Targeting takes precedence over inspection: clicking a token or its occupied cell calls the same cell-target route. One click emits at most one command. Overlays and menus block underlying board input, including clicks on disabled controls. Decorations do not intercept their owning cell/button. Menu placement, hit testing and target previews use the same fitted coordinates, including every edge and corner.

Retain keyboard mirrors M, 1/2/3, P, C/G/E, F, Space, R through the same guarded command paths. Add Escape cancellation. No keyboard-only bypass of asset readiness, playback, pending transition, terminal state or command availability.

### Pilot skills

Aegis highlights only valid living orthogonal allies and preserves Guard non-stacking. Focus applies only to the next validated player Action attack, never to Counter. Overdrive is usable only before Move and adds its existing movement bonus. Used/active/ineligible visuals come from domain skill state; no UI-local consumption. An invalid Aegis click must not consume the skill.

### Restart and feedback

Permit Restart during an idle Player phase and Retry at idle Defeat. Reject it during loading, queued/active playback, enemy planning/resolution, Victory or a pending campaign transition. Rebuild the current authored mission with the current upgrades and a fresh seed, run opening planning once, reset inspection/menu/log/preview/effects, and preserve credits/save/progression. Reuse the existing restart path rather than adding a domain checkpoint or reset API.

Keep the six most recent battle-event entries, newest first, matching the source's compact log. Append events once from the existing ordered playback path; do not consume a second queue or infer authoritative HP/phase from the log. Reset on restart/mission exit. Show real hit/miss, damage, KO, counter, collision, hazard and explosion feedback; retain readable explanation of invalid commands.

Domain mutation completes before playback. The HUD continues to read domain state; event animation only presents what occurred and blocks commands until drained. Terminal scrim/actions wait until relevant playback finishes. Replace world-space damage text, glTF attack effects and boss camera shake with same-style token/board-layer effects. Do not shake the entire HUD or add a battle cutscene.

## 7. Whole-campaign extensions

The original mainly illustrates Mission 1. Include flat-glyph entries for Flanker, Bulwark, Controller, Dreadnought and Regent, with distinct silhouettes/details and consistent faction styling. No unknown-archetype fallback to Rifleman. Use the existing model's shape/range and skill data; bosses remain single-cell and ordinarily pushable.

Objective adapters cover all current variants: elimination counts; protected-unit HP and round cap; courier position/exit/deadline; marked-target HP; optional objective progress including victory-by-round. Overlapping footprints retain multiple locked threats, not only the last one drawn. A displaced Controller's damaged-only committed attack and future-only boss threshold behavior remain unchanged.

Define same-style extensions for those glyphs, objective cards/exit markers, Aegis targeting, loading/save errors, disabled interactions, and text overflow. Keep ordinary reference screenshots unchanged. Review the extension captures within this PR before final acceptance. Longer text and larger threat lists use bounded scroll/detail regions inside the assigned panels, not overlaps, unreadable font shrinkage or silent dropped entries.

## 8. Small implementation boundaries

Keep `campaign_ui.rs` as the existing campaign action/state adapter and split its large layout code into `presentation/screens/{title,dialogue,briefing,hangar,ending}.rs`. Dialogue rendering is shared by Story and Aftermath. This is a focused split of code being replaced, not a repository-wide refactor.

Add `presentation/theme.rs` for concrete colors/type/button/panel/pip helpers and `presentation/layout.rs` for canvas fit, grid geometry and menu clamping. Keep a single flat `battlefield.rs` renderer, `ui.rs` HUD snapshot/builder, and a small `battle_menu.rs` for menu rendering. Keep interaction routing in `interaction.rs`. Adapt `sync.rs` and `playback.rs` to the new components rather than creating parallel state engines.

The app retains its existing `GameScreen` variants. Update camera/root ownership so entering Battle, leaving Battle, retrying and returning to Title each leave one screen hierarchy and the needed Camera2d only. Retire mesh picking, old 3D setup, old glTF load gates, 3D-only reconciliation and boss-camera effects after replacements pass. Update README/CLAUDE guidance in the same PR; preserve historical design/validation documents as history.

Add only a repository-local, opt-in native capture example and a small image comparison script for this feature. Do not make this depend on an unfinished MCP/E2E framework. New helper APIs and exact task/test ownership are in the plan.

## 9. Acceptance contract

### Reference discipline

Preserve original source captures separately from state-aligned references, approved extensions and actual native captures. The current ten PNGs were rendered in Chromium with the 58 px rail cropped out and animations paused; the exact paused instant was not recorded. Recapture at a defined animation time before using pulsing pixels as goldens. The original result PNG uses the rail's forced victory and is composition evidence, not a valid terminal fixture.

For comparison, use the same deterministic legal battle/campaign fixture and corresponding displayed values in both reference and native captures. Data-only rebinding of the reference is allowed and recorded; changing its CSS/artwork to accommodate native drift is not. Dynamic copy not present in the prototype is an extension, not a silent mask. Do not run the prototype simulation as the game's rule source.

Record scenario, seed, mission, upgrades, credits, inspected unit, action/target state, viewport, logical canvas crop, DPI, animation time, asset-ready state and implementation commit. Native captures must be the actual Bevy-rendered game, not a pasted mock image. Keep before/after, side-by-side, overlay and difference images.

### Visual acceptance

Primary canvas is 1920 × 1080. Also verify 1280 × 720, 1600 × 900, 1600 × 1000 letterboxing, and HiDPI input alignment. There must be no unapproved differences in layout, typography, artwork/crops, color, borders, glyphs, pips, menu placement, overlays or state treatment. Only narrowly documented font/edge rasterization differences may be normalized. No broad masks, loose mismatch percentage, or baseline replacement to turn a failure green. Review pulses, effects and feedback in motion as well as stills.

Cover title no-save/valid/completed/error; every opening and aftermath portrait line; Skip; briefing; battle idle/player/enemy/finished inspection; root/weapons/stances; move/attack/Aegis targeting; focus/overdrive/used states; cancel; edge-clamped menus; resolve ready/locked playback; victory/defeat/save-error; hangar affordable/unaffordable/purchased/MAX/failure; and Ending. Include all later-mission objective/glyph/overlap/boss variants and long text.

### Functional acceptance

Exercise New Game through all seven missions, reward/aftermath/hangar, and Ending. Reload between missions and after completion. Verify no duplicate rewards, no unaffordable/maxed mutation, failed-save behavior, correct active mission on Retry, no input behind result/menu overlays, no lost activation on inspection, and no spent-action reset on cancel. Preserve domain regression tests for movement, previews, counters, pilot skills, environment and locked intents.

Run at the final implementation head:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Also compile/test the opt-in visual fixture feature without requiring a display in the normal test suite. Record manual/native evidence in `docs/validation/hpa-480.md`. The ticket is not Done and the PR is not ready to merge until all eight screen families, new interactions, campaign regressions and visual sign-off are complete. Documentation-only CI success is not implementation acceptance.
