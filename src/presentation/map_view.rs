use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};

use super::{
    AttackPreviewCells, BattleMap, BattleRuntime,
    assets::UiAssets,
    interaction::InteractionState,
    layout::{
        BATTLE_STAGE_SIZE, CanvasLayout, ISO_ORIGIN_STAGE, TILE_HEIGHT, TILE_WIDTH,
        battle_stage_rect, fractional_grid, grid_from_map_point, iso_center,
    },
    theme,
};
use crate::domain::{
    board::{BoardState, GridPos, Terrain},
    model::{Faction, UnitId},
};

const MINIMAP_SIZE: f32 = 176.0;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MapView {
    pub center: Vec2,
    pub zoom: f32,
    pub width: u8,
    pub height: u8,
}

impl MapView {
    pub fn new(board: &BoardState) -> Self {
        Self {
            center: BATTLE_STAGE_SIZE * 0.5,
            zoom: 1.0,
            width: board.width(),
            height: board.height(),
        }
    }

    pub fn is_regional(&self) -> bool {
        self.width > 9 || self.height > 9
    }

    pub fn to_stage(&self, point: Vec2) -> Vec2 {
        (point - self.center) * self.zoom + BATTLE_STAGE_SIZE * 0.5
    }

    pub fn to_map(&self, point: Vec2) -> Vec2 {
        (point - BATTLE_STAGE_SIZE * 0.5) / self.zoom + self.center
    }

    pub fn cell_at(&self, point: Vec2) -> Option<GridPos> {
        if !point.is_finite()
            || point.x < 0.0
            || point.y < 0.0
            || point.x >= BATTLE_STAGE_SIZE.x
            || point.y >= BATTLE_STAGE_SIZE.y
        {
            return None;
        }
        grid_from_map_point(self.to_map(point), self.width, self.height)
    }

    pub fn focus(&mut self, cell: GridPos) {
        self.center = iso_center(cell) - battle_stage_rect().min;
        self.clamp();
    }

    pub fn zoom_at(&mut self, point: Vec2, factor: f32) {
        let anchor = self.to_map(point);
        self.zoom = (self.zoom * factor).clamp(0.35, 1.5);
        self.center += anchor - self.to_map(point);
        self.clamp();
    }

    fn clamp(&mut self) {
        let grid = fractional_grid(self.center).clamp(
            Vec2::ZERO,
            Vec2::new(f32::from(self.width - 1), f32::from(self.height - 1)),
        );
        self.center = ISO_ORIGIN_STAGE
            + Vec2::new(
                (grid.x - grid.y) * TILE_WIDTH * 0.5,
                (grid.x + grid.y) * TILE_HEIGHT * 0.5,
            );
    }

    /// Only the screen's diamonds plus a tile margin receive entities.
    pub fn visible_cells(&self) -> Vec<GridPos> {
        let padding = 64.0 * self.zoom;
        let margin = Vec2::splat(padding);
        let corners = [
            -margin,
            Vec2::new(BATTLE_STAGE_SIZE.x + padding, -padding),
            BATTLE_STAGE_SIZE + margin,
            Vec2::new(-padding, BATTLE_STAGE_SIZE.y + padding),
        ];
        let grid = corners.map(|point| fractional_grid(self.to_map(point)));
        let min = grid
            .into_iter()
            .fold(Vec2::splat(f32::INFINITY), Vec2::min)
            .floor()
            .max(Vec2::ZERO);
        let max = grid
            .into_iter()
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max)
            .ceil()
            .min(Vec2::new(
                f32::from(self.width - 1),
                f32::from(self.height - 1),
            ));
        let mut cells = Vec::new();
        for y in min.y as i32..=max.y as i32 {
            for x in min.x as i32..=max.x as i32 {
                let cell = GridPos::new(x as u8, y as u8);
                let point = self.to_stage(iso_center(cell) - battle_stage_rect().min);
                if point.x >= -padding
                    && point.y >= -padding
                    && point.x <= BATTLE_STAGE_SIZE.x + padding
                    && point.y <= BATTLE_STAGE_SIZE.y + padding
                {
                    cells.push(cell);
                }
            }
        }
        cells
    }

    pub fn transform(&self) -> UiTransform {
        // UI scales around the node center, which is the viewport center.
        let offset = (BATTLE_STAGE_SIZE * 0.5 - self.center) * self.zoom;
        UiTransform {
            translation: Val2::new(px(offset.x), px(offset.y)),
            scale: Vec2::splat(self.zoom),
            ..default()
        }
    }
}

