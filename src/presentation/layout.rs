use bevy::{prelude::*, window::PrimaryWindow};

use crate::domain::board::GridPos;

pub const DESIGN_SIZE: Vec2 = Vec2::new(1920.0, 1080.0);
pub const BATTLE_STAGE_SIZE: Vec2 = Vec2::new(1008.0, 764.0);
pub const TILE_WIDTH: f32 = 112.0;
pub const TILE_HEIGHT: f32 = 56.0;
pub const BLOCK_HEIGHT: f32 = 26.0;
pub const ISO_ORIGIN: Vec2 = Vec2::new(960.0, 394.0);
pub const ISO_ORIGIN_STAGE: Vec2 = Vec2::new(504.0, 190.0);
pub const BATTLE_GRID_WIDTH: u8 = 9;
pub const BATTLE_GRID_HEIGHT: u8 = 9;

/// The percent-sized window surface that owns the fitted design canvas.
#[derive(Component)]
pub struct ViewportRoot;

/// The fixed-pixel 1920×1080 design surface shared by every screen.
#[derive(Component)]
pub struct CanvasRoot;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasLayout {
    pub scale: f32,
    pub offset: Vec2,
}

impl CanvasLayout {
    pub fn fit(window_logical: Vec2) -> Self {
        let scale = (window_logical.x / DESIGN_SIZE.x)
            .min(window_logical.y / DESIGN_SIZE.y)
            .max(0.0);
        Self {
            scale,
            offset: (window_logical - DESIGN_SIZE * scale) * 0.5,
        }
    }

    /// Converts a logical window point into design pixels for capture and
    /// diagnostic assertions. Production board input uses UI picking's
    /// node-local coordinates instead.
    pub fn to_design(self, window_point: Vec2) -> Option<Vec2> {
        if self.scale <= 0.0 || !window_point.is_finite() {
            return None;
        }
        let size = DESIGN_SIZE * self.scale;
        let local = window_point - self.offset;
        if local.x < 0.0 || local.y < 0.0 || local.x > size.x || local.y > size.y {
            return None;
        }
        Some(local / self.scale)
    }
}

/// Spawn the one fixed design canvas used by all campaign and battle screens.
///
/// The viewport stays in logical window coordinates while `UiScale` scales the
/// fixed child. Flex centering therefore provides the letterbox offset without
/// a second hand-rolled transform path.
pub fn spawn_canvas_root(commands: &mut Commands) -> Entity {
    let viewport = commands
        .spawn((
            Name::new("Viewport Root"),
            ViewportRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Visible,
        ))
        .id();
    commands
        .spawn((
            Name::new("Canvas Root"),
            CanvasRoot,
            Node {
                width: Val::Px(DESIGN_SIZE.x),
                height: Val::Px(DESIGN_SIZE.y),
                ..default()
            },
            Visibility::Visible,
            ChildOf(viewport),
        ))
        .id()
}

/// Startup system for the shared viewport/canvas hierarchy.
pub fn setup_canvas(mut commands: Commands, canvas_roots: Query<Entity, With<CanvasRoot>>) {
    if canvas_roots.iter().next().is_none() {
        spawn_canvas_root(&mut commands);
    }
}

/// Keep fixed-pixel UI values fitted to the primary window's logical size.
///
/// This runs before Bevy's UI picking backend so the backend sees the current
/// scale after a resize in the same frame.
pub fn update_canvas_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    ui_scale.0 = CanvasLayout::fit(window.resolution.size()).scale;
}

pub const fn battle_stage_rect() -> Rect {
    Rect::from_corners(Vec2::new(456.0, 204.0), Vec2::new(1464.0, 968.0))
}

pub fn iso_center(pos: GridPos) -> Vec2 {
    ISO_ORIGIN
        + Vec2::new(
            (f32::from(pos.x) - f32::from(pos.y)) * (TILE_WIDTH * 0.5),
            (f32::from(pos.x) + f32::from(pos.y)) * (TILE_HEIGHT * 0.5),
        )
}

pub fn tile_bounds(pos: GridPos) -> Rect {
    let half = Vec2::new(TILE_WIDTH * 0.5, TILE_HEIGHT * 0.5);
    Rect::from_corners(iso_center(pos) - half, iso_center(pos) + half)
}

pub const fn depth_key(pos: GridPos) -> i16 {
    pos.x as i16 + pos.y as i16
}

/// Sibling `ZIndex` for the token card and selection ring at `pos`; the
/// footprint shadow sits one step below at `token_depth(pos) - 1`.
pub const fn token_depth(pos: GridPos) -> i32 {
    11 + (depth_key(pos) as i32) * 3
}

