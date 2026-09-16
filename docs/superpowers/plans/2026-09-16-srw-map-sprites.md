# SRW-Style Map Sprites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the upright 76×64 token cards with dedicated 96×96 SD anime-mecha map sprites standing on the isometric board (PR #8, presentation-only).

**Architecture:** The existing `TokenCard` entity stays the unit root and pick target; it becomes a full-image `ImageNode` using one new authored geometry (96×96, bottom-centered, feet at the tile center). Shared stage-local placement helpers in `layout.rs` feed spawn/sync/playback. Token clicks stay UnitId-routed in Inspect mode but convert token-local hits to stage points and resolve the underlying diamond in targeting modes.

**Tech Stack:** Rust 2024, Bevy 0.19 UI (`ImageNode`, `UiTransform`, `UiPickingCamera`), native Bevy UI nodes only.

**Spec:** PR #8 body (`https://github.com/cwchanap/scorpius/pull/8`) — the binding design contract. It supersedes HPA-480 §4.2's upright-token pin only; every other HPA-480 contract (9×9 projection, `iso_center()`, depth stack, 112×96 footprint, diamond resolver) is unchanged.

## Global Constraints

- Presentation-only: no `src/domain/` or `src/mission/` behavior changes. `src/domain/` must never import Bevy.
- One crate, `bevy = "0.19"` pinned. No new dependencies, no second asset manager, no skin registry, no sampler/mipmap infrastructure.
- `MAP_UNIT_WIDTH = 96`, `MAP_UNIT_HEIGHT = 96` are the single authored unit geometry. `TOKEN_WIDTH`/`TOKEN_HEIGHT` are deleted, not deprecated.
- Overlay geometry (root-local px): HP bar `left 12, top -10, 72×6`; HP text `right 0, top -30`; awaiting dot `left 4, top 4, 9×9`; enemy glyph `left 4, top 18, 18×18`. Player glyphs are removed (sprites are faction-distinct); enemy glyphs stay.
- Activation alpha on the root `ImageNode.color`: finished player `0.52`; active/unfinished player and all enemies `1.0`. No `BackgroundColor` card chrome anywhere on the token root.
- Footprint shadow stays centered at `cy + 1` (authored atlas offset — do not "fix" it).
- Enemy archetypes all share `enemy_map`; `UiAssets::map_sprite` matches exhaustively with no `_` arm. No `Default for UiAssets` (conflicts with the custom `FromWorld`).
- Existing E2E ids (`battle.unit.vanguard`, `battle.cell.4.8`) and observer/component contract (`TokenCard + UnitVisual + e2e id + pointer observers`) are unchanged.
- CI gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `cargo build --release`. E2E: `cargo test --features e2e --test e2e -- --test-threads=1` (CI runs it under xvfb).
- Map sprite assets already exist on this branch at `assets/ui/map/{vanguard,gunner,interceptor,enemy}.png` (256×256 RGBA, bottom-center feet anchoring).
- Commits follow Conventional Commits (`feat:`, `test:`, `docs:`).

---

### Task 1: Extend the asset catalog with map sprites

**Files:**
- Modify: `src/presentation/assets.rs`
- Modify (fixture constructors only): `src/presentation/ui.rs:3688` (`test_ui_assets`), `src/presentation/playback.rs:719` and `src/presentation/playback.rs:810` (inline tests), `tests/campaign_flow.rs:54` (`blank_ui_assets`), `tests/presentation_app.rs:69` (`blank_ui_assets`), `tests/ui_snapshots.rs:72` (`test_assets`)

**Interfaces:**
- Produces: `pub const VANGUARD_MAP_PATH/GUNNER_MAP_PATH/INTERCEPTOR_MAP_PATH/ENEMY_MAP_PATH: &str`; `UiAssets { vanguard_map, gunner_map, interceptor_map, enemy_map: Handle<Image> }`; `pub fn map_sprite(&self, archetype: UnitArchetype) -> &Handle<Image>`; `images()` returns an 11-element array.