pub fn terrain_color(terrain: Terrain, cell: GridPos) -> Color {
    let light = (cell.x + cell.y).is_multiple_of(2);
    match terrain {
        Terrain::Plain => {
            if light {
                Color::srgb_u8(78, 105, 75)
            } else {
                Color::srgb_u8(72, 98, 70)
            }
        }
        Terrain::Road => Color::srgb_u8(156, 141, 103),
        Terrain::Forest => {
            if light {
                Color::srgb_u8(34, 76, 51)
            } else {
                Color::srgb_u8(30, 69, 46)
            }
        }
        Terrain::Sea => {
            if light {
                Color::srgb_u8(25, 81, 116)
            } else {
                Color::srgb_u8(22, 75, 110)
            }
        }
        Terrain::Mountain => Color::srgb_u8(105, 110, 115),
    }
}

#[derive(Component)]
pub(crate) struct MapReadout;
#[derive(Component)]
pub(crate) struct MinimapUnit(UnitId);
#[derive(Component)]
pub(crate) struct MinimapEdge(usize);
#[derive(Component, Clone, Copy)]
enum MapAction {
    ZoomIn,
    ZoomOut,
    Center,
}

pub fn spawn_map_controls(
    commands: &mut Commands,
    stage: Entity,
    assets: &UiAssets,
    board: &BoardState,
    battle: &BattleRuntime,
    images: &mut Assets<Image>,
) {
    let pixels: Vec<u8> = (0..board.height())
        .flat_map(|y| {
            (0..board.width()).flat_map(move |x| {
                let cell = GridPos::new(x, y);
                terrain_color(board.terrain_at(cell).unwrap(), cell)
                    .to_srgba()
                    .to_u8_array()
            })
        })
        .collect();
    let map_image = images.add(Image::new(
        Extent3d {
            width: u32::from(board.width()),
            height: u32::from(board.height()),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    let minimap = commands
        .spawn((
            Name::new("Regional minimap — click to navigate"),
            Node {
                position_type: PositionType::Absolute,
                right: px(16),
                top: px(16),
                width: px(MINIMAP_SIZE),
                height: px(MINIMAP_SIZE),
                overflow: Overflow::clip(),
                ..default()
            },
            ImageNode::new(map_image),
            Pickable::default(),
            ZIndex(2000),
            ChildOf(stage),
        ))
        .observe(on_minimap_click)
        .id();
    for unit in battle.0.units() {
        commands.spawn((
            MinimapUnit(unit.id),
            Node {
                position_type: PositionType::Absolute,
                width: px(5),
                height: px(5),
                ..default()
            },
            BackgroundColor(if unit.faction == Faction::Player {
                theme::MINT
            } else {
                theme::ENEMY
            }),
            ZIndex(2),
            Pickable::IGNORE,
            ChildOf(minimap),
        ));
    }
    for edge in 0..4 {
        commands.spawn((
            MinimapEdge(edge),
            Node {
                position_type: PositionType::Absolute,
                height: px(1.5),
                ..default()
            },
            UiTransform::IDENTITY,
            BackgroundColor(Color::WHITE),
            ZIndex(1),
            Pickable::IGNORE,
            ChildOf(minimap),
        ));
    }
    let controls = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(16),
                top: px(200),
                column_gap: px(4),
                ..default()
            },
            ZIndex(2000),
            Pickable::IGNORE,
            ChildOf(stage),
        ))
        .id();
    for (label, action) in [
        ("−", MapAction::ZoomOut),
        ("+", MapAction::ZoomIn),
        ("Center", MapAction::Center),
    ] {
        commands
            .spawn((
                Button,
                action,
                Text::new(label),
                theme::ibm_plex_mono(&assets.fonts, 16.0, FontWeight(500)),
                TextColor(theme::TEXT),
                Node {
                    padding: UiRect::axes(px(12), px(8)),
                    ..default()
                },
                BackgroundColor(theme::PANEL),
                ChildOf(controls),
            ))
            .observe(on_map_action);
    }
    commands.spawn((
        Text::new("Arrows / MMB: pan\nWheel: zoom\nHome: center"),
        theme::ibm_plex_mono(&assets.fonts, 14.0, FontWeight(400)),
        TextColor(theme::TEXT),
        Node {
            position_type: PositionType::Absolute,
            right: px(16),
            top: px(246),
            padding: UiRect::all(px(6)),
            ..default()
        },
        BackgroundColor(theme::PANEL),
        ZIndex(2000),
        Pickable::IGNORE,
        ChildOf(stage),
    ));
    commands.spawn((
        MapReadout,
        Text::new("128 × 128"),
        theme::ibm_plex_mono(&assets.fonts, 15.0, FontWeight(500)),
        TextColor(theme::TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            bottom: px(12),
            padding: UiRect::all(px(8)),
            ..default()
        },
        BackgroundColor(theme::PANEL),
        ZIndex(2000),
        Pickable::IGNORE,
        ChildOf(stage),
    ));
}

