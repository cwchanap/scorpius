# HPA-480 — Complete native UI visual parity

**Date:** 2026-09-05 (America/Vancouver)
**Issue:** [HPA-480](https://linear.app/cwchanap/issue/HPA-480)
**Branch:** `hpa-480-ui-visual-parity`
**Status:** Design for implementation; native UI parity is not yet implemented or verified.
**Baseline:** `d981682840eb9147ba9eb7f7c56b2ceae88a3aed` on `main`.
**Plan:** [Implementation plan](../plans/2026-09-05-hpa-480-ui-visual-parity.md)
**Reference record:** [Source identity and capture inventory](../../references/hpa-480/reference-manifest.json)

## 1. Delivery decision

Deliver the complete visual overhaul as **one ticket, one branch, one PR**. The draft begins with this design and its implementation plan; implementation, tests, reference extensions, visual fixes, and acceptance evidence continue on the same branch and PR. Do not merge a planning-only PR, open dependent implementation/closeout PRs, or split plan phases into Linear sub-issues.

Replace all eight presentation families: Title, Story, Briefing, Battle, Result, Aftermath, Hangar, and Ending. Include the missing interactions required by the design while preserving the existing seven-mission game. Completion means native visual parity plus functioning gameplay, not palette similarity.

This supersedes both the earlier three-ticket proposal and the old angled battlefield presentation constraint. Keep Bevy and the existing game rules; do not retain a selectable old renderer or add a WebView.

### Source precedence

`Scorpius UI (offline).html` is authoritative for appearance and displayed interaction affordances. The prior `scorpius-ui-scope.md` remains the agreed feature scope except for its superseded three-ticket section. Existing Rust domain/campaign rules remain authoritative for values, legal actions, outcomes, rewards, and persistence.

The HTML's simplified simulation, hardcoded credits, prototype screen rail, and forced result previews are not game rules. Later-mission states absent from the original require explicitly identified same-style extensions; they do not justify silently changing the reference or gameplay.

## 2. Verified reuse and non-goals

The current game already has Missions 1–7, three fixed player mechs, six regular enemy archetypes, two bosses, committed intents, reactions, environmental interactions, pilot skills, rewards, upgrades, saves, and campaign completion. HPA-632/635/637/523/524/386 are complete. This is a presentation replacement plus narrow interaction plumbing.

| Existing seam | Use in this change |
| --- | --- |
| `src/app.rs`: `GameScreen`, enter/exit systems, `enter_battle`, `teardown_battle_screen` | Keep screen flow. Result remains an overlay inside Battle; Hangar remains `GameScreen::Upgrade`. |
| `src/domain/battle.rs`, `combat.rs`, `enemy.rs`, `environment.rs` | Keep authoritative rules, queries, previews, event generation, RNG order, and committed-intent semantics. |
| `src/mission/*` | Read authored mission data and stable IDs. Do not move rules into presentation. |
| `src/campaign/session.rs`, `progression.rs`, `save.rs` | Reuse New Game, Continue, `CompletionReceipt`, completion, purchases, and persistence. |
| `src/presentation/campaign_ui.rs` | Reuse `apply_campaign_action`, `DialogueCursor`, `dialogue_snapshot`, `campaign_destination`, status/error routing, and persistence adapters; replace layout builders. |
| `src/presentation/interaction.rs` | Reuse/extend `route_cell_click`, `execute_command`, `restart_battle`, `reset_transient_battle_state`, keyboard/observer adapters. |
| `src/presentation/ui.rs`: `HudSnapshot::from_battle` | Replace string blobs with typed fields while retaining one derived snapshot path. |
| `src/presentation/playback.rs` | Keep ordered `BattleEventQueue` consumption and `EventPlayback::input_locked`; change only presentation effects/logging. |
| `src/presentation/assets.rs`: `AssetLoadStatus` | Keep one readiness/error gate; replace glTF-only asset contents. |
| `tests/presentation_app.rs`, campaign tests, inline domain tests | Preserve behavioral coverage while replacing renderer-specific assertions. |

Keep one crate, Rust 2024, Bevy 0.19, domain Bevy-free, and one save model. Do **not** add a browser runtime, second UI framework, plugin/registry system, save migration, backward-compatibility layer, mission select, save slots, checkpoints, undo, inventory, new combat engine, MCP framework, generic E2E framework, or product Settings screen. Normal tests remain headless; native capture is opt-in.

The old `MissionAssets`/15 glTF scenes, `Camera3d`, `MeshPickingPlugin`, `grid_to_world`, world-space effects, and boss camera shake form one 3D presentation path. Retire them atomically with their new UI-grid replacements rather than preserving parallel paths.

## 3. Fixed visual geometry and coordinate model

The production canvas is **1920 × 1080 design pixels**. It scales uniformly by `min(window_logical_width / 1920, window_logical_height / 1080)` and centers with letterboxing. Rendering, hit testing, token/menu anchoring, and screenshot cropping use the same `CanvasLayout`; physical/logical DPI conversion happens once at the window boundary.

Battle is **Bevy UI under that fitted canvas**, not sprites or meshes in a second world coordinate system. The source battle root has 22 px padding, 14 px vertical/column gaps, a 78 px header, 352 px sidebars, and a centered 912 × 912 board. Therefore the board rect at 1920 × 1080 is fixed at design coordinates:

```text
left = 504
 top = 130
width = 912
height = 912
```

The middle grid column is 1144 px wide, which centers the 912 px board with 116 px on either side. The post-header content region is 944 px tall, which centers the board with 16 px above/below.

Every current authored mission is 9 × 9. HPA-480 deliberately treats that as a presentation invariant rather than inventing a generic board engine. `layout.rs` exposes:

```rust
pub const DESIGN_SIZE: Vec2 = Vec2::new(1920.0, 1080.0);
pub const BOARD_SIZE: Vec2 = Vec2::splat(912.0);

pub struct CanvasLayout {
    pub scale: f32,
    pub offset: Vec2,
}

impl CanvasLayout {
    pub fn fit(window_logical: Vec2) -> Self;
    pub fn to_design(&self, window_point: Vec2) -> Option<Vec2>;
}

pub const fn board_rect() -> Rect;
pub fn cell_rect(pos: GridPos) -> Rect;
pub fn clamp_menu(anchor: Vec2, menu_size: Vec2, bounds: Rect) -> Vec2;
```

`board_rect()` is `Rect::from_corners(Vec2::new(504.0, 130.0), Vec2::new(1416.0, 1042.0))`. `cell_rect(GridPos { x, y })` is board-local `x * 102`, `y * 102` plus the board origin, with a 96 × 96 cell. Domain `GridPos(0,0)` is the **top-left** visual cell; `y = 8` is the bottom player row. Reject/loud-error if an active mission is not 9 × 9. Letterbox pixels are never board input.

One live screen owns one `Camera2d`. Enter/exit/restart tests must catch duplicate camera/root spawning before the 3D cutover proceeds.

## 4. Typography, art, and icon contract

Use the source values, not approximate replacements: Chakra Petch for interface copy; IBM Plex Mono for numeric/technical copy; cyan `#3ec7db`, background `#05080f`, panel `#0a1420`, red `#ff6b5c`, amber `#ffd175`, green `#7fdc9a`; source alpha/gradient/stroke/glow/spacing values elsewhere.

`theme.rs` becomes the **only** source of production text/font/color/panel/button/pip/bar helpers. Delete the current local `text_font` helpers in `campaign_ui.rs` and `ui.rs` during cutover. Fallback/default fonts are an asset error, not an acceptable baseline.

Reuse the four matching VN assets and import the five missing PNGs: title key art, briefing illustration, Vanguard, Gunner, and Interceptor art. The reference manifest now pins the exact seven Latin font resources actually needed by the English UI: Chakra Petch 400/500/600/700 and IBM Plex Mono 400/500/600. It records both the source WOFF2 hashes and deterministic TTF conversion hashes for Bevy-native assets. Implementation uses those exact bytes/converted outputs; it does not fetch an unpinned “current” font release.

The HTML contains 55 inline SVG occurrences, 51 exact variants, and 40 unique path geometries. The reference manifest pins both normalized catalog hashes and a closed semantic icon list. Task 1 exports them once into a static native atlas (plus fixed frame-corner decorations) and records the atlas hash. There is no runtime SVG/HTML dependency and no later emoji/generic-icon substitution.

Required images, font assets, and icon atlas entries must reach `AssetLoadStatus::Ready` before affected interaction/capture. Missing required assets show a visible error. The 58 px source rail and its editor controls remain prototype-only.

## 5. Typed presentation data

The current presentation formats state into text blobs such as `selected_summary`, `round_phase`, `ThreatSnapshot.cells`, `briefing_copy`, `aftermath_reward_copy`, `ending_copy`, and `upgrade_row_copy`. The new card/bar/pip layouts must not parse those strings or bypass the snapshot layer. Replace them with the following closed typed view data.

### Battle snapshots

```rust
pub struct InspectorSnapshot {
    pub id: UnitId,
    pub name: &'static str,
    pub faction: Faction,
    pub archetype: UnitArchetype,
    pub hp: i16,
    pub max_hp: i16,
    pub en: i16,
    pub max_en: i16,
    pub armor: i16,
    pub movement: u8,
    pub evasion: i16,
    pub moved: bool,
    pub acted: bool,
    pub finished: bool,
    pub knocked_out: bool,
    pub stance: Option<Reaction>,
    pub active: bool,
}

pub struct ThreatSnapshot {
    pub attacker: UnitId,
    pub attacker_name: &'static str,
    pub weapon: WeaponId,
    pub weapon_name: &'static str,
    pub cells: Vec<GridPos>,
    pub intended_occupant: Option<UnitId>,
    pub normal_damage: i16,
    pub hit_chance: u8,
}

pub enum PrimaryProgressSnapshot {
    EliminateAll { remaining: usize },
    Protect { target: UnitId, name: &'static str, hp: i16, max_hp: i16, round: u16, cap: u16 },
    Intercept { target: UnitId, name: &'static str, position: GridPos, escape: GridPos, distance: u8, round: u16, deadline: u16 },
    EliminateTarget { target: UnitId, name: &'static str, hp: i16, max_hp: i16 },
}

pub enum OptionalProgressSnapshot {
    Turnabout { complete: bool },
    ProtectTargetAtHalfHp { target: UnitId, hp: i16, max_hp: i16, complete: bool },
    VictoryByRound { current: u16, cap: u16, complete: bool },
}
```

`HudSnapshot` carries `round: u16`, `phase: BattlePhase`, optional round cap, `inspector: Option<InspectorSnapshot>`, typed primary/optional progress, typed weapon/pilot availability, `Vec<ThreatSnapshot>`, terminal/result fields, and resolve/restart availability. Display strings are generated only in leaf rendering helpers.

The battle menu uses typed `WeaponSnapshot` rows (`WeaponId`, name, range, `WeaponShape`, base damage, EN cost, push, counter-weapon, enabled) and typed pilot state (`Ready`, `Active`, `Used`, `Ineligible`). No combat formula is duplicated in UI.

`RecentBattleLog` stores at most six already-formatted entries, newest first. `playback.rs` calls the existing `ui::format_event` (made `pub(crate)`) once when an event is dequeued; do not create a second event formatter or second event queue.

### Campaign snapshots

```rust
pub struct BriefingSnapshot {
    pub mission: MissionId,
    pub title: &'static str,
    pub board_width: u8,
    pub board_height: u8,
    pub enemy_count: usize,
    pub primary: &'static str,
    pub optional: &'static str,
    pub base_reward: u32,
    pub optional_reward: u32,
    pub credits: u32,
}

pub struct UpgradeRowSnapshot {
    pub mech: PlayerMech,
    pub track: UpgradeTrack,
    pub level: u8,
    pub current_bonus: u16,
    pub next_bonus: Option<u16>,
    pub cost: Option<u32>,
    pub affordable: bool,
}

pub struct HangarSnapshot {
    pub credits: u32,
    pub rows: Vec<UpgradeRowSnapshot>,
}
```

Aftermath renders `CompletionReceipt` fields directly; it does not turn the receipt into one multiline string. Ending uses typed credits and upgrade levels per `PlayerMech`. Title/save progress uses `CampaignState.next_mission/completed` rather than a formatted summary. `DialogueSnapshot` remains as-is.

## 6. Screen requirements

| Screen | Required appearance | Live behavior/data |
| --- | --- | --- |
| Title | Key art, shading, spaced wordmark, paired hex emblems, decorative glyphs, reference buttons, seven save pips | Existing New Game/Continue; missing/unreadable save disabled; completed Continue goes to Ending. |
| Story | Background treatment, 300 × 300 portrait, frame corners, speaker, 212 px dialogue band, line pips, next arrow, Skip | Current pre-mission `DialogueSnapshot`; Skip is legal only here and goes once to Briefing. |
| Briefing | Mission header/pips, credits badge, illustration, map/enemy/bonus metadata, objective cards, reward tiles, Deploy | `BriefingSnapshot`; illustration is never collision data. |
| Battle | UI grid/tokens/terrain, header, inspector/log, anchored menus, threat/preview column, Resolve | Typed battle snapshots and interaction composition below. |
| Result | Full-canvas scrim, 760 px card, 168 px ring, outcome icon/color, primary badge, bonus state, Continue/Retry | Real terminal result/objective; Continue persists once, Retry does not pay. |
| Aftermath | Story framing plus Base/Bonus/Total/Credits receipt | Just-completed `ActiveMission` dialogue plus `CompletionReceipt`; no Skip. |
| Hangar | Three illustrated mech columns, four tracks, level pips, next effect, cost, affordable/unaffordable/MAX, Next Drop | `HangarSnapshot`; purchases still use `persist_purchase`. Mobility remains evasion. |
| Ending | Completion emblem, campaign heading, seven complete pips, three upgrade summaries, Title | Completed saved state; completed Continue reopens Ending. |

## 7. Battle interaction composition

`InteractionState` remains the single presentation interaction resource but its fields are made explicit:

```rust
pub struct InteractionState {
    pub inspected_unit: Option<UnitId>,
    pub hovered_cell: Option<GridPos>,
    pub mode: InteractionMode, // Inspect | Move | Attack(WeaponId) | AegisTarget
    pub menu: MenuState,       // Hidden | Root | Weapons | Stances
    pub preview: Option<AttackPreview>,
}
```

`inspected_unit` is **view-only**. Every mutating command derives its subject from `battle.active_unit()`; `require_selected_active_unit` is replaced with a helper that validates/returns the domain active unit. Inspection never owns or changes activation authority.

| Chrome (`MenuState`) | Targeting (`InteractionMode`) | Inspected ID | Mutating commands use |
| --- | --- | --- | --- |
| Hidden | Inspect | none / last inspected | none |
| Root / Weapons / Stances | Inspect | active living player | `battle.active_unit()` |
| Hidden | Move / Attack / AegisTarget | active living player | `battle.active_unit()` |
| Hidden | Inspect | enemy or finished/inactive player | none; inspector explains active unit remains authoritative |

Clicking an enemy or finished/inactive player in Inspect mode changes inspection only. Clicking a ready player starts activation only when `battle.active_unit()` is `None`; while another activation exists it is inspection-only. Re-focusing the active unit restores Root. At battle entry no unit is inspected.

Next-ready uses stable Vanguard/Gunner/Interceptor order. If an activation exists, next-ready re-focuses that active unit rather than starting another. After a **successful** `FinishUnit`/Wait, it starts/focuses the next living unfinished unit. All finished means no next unit and Resolve becomes available.

### Menu, targeting, and Cancel

Root contains Move, Attack, Stance, Skill, Wait. Weapons/Stances are submenus. Back returns to Root. Introduce `CommandAction::Cancel` so pointer Cancel and Escape use the same guarded dispatch; there is no keyboard-only cancellation path.

Move/Attack/Aegis targeting state changes **only after the domain call succeeds**. Invalid move/attack/Aegis targets retain their targeting mode/preview context and show an error; an invalid Aegis click no longer drops `AegisTarget`. Successful target execution returns to Inspect/Root as appropriate. Cancel clears only tentative target/preview state; it never restores spent movement/action or undoes a committed attack.

Targeting takes precedence over inspection: token clicks resolve to the token's `unit.position` and call the same cell route. One click emits at most one command. UI controls, scrims, and disabled controls block board input. Decorations do not steal their owning cell/button hit target. Menu anchors and hit testing use `board_rect`/`cell_rect` under the same canvas fit.

Keep existing M, 1/2/3, P, C/G/E, F, Space, R mirrors. Escape maps to `CommandAction::Cancel` and therefore cannot bypass asset readiness, playback, terminal state, or command availability.

Aegis highlights only legal living orthogonal allies; Focus and Overdrive retain their real timing/consumption rules. No UI-local skill consumption.

## 8. Campaign transition guard and restart

The existing `NextState<GameScreen>` already represents whether a screen transition is queued, so HPA-480 does **not** add a duplicate pending-transition resource. Add one small shared predicate over `NextState` and use it before any transition-causing action.

`CampaignUiAction::SkipDialogue` is legal only when the current screen is `PreMissionStory`; otherwise it sets a `CampaignStatus` error and leaves state unchanged. If `NextState` is already pending, Skip/Advance/Continue/Proceed are no-ops before cursor, save, reward, or purchase mutation. `apply_campaign_action` receives the current `GameScreen` (or an equivalent explicit guard input) so this rule is testable instead of relying on which button happened to be rendered.

Victory Continue in `interaction.rs` uses the same pending check **before** `complete_current_mission`. This preserves the existing `AlreadyAdvanced` guard as a backstop without intentionally invoking it on duplicate same-frame clicks. Hangar Proceed and last-line dialogue advance use the same guard.

Permit Restart during idle Player and Retry during idle Defeat. Reject during loading, queued/active playback, enemy planning/resolution, Victory, or pending screen transition. Reuse `restart_battle`/`reset_transient_battle_state`; rebuild current mission with current upgrades/fresh seed, run opening planning once, clear interaction/menu/log/preview/effects, and preserve campaign/save/credits.

## 9. Flat board, glyphs, playback, and whole-campaign states

Cells, terrain, props, extraction, unit tokens, telegraphs, intent markers, target guides, HP bars, awaiting dots, selection/inspection rings, and finished dimming are Bevy UI children of the fitted canvas/board. Cell nodes carry `CellVisual(GridPos)` and token nodes carry `UnitVisual(UnitId)`. Token clicks map to the unit's current `GridPos`; occupancy remains domain-only.

Replace `scene_index(UnitArchetype)` with an equally exhaustive `glyph_for(UnitArchetype) -> UnitGlyph` match covering all eleven current archetypes: Vanguard, Gunner, Interceptor, Rifleman, Striker, Artillery, Flanker, Bulwark, Controller, Dreadnought, Regent. No fallback glyph.

`play_battle_events` preserves ordering/input lock and moves effects into board-local UI coordinates. `RecentBattleLog` receives `format_event` output once per dequeued event. Remove Camera3d/glTF transforms, world-to-viewport damage text, and boss camera shake in the same Task 3 cutover. Re-entry/restart must leave exactly one Camera2d and one battle root.

Objective adapters cover elimination count, protect target HP/round cap, courier position/escape/deadline, target HP, Turnabout completion, protect-at-half-HP, and `VictoryByRound { current, cap }`. Overlapping committed footprints render independently. Controller damage-only degraded intents and future-only boss threshold changes remain untouched.

Same-style extensions are required for Flanker/Bulwark/Controller/Dreadnought/Regent glyphs, objective/exit cards, Aegis targeting, loading/save errors, disabled interactions, and text overflow. Long text/threat lists use bounded scroll/detail regions, not font shrinkage or silent truncation.

## 10. Closed native capture scenario matrix

The scenario set is fixed **now**, before capture tooling exists. The manifest carries the same matrix. A `CaptureScenario` parser/type may group these dimensions, but it must accept exactly this closed set and reject unknown names.

### Campaign scenarios

- Title: `title-no-save`, `title-progress-m1`, `title-progress-m4`, `title-completed`, `title-save-error`.
- Story: `story-m1-line1..3` through `story-m7-line1..3` (all seven pre-mission scenes have exactly three lines).
- Briefing: `briefing-m1` through `briefing-m7`.
- Aftermath: `aftermath-m1-line1..2` through `aftermath-m6-line1..2`, plus `aftermath-m7-line1..3`.
- Hangar: `hangar-affordable`, `hangar-unaffordable`, `hangar-purchased`, `hangar-maxed`, `hangar-save-error`.
- Ending: `ending-complete`.

### Battle core scenarios

`battle-idle`, `battle-active-vanguard`, `battle-inspect-enemy`, `battle-inspect-finished-while-active`, `battle-menu-root`, `battle-menu-weapons`, `battle-menu-stances`, `battle-move-targeting`, `battle-attack-targeting-empty`, `battle-attack-targeting-occupied`, `battle-aegis-targeting`, `battle-aegis-invalid`, `battle-focus-ready`, `battle-focus-pending`, `battle-focus-used`, `battle-overdrive-ready`, `battle-overdrive-active`, `battle-overdrive-used`, `battle-cancel-targeting`, `battle-resolve-ready`, `battle-playback`, `result-victory-bonus`, `result-victory-no-bonus`, `result-defeat`, `result-save-error`, `asset-loading`, `asset-error`.

### Menu-clamp scenarios

For each menu kind `root`, `weapons`, `stances`, capture all eight anchor classes: `top-left`, `top`, `top-right`, `left`, `right`, `bottom-left`, `bottom`, `bottom-right`. Scenario names are `menu-{kind}-{anchor}`. These 24 cases are part of acceptance, not Task 6 discovery work.

### Whole-campaign battle extensions

`m2-protect-full`, `m2-protect-low`, `m2-round-cap`; `m3-intercept-far`, `m3-intercept-near`, `m3-deadline`; `m4-target-bulwark`, `m4-chain-reaction`; `m5-overlapping-threats`, `m5-victory-by-round`; `m6-boss-high`, `m6-boss-low`; `m7-boss-high`, `m7-boss-low`, `m7-victory-by-round`; `roster-all-glyphs`; `long-objective-copy`; `max-threat-list`.

Primary captures are 1920 × 1080 at deterministic seed/time. Secondary sizes 1280 × 720, 1600 × 900, 1600 × 1000 validate fit/letterboxing; HiDPI validates pointer alignment. Original, state-aligned, extension, native, and comparison evidence remain separately named.

## 11. Implementation boundaries

Keep `campaign_ui.rs` as action/state adapter and move only replaced layouts into `presentation/screens/{title,dialogue,briefing,hangar,ending}.rs`. Dialogue rendering is shared by Story/Aftermath; Skip is rendered only for Story. Keep interaction logic in `interaction.rs` rather than hiding it in screen files.

Add only `presentation/theme.rs`, `layout.rs`, and a small `battle_menu.rs` as new runtime presentation modules. Adapt `battlefield.rs`, `ui.rs`, `sync.rs`, `playback.rs`, `assets.rs`, `mod.rs`, and `app.rs`; do not create parallel engines. Add repository-local opt-in capture code and a small comparison script only for HPA-480.

Update README/CLAUDE after cutover. Preserve historical specs/validation as history.

## 12. Acceptance contract

Preserve original source captures separately from state-aligned references, approved extensions, and actual native captures. Recapture pulsing reference states at an explicit animation time before treating animated pixels as goldens. The original forced-result PNG is composition evidence only.

For every scenario record seed, mission, upgrades, credits, inspected/active unit, menu/target state, viewport, canvas crop, DPI, animation time, asset-ready state, and implementation commit. Native captures must be actual Bevy output. Compare side-by-side, 50% overlay, and absolute difference at equal dimensions; do not rescale, broadly mask, or auto-approve by a loose percentage.

There must be no unapproved differences in layout, typography, artwork/crops, color, borders, glyphs, pips, menus, overlays, or interaction state. Narrow rasterization-only exceptions must be documented individually. Review motion as well as stills.

Functionally exercise New Game through all seven missions and Ending, reloads, bonus success/failure, purchases, save failures, restart/retry, duplicate transition clicks, target cancellation, inspection during an active unit, and completed Continue. Preserve domain regression tests for movement, previews, counters, pilots, environment, RNG order, and locked intents.

Run at the final implementation head:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo build --release
```

The ticket remains open and PR remains draft until the full scenario matrix, behavioral gates, and visual evidence are accepted. Documentation-only CI success is not implementation acceptance.