/// Foreground layer above the deepest token slice of a board this size;
/// transient combat feedback (impact icons, damage numbers) uses it so it
/// always overlays the stage stack, whatever the board dimensions.
pub fn stage_effect_depth(width: u8, height: u8) -> i32 {
    token_depth(GridPos::new(width - 1, height - 1)) + 1
}

/// Authored size of one tactical unit's map-sprite root. The sprite feet
/// land on the tile center; this is the single geometry for spawn, sync,
/// playback, and token-hit conversion.
pub const MAP_UNIT_WIDTH: f32 = 96.0;
pub const MAP_UNIT_HEIGHT: f32 = 96.0;

/// Stage-local top-left of a unit's 96x96 sprite root (bottom-center anchor).
pub fn unit_root_top_left(pos: GridPos) -> Vec2 {
    let center = iso_center(pos) - battle_stage_rect().min;
    Vec2::new(center.x - MAP_UNIT_WIDTH * 0.5, center.y - MAP_UNIT_HEIGHT)
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
    Vec2::new(center.x - TILE_WIDTH * 0.5, center.y - TILE_HEIGHT * 0.5)
}

/// Maps a stage-local pixel point to one of the authored 9×9 diamonds.
///
/// The inclusive edge check intentionally resolves a shared edge by the
/// stable y-then-x scan order, so a point can never produce two cells.
pub fn grid_from_stage_point(local: Vec2) -> Option<GridPos> {
    if !local.is_finite()
        || local.x < 0.0
        || local.y < 0.0
        || local.x >= BATTLE_STAGE_SIZE.x
        || local.y >= BATTLE_STAGE_SIZE.y
    {
        return None;
    }

    grid_from_map_point(local, BATTLE_GRID_WIDTH, BATTLE_GRID_HEIGHT)
}

/// Inverse projection checks at most nine diamonds, independent of board size.
/// Neighbors preserve the original y-then-x tie break on shared edges.
pub fn grid_from_map_point(local: Vec2, width: u8, height: u8) -> Option<GridPos> {
    if !local.is_finite() {
        return None;
    }
    let absolute = battle_stage_rect().min + local;
    let grid = fractional_grid(local);
    if grid.x < -1.0 || grid.y < -1.0 || grid.x > f32::from(width) || grid.y > f32::from(height) {
        return None;
    }
    let cx = grid.x.round() as i32;
    let cy = grid.y.round() as i32;
    for y in (cy - 1).max(0)..=(cy + 1).min(i32::from(height) - 1) {
        for x in (cx - 1).max(0)..=(cx + 1).min(i32::from(width) - 1) {
            let cell = GridPos::new(x as u8, y as u8);
            let delta = absolute - iso_center(cell);
            let diamond_distance =
                delta.x.abs() / (TILE_WIDTH * 0.5) + delta.y.abs() / (TILE_HEIGHT * 0.5);
            if diamond_distance <= 1.0 + f32::EPSILON * 8.0 {
                return Some(cell);
            }
        }
    }
    None
}

pub fn fractional_grid(local: Vec2) -> Vec2 {
    let delta = local - ISO_ORIGIN_STAGE;
    Vec2::new(
        delta.x / TILE_WIDTH + delta.y / TILE_HEIGHT,
        delta.y / TILE_HEIGHT - delta.x / TILE_WIDTH,
    )
}

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
        assert_eq!(
            footprint_top_left(pos),
            Vec2::new(center.x - 56.0, center.y - 68.0)
        );
        assert_eq!(
            selection_top_left(pos),
            Vec2::new(center.x - 56.0, center.y - 28.0)
        );
    }

    #[test]
    fn map_point_inverse_rejects_non_finite_and_far_outside_points() {
        assert_eq!(grid_from_map_point(Vec2::splat(f32::NAN), 128, 128), None);
        assert_eq!(
            grid_from_map_point(Vec2::splat(f32::INFINITY), 128, 128),
            None
        );
        // Inside the stage rect but past the far corner of a 128x128 grid.
        assert_eq!(
            grid_from_map_point(BATTLE_STAGE_SIZE - Vec2::ONE, 9, 9),
            None
        );
        // The regional board still resolves its own far corner.
        assert_eq!(
            grid_from_map_point(
                iso_center(GridPos::new(127, 127)) - battle_stage_rect().min,
                128,
                128,
            ),
            Some(GridPos::new(127, 127))
        );
    }
}