fn focus_unit(view: &mut MapView, battle: &BattleRuntime, interaction: &InteractionState) {
    let unit = battle
        .0
        .active_unit()
        .or(interaction.inspected_unit)
        .and_then(|id| battle.0.unit(id))
        .or_else(|| {
            battle
                .0
                .units()
                .find(|unit| unit.faction == Faction::Player && !unit.is_knocked_out())
        });
    if let Some(unit) = unit {
        view.focus(unit.position);
    }
}

fn on_minimap_click(
    mut click: On<Pointer<Click>>,
    mut view: ResMut<MapView>,
    mut interaction: ResMut<InteractionState>,
    mut preview: ResMut<AttackPreviewCells>,
) {
    click.propagate(false);
    if click.button != PointerButton::Primary {
        return;
    }
    let Some(hit) = click.hit.position else {
        return;
    };
    let normalized = hit.truncate() + Vec2::splat(0.5);
    if !normalized.is_finite() {
        return;
    }
    let x = (normalized.x * f32::from(view.width))
        .floor()
        .clamp(0.0, f32::from(view.width - 1)) as u8;
    let y = (normalized.y * f32::from(view.height))
        .floor()
        .clamp(0.0, f32::from(view.height - 1)) as u8;
    view.focus(GridPos::new(x, y));
    interaction.hovered_cell = None;
    interaction.preview = None;
    preview.0.clear();
}

fn on_map_action(
    mut click: On<Pointer<Click>>,
    buttons: Query<&MapAction>,
    mut view: ResMut<MapView>,
    battle: Res<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut preview: ResMut<AttackPreviewCells>,
) {
    click.propagate(false);
    if click.button != PointerButton::Primary {
        return;
    }
    match buttons.get(click.entity) {
        Ok(MapAction::ZoomIn) => view.zoom_at(BATTLE_STAGE_SIZE * 0.5, 1.25),
        Ok(MapAction::ZoomOut) => view.zoom_at(BATTLE_STAGE_SIZE * 0.5, 0.8),
        Ok(MapAction::Center) => focus_unit(&mut view, &battle, &interaction),
        Err(_) => (),
    }
    interaction.hovered_cell = None;
    interaction.preview = None;
    preview.0.clear();
}

#[allow(clippy::too_many_arguments)]
pub fn navigate_map(
    mut view: ResMut<MapView>,
    battle: Res<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mut last_cursor: Local<Option<Vec2>>,
    mut preview: ResMut<AttackPreviewCells>,
) {
    if !view.is_regional() {
        wheel.clear();
        return;
    }
    let before = *view;
    let axis = Vec2::new(
        i32::from(keyboard.pressed(KeyCode::ArrowRight)) as f32
            - i32::from(keyboard.pressed(KeyCode::ArrowLeft)) as f32,
        i32::from(keyboard.pressed(KeyCode::ArrowDown)) as f32
            - i32::from(keyboard.pressed(KeyCode::ArrowUp)) as f32,
    );
    let zoom = view.zoom;
    view.center += axis * (650.0 * time.delta_secs() / zoom);
    if keyboard.just_pressed(KeyCode::Home) {
        focus_unit(&mut view, &battle, &interaction);
    }
    let cursor = windows
        .iter()
        .next()
        .and_then(|window| {
            CanvasLayout::fit(window.resolution.size()).to_design(window.cursor_position()?)
        })
        .map(|point| point - battle_stage_rect().min);
    let inside = cursor.filter(|point| {
        point.x >= 0.0
            && point.y >= 0.0
            && point.x < BATTLE_STAGE_SIZE.x
            && point.y < BATTLE_STAGE_SIZE.y
    });
    if let Some(point) = inside {
        if mouse.pressed(MouseButton::Middle)
            && let Some(last) = *last_cursor
        {
            let zoom = view.zoom;
            view.center -= (point - last) / zoom;
        }
        for event in wheel.read() {
            let amount = event.y
                * if event.unit == MouseScrollUnit::Pixel {
                    0.005
                } else {
                    0.15
                };
            view.zoom_at(point, amount.clamp(-1.0, 1.0).exp());
        }
    } else {
        wheel.clear();
    }
    *last_cursor = inside;
    if *view != before {
        view.clamp();
        interaction.hovered_cell = None;
        interaction.preview = None;
        preview.0.clear();
    }
}