- [ ] **Step 1: Write the failing tests** — add an inline `#[cfg(test)] mod tests` at the end of `src/presentation/assets.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::UnitArchetype;

    fn catalog() -> UiAssets {
        UiAssets {
            key_art: Handle::default(),
            briefing_art: Handle::default(),
            vanguard_art: Handle::default(),
            gunner_art: Handle::default(),
            interceptor_art: Handle::default(),
            vanguard_map: Handle::default(),
            gunner_map: Handle::default(),
            interceptor_map: Handle::default(),
            enemy_map: Handle::default(),
            icons: Handle::default(),
            board: Handle::default(),
            fonts: std::array::from_fn(|_| Handle::default()),
        }
    }

    #[test]
    fn map_sprite_covers_every_archetype_and_shares_one_enemy_handle() {
        let assets = catalog();
        let enemies = [
            UnitArchetype::Rifleman,
            UnitArchetype::Striker,
            UnitArchetype::Artillery,
            UnitArchetype::Flanker,
            UnitArchetype::Bulwark,
            UnitArchetype::Controller,
            UnitArchetype::Dreadnought,
            UnitArchetype::Regent,
        ];
        for archetype in enemies {
            assert_eq!(
                std::ptr::eq(assets.map_sprite(archetype), &assets.enemy_map),
                true,
                "{archetype:?} must return the shared enemy map handle"
            );
        }
        assert_eq!(assets.map_sprite(UnitArchetype::Vanguard), &assets.vanguard_map);
        assert_eq!(assets.map_sprite(UnitArchetype::Gunner), &assets.gunner_map);
        assert_eq!(
            assets.map_sprite(UnitArchetype::Interceptor),
            &assets.interceptor_map
        );
    }

    #[test]
    fn image_readiness_gate_covers_all_eleven_images() {
        let assets = catalog();
        assert_eq!(assets.images().len(), 11);
    }
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test --lib presentation::assets` → FAIL (no `vanguard_map` field).

- [ ] **Step 3: Implement.** In `src/presentation/assets.rs`:

Add after `INTERCEPTOR_ART_PATH`:

```rust
pub const VANGUARD_MAP_PATH: &str = "ui/map/vanguard.png";
pub const GUNNER_MAP_PATH: &str = "ui/map/gunner.png";
pub const INTERCEPTOR_MAP_PATH: &str = "ui/map/interceptor.png";
pub const ENEMY_MAP_PATH: &str = "ui/map/enemy.png";
```

Add `use crate::domain::model::UnitArchetype;` to imports. Extend the `UiAssets` struct with `vanguard_map`, `gunner_map`, `interceptor_map`, `enemy_map` (`Handle<Image>`, after the `*_art` fields). Load them in `FromWorld::from_world`. Change `images()` to:

```rust
    fn images(&self) -> [(&'static str, &Handle<Image>); 11] {
        [
            (KEY_ART_PATH, &self.key_art),
            (BRIEFING_ART_PATH, &self.briefing_art),
            (VANGUARD_ART_PATH, &self.vanguard_art),
            (GUNNER_ART_PATH, &self.gunner_art),
            (INTERCEPTOR_ART_PATH, &self.interceptor_art),
            (VANGUARD_MAP_PATH, &self.vanguard_map),
            (GUNNER_MAP_PATH, &self.gunner_map),
            (INTERCEPTOR_MAP_PATH, &self.interceptor_map),
            (ENEMY_MAP_PATH, &self.enemy_map),
            (theme::ICON_ATLAS_PATH, &self.icons),
            (theme::BOARD_ATLAS_PATH, &self.board),
        ]
    }
```

Add to `impl UiAssets`:

```rust
    /// Handle of the tactical-map sprite for `archetype`. Exhaustive by
    /// design: a new archetype must decide its map art here.
    pub fn map_sprite(&self, archetype: UnitArchetype) -> &Handle<Image> {
        match archetype {
            UnitArchetype::Vanguard => &self.vanguard_map,
            UnitArchetype::Gunner => &self.gunner_map,
            UnitArchetype::Interceptor => &self.interceptor_map,
            UnitArchetype::Rifleman
            | UnitArchetype::Striker
            | UnitArchetype::Artillery
            | UnitArchetype::Flanker
            | UnitArchetype::Bulwark
            | UnitArchetype::Controller
            | UnitArchetype::Dreadnought
            | UnitArchetype::Regent => &self.enemy_map,
        }
    }
```

Update all six fixture/construction sites (paths above) by adding the four `*_map: Handle::default(),` fields in the same position. Do not add `Default for UiAssets`.

