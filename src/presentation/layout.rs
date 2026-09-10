use bevy::{prelude::*, window::PrimaryWindow};

use crate::domain::board::GridPos;

pub const DESIGN_SIZE: Vec2 = Vec2::new(1920.0, 1080.0);
pub const BATTLE_STAGE_SIZE: Vec2 = Vec2::new(1008.0, 764.0);
pub const TILE_WIDTH: f32 = 112.0;
pub const TILE_HEIGHT: f32 = 56.0;
pub const BLOCK_HEIGHT: f32 = 26.0;
pub const ISO_ORIGIN: Vec2 = Vec2::new(960.0, 394.0);
pub const ISO_ORIGIN_STAGE: Vec2 = Vec2::new(504.0, 190.0);
pub const TOKEN_WIDTH: f32 = 76.0;
pub const TOKEN_HEIGHT: f32 = 64.0;
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

    let absolute = battle_stage_rect().min + local;
    for y in 0..BATTLE_GRID_HEIGHT {
        for x in 0..BATTLE_GRID_WIDTH {
            let cell = GridPos::new(x, y);
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