pub(crate) fn sync_map_view(
    view: Res<MapView>,
    battle: Res<BattleRuntime>,
    interaction: Res<InteractionState>,
    mut maps: Query<&mut UiTransform, (With<BattleMap>, Without<MinimapEdge>)>,
    mut edges: Query<(&MinimapEdge, &mut Node, &mut UiTransform), Without<MinimapUnit>>,
    mut units: Query<(&MinimapUnit, &mut Node, &mut Visibility), Without<MinimapEdge>>,
    mut readouts: Query<&mut Text, With<MapReadout>>,
) {
    for mut transform in &mut maps {
        *transform = view.transform();
    }
    let size = Vec2::new(f32::from(view.width), f32::from(view.height));
    let corners = [
        Vec2::ZERO,
        Vec2::new(BATTLE_STAGE_SIZE.x, 0.0),
        BATTLE_STAGE_SIZE,
        Vec2::new(0.0, BATTLE_STAGE_SIZE.y),
    ]
    .map(|point| (fractional_grid(view.to_map(point)) + Vec2::splat(0.5)) / size * MINIMAP_SIZE);
    for (edge, mut node, mut transform) in &mut edges {
        let Some((from, to)) = clip_minimap_segment(corners[edge.0], corners[(edge.0 + 1) % 4])
        else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let delta = to - from;
        let center = from.midpoint(to);
        node.left = px(center.x - delta.length() * 0.5);
        node.top = px(center.y - 0.75);
        node.width = px(delta.length());
        transform.rotation = Rot2::radians(delta.y.atan2(delta.x));
    }
    for (marker, mut node, mut visibility) in &mut units {
        if let Some(unit) = battle.0.unit(marker.0) {
            node.left = px((f32::from(unit.position.x) + 0.5) / size.x * MINIMAP_SIZE - 2.5);
            node.top = px((f32::from(unit.position.y) + 0.5) / size.y * MINIMAP_SIZE - 2.5);
            *visibility = if unit.is_knocked_out() {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
    for mut text in &mut readouts {
        text.0 = format!(
            "{} × {}  ·  {:.0}%{}",
            view.width,
            view.height,
            view.zoom * 100.0,
            interaction
                .hovered_cell
                .and_then(
                    |cell| battle.0.board().terrain_at(cell).map(|terrain| format!(
                        "  ·  {},{}  {} · {}",
                        cell.x,
                        cell.y,
                        terrain.name(),
                        battle
                            .0
                            .board()
                            .movement_cost(cell)
                            .map_or_else(|| "Impassable".to_owned(), |cost| format!("Move {cost}"))
                    ))
                )
                .unwrap_or_default()
        );
    }
}

// Clip geometry as well as the UI node: rotated lines can otherwise protrude
// past the minimap's overflow clip at the extreme corners of the region.
fn clip_minimap_segment(from: Vec2, to: Vec2) -> Option<(Vec2, Vec2)> {
    let delta = to - from;
    let (mut start, mut end) = (0.0_f32, 1.0_f32);
    for (p, q) in [
        (-delta.x, from.x - 1.0),
        (delta.x, MINIMAP_SIZE - 1.0 - from.x),
        (-delta.y, from.y - 1.0),
        (delta.y, MINIMAP_SIZE - 1.0 - from.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else if p < 0.0 {
            start = start.max(q / p);
        } else {
            end = end.min(q / p);
        }
    }
    (start <= end).then(|| (from + delta * start, from + delta * end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::mission_one::mission_one;

    #[test]
    fn regional_projection_culling_and_zoom_cover_far_edges() {
        let battle = mission_one(7);
        let mut view = MapView::new(battle.board());
        for cell in [
            GridPos::new(0, 0),
            GridPos::new(127, 0),
            GridPos::new(0, 127),
            GridPos::new(127, 127),
            GridPos::new(64, 64),
        ] {
            for zoom in [0.35, 0.7, 1.5] {
                view.zoom = zoom;
                view.focus(cell);
                assert_eq!(view.cell_at(BATTLE_STAGE_SIZE * 0.5), Some(cell));
                let visible = view.visible_cells();
                assert!(visible.contains(&cell));
                assert!(
                    visible.len() < 2500,
                    "viewport must not spawn all 16384 tiles"
                );
                assert!(visible.iter().all(|cell| battle.board().contains(*cell)));
            }
        }
        view.focus(GridPos::new(64, 64));
        view.zoom = 1.0;
        let anchor = Vec2::new(210.0, 300.0);
        let before = view.to_map(anchor);
        view.zoom_at(anchor, 0.8);
        assert!((view.to_map(anchor) - before).length() < 0.01);
        assert_eq!(view.cell_at(Vec2::new(-1.0, 10.0)), None);
        let (from, to) =
            clip_minimap_segment(Vec2::new(-10.0, 88.0), Vec2::new(200.0, 88.0)).unwrap();
        assert!((from - Vec2::new(1.0, 88.0)).length() < 0.001);
        assert!((to - Vec2::new(175.0, 88.0)).length() < 0.001);
        assert!(clip_minimap_segment(Vec2::new(-10.0, -10.0), Vec2::new(-5.0, 150.0)).is_none());
    }
}