- [ ] **Step 4: Verify green + gates** — `cargo test --lib presentation::assets` PASS, then `cargo test --all-targets` and `cargo clippy --all-targets --all-features -- -D warnings` PASS.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: add map sprite assets to the UI catalog"`

---

### Task 2: Authored unit geometry and shared placement helpers

**Files:**
- Modify: `src/presentation/layout.rs`

**Interfaces:**
- Produces: `pub const MAP_UNIT_WIDTH: f32 = 96.0;` `pub const MAP_UNIT_HEIGHT: f32 = 96.0;` and stage-local helpers `pub fn unit_root_top_left(pos: GridPos) -> Vec2`, `pub fn footprint_top_left(pos: GridPos) -> Vec2`, `pub fn selection_top_left(pos: GridPos) -> Vec2`. `TOKEN_WIDTH`/`TOKEN_HEIGHT` are NOT removed in this task (call sites still compile).

- [ ] **Step 1: Write failing tests** — add an inline `#[cfg(test)] mod tests` to `layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_map_unit_geometry_pins_feet_to_the_tile_center() {
        let pos = GridPos::new(4, 4);
        let center = iso_center(pos) - battle_stage_rect().min;
        let root = unit_root_top_left(pos);
        assert_eq!(MAP_UNIT_WIDTH, 96.0);
        assert_eq!(MAP_UNIT_HEIGHT, 96.0);
        assert_eq!(root, Vec2::new(center.x - 48.0, center.y - 96.0));
    }

    #[test]
    fn footprint_and_selection_helpers_reproduce_authored_offsets() {
        let pos = GridPos::new(2, 6);
        let center = iso_center(pos) - battle_stage_rect().min;
        // The 112x96 shadow atlas rect keeps its ellipse centered at cy + 1.
        assert_eq!(footprint_top_left(pos), Vec2::new(center.x - 56.0, center.y - 68.0));
        assert_eq!(selection_top_left(pos), Vec2::new(center.x - 56.0, center.y - 28.0));
    }
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test --lib presentation::layout` → FAIL (helpers undefined).

- [ ] **Step 3: Implement** in `layout.rs` (place next to `token_depth`):

```rust
/// Authored size of one tactical unit's map-sprite root. The sprite feet
/// land on the tile center; this is the single geometry for spawn, sync,
/// playback, and token-hit conversion.
pub const MAP_UNIT_WIDTH: f32 = 96.0;
pub const MAP_UNIT_HEIGHT: f32 = 96.0;

/// Stage-local top-left of a unit's 96x96 sprite root (bottom-center anchor).
pub fn unit_root_top_left(pos: GridPos) -> Vec2 {
    let center = iso_center(pos) - battle_stage_rect().min;
    Vec2::new(
        center.x - MAP_UNIT_WIDTH * 0.5,
        center.y - MAP_UNIT_HEIGHT,
    )
}

/// Stage-local top-left of the 112x96 packed footprint/shadow rect. The
/// authored atlas centers its ellipse at `cy + 1`; do not "correct" this.
pub fn footprint_top_left(pos: GridPos) -> Vec2 {
    let center = iso_center(pos) - battle_stage_rect().min;
    Vec2::new(center.x - TILE_WIDTH * 0.5, center.y - 68.0)
}

/// Stage-local top-left of the 112x56 selection diamond.
pub fn selection_top_left(pos: GridPos) -> Vec2 {
    let center = iso_center(pos) - battle_stage_rect().min;
    Vec2::new(
        center.x - TILE_WIDTH * 0.5,
        center.y - TILE_HEIGHT * 0.5,
    )
}
```

- [ ] **Step 4: Verify** — `cargo test --lib presentation::layout` PASS; `cargo clippy --all-targets --all-features -- -D warnings` PASS.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: author the 96px map-unit geometry and placement helpers"`

---

### Task 3: Atomic sprite cutover — root becomes the image, card chrome dies

One commit. No intermediate state where an opaque card sits behind a sprite.

**Files:**
- Modify: `src/presentation/battlefield.rs` (`spawn_token`, `token_card_node` → `token_sprite_node`, `token_selection_node`)
- Modify: `src/presentation/sync.rs` (`apply_unit_transforms`, `sync_token_cards`)
- Modify: `src/presentation/playback.rs` (`node_position`, `footprint_position`, `selection_position`, `rendered_stage_center`, damage-number origin, inline test at ~line 776)
- Modify: `src/presentation/layout.rs` (delete `TOKEN_WIDTH`/`TOKEN_HEIGHT`)
- Modify: `tests/ui_layout.rs` (`setup_production_picker_scene` token fixture)
- Modify: `tests/presentation_app.rs` (`canonical_move_drives_visual_transform_without_renderer`, `token_health_bar_clips...`)

**Interfaces:**
- Consumes: Task 1 `UiAssets::map_sprite`, Task 2 helpers + `MAP_UNIT_*`.
- Produces: token root = `TokenCard + UnitVisual + ImageNode(map_sprite) + Node(96×96 bottom-center) + UiTransform + ZIndex(token_depth) + Pickable::default()`; `sync_token_cards` drives `ImageNode.color` alpha.

- [ ] **Step 1: Update tests first.**

`tests/presentation_app.rs` — in `canonical_move_drives_visual_transform_without_renderer`, replace the two geometry assertions with helper values and spawn an `ImageNode` on the probe:

```rust
    let root = scorpius::presentation::layout::unit_root_top_left(GridPos::new(1, 2));
    assert_eq!(node.left, px(root.x));
    assert_eq!(node.top, px(root.y));
```

Add a new test in the same file asserting the alpha contract (spawn a `TokenCard(UnitId(1))` root with `ImageNode::default()`, run `sync_token_cards` via a one-system app with `BattleRuntime(viability_fixture())`; assert `ImageNode.color` alpha `1.0` while unfinished; set `activation.finished`-equivalent by using a unit whose activation is finished — the viability fixture unit is unfinished, so also cover the finished case by mutating the domain unit's activation through a second fixture where you call `battle.finish_activation(UnitId(1))` (or `battle.wait()`/the existing finish API — use whatever `BattleState` exposes to finish an activation) before asserting alpha `0.52`).

In `token_health_bar_clips_full_and_partial_fill_to_sixty_two_pixels`: the fill now clips to **72** px — rename to `..._to_seventy_two_pixels` and update the expected widths (`full → 72.0`, partial scaled against 72).

`tests/ui_layout.rs` — in `setup_production_picker_scene`, replace the hardcoded token node with the authored geometry:

```rust
        let token_cell = battle.0.unit(unit_id).unwrap().position;
        let root = scorpius::presentation::layout::unit_root_top_left(token_cell);
        commands
            .spawn((
                TokenCard(unit_id),
                Pickable::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                Node {
                    width: Val::Px(scorpius::presentation::layout::MAP_UNIT_WIDTH),
                    height: Val::Px(scorpius::presentation::layout::MAP_UNIT_HEIGHT),
                    position_type: bevy::prelude::PositionType::Absolute,
                    left: Val::Px(root.x),
                    top: Val::Px(root.y),
                    ..Default::default()
                },
                ChildOf(stage),
            ))
```

- [ ] **Step 2: Run to verify failures** — `cargo test --test presentation_app --test ui_layout` → FAIL (old 76×64 values still implemented).

- [ ] **Step 3: Implement `battlefield.rs`.** In `spawn_token`:

1. Footprint + selection spawn: use `footprint_top_left(unit.position)` / `selection_top_left(unit.position)` for `left`/`top` (delete the local recomputation, keep sizes 112×96 / 112×56 and the `cy + 1` comment).
2. Replace the card root bundle with the sprite root (keep name, `UnitVisual`, `TokenCard`, observers, `e2e_id`, depth, `Pickable::default()`):

```rust
    let card = commands
        .spawn((
            Name::new(unit.name),
            UnitVisual(unit.id),
            TokenCard(unit.id),
            token_sprite_node(unit.position),
            UiTransform::IDENTITY,
            ImageNode::new(ui_assets.map_sprite(unit.archetype).clone()),
            Visibility::Visible,
            ZIndex(depth),
            Pickable::default(),
            ChildOf(stage),
        ))
        .observe(on_battlefield_token_click)
        .observe(on_battlefield_token_move)
        .observe(on_battlefield_token_out)
        .id();
```

with:

```rust
fn token_sprite_node(position: GridPos) -> Node {
    let root = unit_root_top_left(position);
    Node {
        position_type: PositionType::Absolute,
        left: px(root.x),
        top: px(root.y),
        width: px(MAP_UNIT_WIDTH),
        height: px(MAP_UNIT_HEIGHT),
        ..default()
    }
}
```

3. Remove the `BackgroundColor(...)` insert, the faction `style` glyph child for players, and the old glyph node. Spawn the glyph **only for enemies**, at the authored overlay slot:

```rust
    if unit.faction != Faction::Player {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(4.0),
                top: px(18.0),
                width: px(18.0),
                height: px(18.0),
                ..default()
            },
            theme::icon_node(ui_assets.icons.clone(), style.glyph_rect, style.color),
            Pickable::IGNORE,
            ChildOf(card),
        ));
    }
```

4. Re-layout the remaining children against the 96×96 root: HP bar outer `left 12, top -10, width 72, height 6` (keep dark `BackgroundColor` + `Overflow::clip()` + inner `TokenHpFill` mint/enemy fill, fill `height 6`); HP text `right 0, top -30` (keep font/colors); awaiting dot `left 4, top 4, 9×9` (keep `TokenAwaiting` + GOLD). All children stay `Pickable::IGNORE`.

- [ ] **Step 4: Implement `sync.rs`.**

In `apply_unit_transforms`: unit arm uses `node.left = px(root.x); node.top = px(root.y);` from `unit_root_top_left(unit.position)`; footprint arm uses `footprint_top_left`; selection arm uses `selection_top_left`.

`sync_token_cards`: replace the `BackgroundColor` query and loop with:

```rust
    mut cards: Query<(&TokenCard, &mut ImageNode)>,
```

```rust
    for (card, mut image) in &mut cards {
        if let Some(unit) = battle.0.unit(card.0) {
            let faded = unit.activation.finished && unit.faction == Faction::Player;
            image.color = Color::WHITE.with_alpha(if faded { 0.52 } else { 1.0 });
        }
    }
```

Update imports: drop `TOKEN_WIDTH`/`TOKEN_HEIGHT`, add `unit_root_top_left`, `footprint_top_left`, `selection_top_left`; `Faction` import stays.

- [ ] **Step 5: Implement `playback.rs` geometry.**

```rust
fn node_position(position: crate::domain::board::GridPos) -> Vec2 {
    unit_root_top_left(position)
}

fn footprint_position(position: crate::domain::board::GridPos) -> Vec2 {
    footprint_top_left(position)
}

fn selection_position(position: crate::domain::board::GridPos) -> Vec2 {
    selection_top_left(position)
}
```

`rendered_stage_center`: reconstruct the center from the root geometry:

```rust
        match (node.left, node.top) {
            (Val::Px(left), Val::Px(top)) => Some(Vec2::new(
                left + MAP_UNIT_WIDTH * 0.5,
                top + MAP_UNIT_HEIGHT,
            )),
            _ => None,
        }
```

Damage-number origin: `let origin = center + Vec2::new(0.0, -MAP_UNIT_HEIGHT - 10.0);` (both in `play_battle_events` and the inline test around line 776 that asserts `iso_center(rendered_cell) + Vec2::new(0.0, -MAP_UNIT_HEIGHT - 10.0)`).

- [ ] **Step 6: Delete the old constants.** Remove `TOKEN_WIDTH`/`TOKEN_HEIGHT` from `layout.rs` and fix every import (`battlefield.rs`, `sync.rs`, `playback.rs`). `grep -rn "TOKEN_WIDTH\|TOKEN_HEIGHT" src tests` must return nothing.

- [ ] **Step 7: Verify green + gates.** `cargo test --all-targets`, `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings` all PASS. Spot-check: `cargo run` is NOT required (renderer); the `ui_snapshots` and `campaign_flow` suites must be untouched-green.

- [ ] **Step 8: Commit** — `git add -A && git commit -m "feat: render tactical units as 96px map sprites and remove card chrome"`

---

### Task 4: Mode-aware token targeting — overhang stops blocking diamonds

**Files:**
- Modify: `src/presentation/interaction.rs` (`route_token_click`, `on_battlefield_token_click`, `on_battlefield_token_move`, `on_battlefield_token_out`, new `token_root_point_from_hit`, inline tests)
- Modify: `tests/ui_layout.rs` (hover test points)

**Interfaces:**
- Consumes: Task 2 `unit_root_top_left`, `MAP_UNIT_*`; existing `grid_from_stage_point`, `route_cell_click`.
- Produces: `pub fn token_root_point_from_hit(hit: &HitData) -> Option<Vec2>` (root-local px); `route_token_click(battle, interaction, unit_id, root_local: Option<Vec2>)`.

- [ ] **Step 1: Write failing tests** in `interaction.rs` inline `mod tests` (it already exists; add):

```rust
    #[test]
    fn targeting_token_hit_routes_the_underlying_diamond() {
        let mut battle = BattleState::viability_fixture();
        let mut interaction = InteractionState::default();
        interaction.mode = InteractionMode::Move;
        // Unit at (1,1); a hit 32px above its center lands on diamond (0,0).
        let events = route_token_click(&mut battle, &mut interaction, UnitId(1), Some(Vec2::new(48.0, 64.0)))
            .expect("converted hit must route a legal move");
        assert_eq!(battle.unit(UnitId(1)).unwrap().position, GridPos::new(0, 0));
        assert_eq!(interaction.hovered_cell, Some(GridPos::new(0, 0)));
        assert!(events.iter().any(|e| matches!(e, crate::domain::model::BattleEvent::UnitMoved { .. })));
    }

    #[test]
    fn inspect_token_hit_still_inspects_the_unit_itself() {
        let mut battle = BattleState::viability_fixture();
        let mut interaction = InteractionState::default();
        route_token_click(&mut battle, &mut interaction, UnitId(1), Some(Vec2::new(48.0, 64.0)))
            .unwrap();
        assert_eq!(interaction.inspected_unit, Some(UnitId(1)));
        assert_eq!(interaction.hovered_cell, Some(GridPos::new(1, 1)));
    }

    #[test]
    fn nine_by_nine_overhang_conversion_matches_the_pr_example() {
        // Striker stands at (4,4); a point 80px above its tile center must
        // resolve through the authored diamond resolver to (3,3).
        let root = unit_root_top_left(GridPos::new(4, 4));
        assert_eq!(
            grid_from_stage_point(root + Vec2::new(48.0, 16.0)),
            Some(GridPos::new(3, 3))
        );
    }
```

Import what the tests need (`unit_root_top_left`, `grid_from_stage_point`, `GridPos` — most are already in scope).

`tests/ui_layout.rs`: in `production_stage_observers_route_blockers_tokens_and_targets_once`, the token hover point `(0.0, -36.0)` now resolves to the diamond behind the mech. Change it to a feet-zone point `(0.0, -10.0)` so the existing `hovered_cell == striker_cell` assertion still holds, and add one overhang assertion after it:

```rust
    // Head-zone hover resolves the diamond behind the mech, not the mech cell.
    let behind = striker_cell.x.checked_sub(1).zip(striker_cell.y.checked_sub(1))
        .map(|(x, y)| GridPos::new(x, y));
    if let Some(behind_cell) = behind {
        let overhang = fit.offset + (iso_center(striker_cell) + Vec2::new(0.0, -80.0)) * fit.scale;
        send_headless_pointer_move(&mut app, window, overhang);
        app.update();
        assert_eq!(
            app.world().resource::<InteractionState>().hovered_cell,
            Some(behind_cell),
            "token overhang must hover the underlying diamond",
        );
    }
```

- [ ] **Step 2: Run to verify failures** — `cargo test --lib presentation::interaction` and `cargo test --test ui_layout` FAIL (signature/behavior unchanged).

- [ ] **Step 3: Implement.** In `interaction.rs`:

```rust
/// Token-local hit position as px inside the authored 96x96 root. Mirrors
/// `stage_point_from_hit` but scales by the unit root instead of the stage.
pub fn token_root_point_from_hit(hit: &HitData) -> Option<Vec2> {
    let normalized = hit.position?.truncate();
    if !normalized.is_finite() {
        return None;
    }
    Some(
        (normalized + Vec2::splat(0.5)) * Vec2::new(MAP_UNIT_WIDTH, MAP_UNIT_HEIGHT),
    )
}
```

```rust
/// Inspect a token using its domain ID; in targeting modes convert the
/// token-local hit to a stage point and route the diamond under the cursor
/// so sprite overhang never blocks Move/Attack/Aegis targets.
pub fn route_token_click(
    battle: &mut BattleState,
    interaction: &mut InteractionState,
    unit_id: UnitId,
    root_local: Option<Vec2>,
) -> Result<Vec<BattleEvent>, BattleError> {
    let position = battle
        .unit(unit_id)
        .ok_or(BattleError::UnknownUnit(unit_id))?
        .position;
    let underlying = root_local
        .and_then(|local| grid_from_stage_point(unit_root_top_left(position) + local));
    match (interaction.mode, underlying) {
        (InteractionMode::Inspect, _) | (_, None) => {
            route_cell_click(battle, interaction, position)
        }
        (_, Some(cell)) => route_cell_click(battle, interaction, cell),
    }
}
```

`on_battlefield_token_click`: pass `token_root_point_from_hit(&click.event.hit)` as the new argument. `on_battlefield_token_move`: replace the domain-position hover with the converted diamond:

```rust
    let cell = token_root_point_from_hit(&event.event.hit)
        .and_then(|local| grid_from_stage_point(unit_root_top_left(position) + local))
        .unwrap_or(position);
    update_hover_preview(&battle.0, &mut interaction, cell);
```

`on_battlefield_token_out`: clear unconditionally (hover can now sit on a diamond that is not the token's own domain cell, so the old equality guard would leak stale previews):

```rust
    interaction.hovered_cell = None;
    interaction.preview = None;
    preview_cells.0.clear();
```

Update the doc comment on `stage_point_from_hit` only if it still reads stale after this change.

- [ ] **Step 4: Verify green + gates.** `cargo test --all-targets`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt` PASS. If any `ui_interaction`/`presentation_app` test pinned the old token-out guard, update it to the unconditional clear and say so in the report.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: route targeting token hits through the underlying diamond"`

---

### Task 5: Grounded playback scale effects

**Files:**
- Modify: `src/presentation/playback.rs` (`animate_unit_event`, new `set_unit_scale`, inline tests)

**Interfaces:**
- Consumes: Task 2 `MAP_UNIT_HEIGHT`.
- Produces: `fn set_unit_scale(transform: &mut UiTransform, scale: f32)` — private.

**Ruling (controller):** the PR body sketches `translation.y = (1.0 - scale) * MAP_UNIT_HEIGHT * 0.5`, but Bevy UI scales around the node center, and the 0.5 factor pins the **center**, not the bottom: at scale 0.9 the feet would lift 4.8px and the knockout shrink would hover ~47px above the shadow. The PR's own acceptance criterion ("pulses and knockout shrink keep the feet grounded on the shadow", "bottom edge fixed") requires `(1.0 - scale) * MAP_UNIT_HEIGHT`. Implement with the full factor.

- [ ] **Step 1: Write failing tests** in `playback.rs` inline `mod tests`:

```rust
    #[test]
    fn unit_scale_effects_keep_the_ninety_six_pixel_bottom_edge_fixed() {
        for scale in [1.10, 1.16, 1.12, 0.02, 1.0] {
            let mut transform = UiTransform::IDENTITY;
            set_unit_scale(&mut transform, scale);
            assert_eq!(transform.scale, Vec2::splat(scale));
            assert_eq!(transform.translation.y, px((1.0 - scale) * MAP_UNIT_HEIGHT));
            // bottom = top + height * scale + translation.y stays at cy for
            // top = cy - MAP_UNIT_HEIGHT:
            let bottom = -MAP_UNIT_HEIGHT + MAP_UNIT_HEIGHT * scale
                + (1.0 - scale) * MAP_UNIT_HEIGHT;
            assert!((bottom - 0.0).abs() < 1e-4, "scale {scale} lifts the feet");
        }
    }
```

- [ ] **Step 2: Run to verify it fails** — `cargo test --lib presentation::playback` FAIL (`set_unit_scale` undefined).

- [ ] **Step 3: Implement.**

```rust
/// Scale a feet-anchored unit root. Bevy UI scales around the node center,
/// so compensate the Y translation to keep the 96px bottom edge (the feet)
/// planted on the shadow while the body pulses or shrinks.
fn set_unit_scale(transform: &mut UiTransform, scale: f32) {
    transform.scale = Vec2::splat(scale);
    transform.translation.y = px((1.0 - scale) * MAP_UNIT_HEIGHT);
}
```

In `animate_unit_event`, replace the four `transform.scale = Vec2::splat(...)` sites for units:
- `AttackRolled` attacker: `set_unit_scale(&mut transform, attack_scale(progress));`
- `AttackRolled` target on hit: `set_unit_scale(&mut transform, UNIT_SCALE * (1.0 + pulse * 0.16));`
- `CounterFired` defender: `set_unit_scale(&mut transform, UNIT_SCALE * (1.0 + pulse * 0.12));`
- `UnitKnockedOut`: `set_unit_scale(&mut transform, UNIT_SCALE * (1.0 - eased).max(0.02));`

Leave `DamageApplied`'s `translation.x` shake and the `EventEffect` sibling scaling untouched.

- [ ] **Step 4: Verify green + gates.** `cargo test --lib presentation::playback`, then `cargo test --all-targets`, clippy, fmt PASS.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: keep map-sprite scale effects anchored at the feet"`

---

### Task 6: Spec parity, gates, and native captures

**Files:**
- Modify: `docs/superpowers/specs/2026-09-05-hpa-480-ui-visual-parity-design.md` (constants table ~lines 105-106, prose lines ~143, ~173, ~294)
- Create: `docs/validation/pr-8-map-sprites.md`

**Interfaces:**
- Consumes: everything above, merged.

- [ ] **Step 1: Update HPA-480 §4.2.** In the constants table replace the `TOKEN_WIDTH = 76` / `TOKEN_HEIGHT = 64` rows with:

```text
MAP_UNIT_WIDTH  = 96
MAP_UNIT_HEIGHT = 96
```

Replace "Upright unit tokens are 76×64, with a footprint/shadow beneath." with "Tactical units are 96×96 SD map sprites, bottom-centered on their tile with the footprint/shadow beneath." Update the upright-token observer sentence (~173) to: "unit sprite roots use ordinary UI observers; Inspect clicks route by `UnitId`, targeting clicks convert the token-local hit to a stage point and resolve the underlying diamond through `route_cell_click`." Update the feature table row (~294) "upright tokens" → "SD map sprites".

- [ ] **Step 2: Write the validation note** `docs/validation/pr-8-map-sprites.md`: gates run + results, the three captures with file paths, the ~8px horizontal clearance note for the `battle.cell.4.8` E2E click vs Gunner's 96px root, and the scale-formula ruling from Task 5.

- [ ] **Step 3: Run all gates.** `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `cargo build --release` — all PASS. Then, if a display is available on this machine: `cargo test --features e2e --test e2e -- --test-threads=1`. If E2E needs CI's xvfb and cannot run locally, record that in the validation note and rely on CI.

- [ ] **Step 4: Native captures** (opt-in renderer fixture):

```bash
cargo build --features ui-capture --example ui_capture
for s in battle-idle battle-active-vanguard; do
  target/debug/examples/ui_capture --scenario $s --size 1920x1080 --seed 7 --time-ms 0 \
    --output target/ui-capture/$s.png
done
target/debug/examples/ui_capture --scenario battle-playback --size 1920x1080 --seed 7 --time-ms 150 \
  --output target/ui-capture/battle-playback.png
```

If no display/GPU is available, mark this step SKIPPED-NO-DISPLAY in the validation note — do not fake evidence.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "docs: update HPA-480 for map sprites and record PR-8 validation"`

---

## Self-Review

- Spec coverage: §1 assets → Task 1; §2 atomic root/sprite/chrome → Task 3; §3 targeting transparency → Task 4; §4 shared helpers + flat siblings → Tasks 2/3; §5 overlays → Task 3; §6 image alpha → Task 3; §7 playback geometry + grounded scale → Tasks 3/5; §8 tests/gates/captures + HPA-480 update → Tasks 3-6. ✓
- Known deliberate deviations (recorded as rulings): scale helper uses the full-height factor (Task 5); token-out clears unconditionally (Task 4); the six-fixture count from the PR matches the six sites found. ✓
- Type consistency: `map_sprite` used verbatim in Task 3; helpers' names identical across Tasks 2-4. ✓
