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
    use std::{collections::BTreeSet, time::Duration};

    use bevy::{
        camera::NormalizedRenderTarget,
        input::{mouse::MouseScrollUnit, touch::TouchPhase},
        picking::{
            backend::HitData,
            events::Click,
            pointer::{Location, PointerId},
        },
    };

    use super::*;
    use crate::{
        domain::{battle::BattleState, combat::DamageSource},
        mission::mission_one::{ids, mission_one},
        presentation::{
            CellVisual,
            battlefield::{reconcile_visible_cells, setup_mission_scene},
        },
    };

    fn blank_ui_assets() -> UiAssets {
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
            terrain: Handle::default(),
            fonts: std::array::from_fn(|_| Handle::default()),
        }
    }

    fn scene_app(battle: BattleState) -> App {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(battle))
            .insert_resource(blank_ui_assets())
            .init_resource::<Assets<Image>>()
            .init_resource::<InteractionState>()
            .init_resource::<AttackPreviewCells>()
            .add_systems(Startup, setup_mission_scene);
        app.update();
        app
    }

    fn click_location() -> Location {
        Location {
            target: NormalizedRenderTarget::None {
                width: 1920,
                height: 1080,
            },
            position: Vec2::ZERO,
        }
    }

    fn trigger_click(app: &mut App, entity: Entity, button: PointerButton, hit: Option<Vec3>) {
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            click_location(),
            Click {
                button,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, hit, None),
                duration: Duration::ZERO,
                count: 1,
            },
            entity,
        ));
    }

    #[test]
    fn regional_scene_installs_view_minimap_controls_and_culled_cells() {
        let mut app = scene_app(mission_one(7));

        let view = *app.world().resource::<MapView>();
        assert!(view.is_regional());
        assert_eq!(view.zoom, 0.7);

        let unit_count = app.world().resource::<BattleRuntime>().0.units().count();
        let mut units = app.world_mut().query::<&MinimapUnit>();
        assert_eq!(units.iter(app.world()).count(), unit_count);
        let mut edges = app.world_mut().query::<&MinimapEdge>();
        assert_eq!(edges.iter(app.world()).count(), 4);
        let mut actions = app.world_mut().query::<&MapAction>();
        assert_eq!(actions.iter(app.world()).count(), 3);
        let mut readouts = app.world_mut().query::<&MapReadout>();
        assert_eq!(readouts.iter(app.world()).count(), 1);
        let mut maps = app.world_mut().query::<(&BattleMap, &UiTransform)>();
        let (_, transform) = maps.single(app.world()).expect("one battle map");
        assert_eq!(*transform, view.transform());
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            1,
            "the minimap builds one generated board image",
        );

        let cell_count = app
            .world_mut()
            .query::<&CellVisual>()
            .iter(app.world())
            .count();
        assert!(
            cell_count > 0 && cell_count < 2500,
            "only the viewport window of the 128x128 board spawns entities",
        );
    }

    #[test]
    fn minimap_clicks_focus_the_view_and_ignore_secondary_buttons() {
        let mut app = scene_app(mission_one(7));
        let minimap = {
            let mut edges = app.world_mut().query::<(&MinimapEdge, &ChildOf)>();
            edges
                .iter(app.world())
                .next()
                .map(|(_, child)| child.parent())
                .expect("minimap edge parent is the minimap")
        };
        let before = *app.world().resource::<MapView>();

        for (button, position) in [
            (PointerButton::Secondary, Some(Vec3::new(0.25, -0.25, 0.0))),
            (PointerButton::Primary, None),
            (PointerButton::Primary, Some(Vec3::splat(f32::NAN))),
        ] {
            trigger_click(&mut app, minimap, button, position);
        }
        assert_eq!(
            *app.world().resource::<MapView>(),
            before,
            "secondary buttons and degenerate hits must not move the view",
        );

        app.world_mut()
            .resource_mut::<InteractionState>()
            .hovered_cell = Some(GridPos::new(1, 1));
        trigger_click(
            &mut app,
            minimap,
            PointerButton::Primary,
            Some(Vec3::new(0.25, -0.25, 0.0)),
        );
        let view = *app.world().resource::<MapView>();
        assert_eq!(
            view.cell_at(BATTLE_STAGE_SIZE * 0.5),
            Some(GridPos::new(96, 32)),
            "a primary click focuses the clicked cell",
        );
        assert_eq!(
            app.world().resource::<InteractionState>().hovered_cell,
            None
        );
    }

    #[test]
    fn map_action_buttons_zoom_and_center_the_view() {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        let mut app = scene_app(battle);

        let button_for = |app: &mut App, label: &str| {
            let mut actions = app.world_mut().query::<(Entity, &Text, &MapAction)>();
            actions
                .iter(app.world())
                .find(|(_, text, _)| text.0 == label)
                .map(|(entity, ..)| entity)
                .expect("map action button exists")
        };

        let zoom_out = button_for(&mut app, "−");
        trigger_click(&mut app, zoom_out, PointerButton::Primary, Some(Vec3::ZERO));
        let zoomed_out = app.world().resource::<MapView>().zoom;
        assert!((zoomed_out - 0.7 * 0.8).abs() < 0.001);

        let zoom_in = button_for(&mut app, "+");
        trigger_click(&mut app, zoom_in, PointerButton::Primary, Some(Vec3::ZERO));
        let zoomed_in = app.world().resource::<MapView>().zoom;
        assert!((zoomed_in - zoomed_out * 1.25).abs() < 0.001);

        // Center focuses the first living player when nothing is active.
        let center = button_for(&mut app, "Center");
        trigger_click(&mut app, center, PointerButton::Primary, Some(Vec3::ZERO));
        let first_player = app
            .world()
            .resource::<BattleRuntime>()
            .0
            .units()
            .find(|unit| unit.faction == Faction::Player)
            .unwrap()
            .position;
        assert_eq!(
            app.world()
                .resource::<MapView>()
                .cell_at(BATTLE_STAGE_SIZE * 0.5),
            Some(first_player)
        );

        // An inspected unit wins over the fallback; an active unit wins over
        // inspection.
        app.world_mut()
            .resource_mut::<InteractionState>()
            .inspected_unit = Some(ids::STRIKER);
        trigger_click(&mut app, center, PointerButton::Primary, Some(Vec3::ZERO));
        let striker = app
            .world()
            .resource::<BattleRuntime>()
            .0
            .unit(ids::STRIKER)
            .unwrap()
            .position;
        assert_eq!(
            app.world()
                .resource::<MapView>()
                .cell_at(BATTLE_STAGE_SIZE * 0.5),
            Some(striker)
        );
        app.world_mut()
            .resource_mut::<BattleRuntime>()
            .0
            .begin_activation(ids::VANGUARD)
            .unwrap();
        trigger_click(&mut app, center, PointerButton::Primary, Some(Vec3::ZERO));
        let vanguard = app
            .world()
            .resource::<BattleRuntime>()
            .0
            .unit(ids::VANGUARD)
            .unwrap()
            .position;
        assert_eq!(
            app.world()
                .resource::<MapView>()
                .cell_at(BATTLE_STAGE_SIZE * 0.5),
            Some(vanguard)
        );

        // Secondary buttons are ignored entirely.
        trigger_click(
            &mut app,
            zoom_in,
            PointerButton::Secondary,
            Some(Vec3::ZERO),
        );
        assert_eq!(app.world().resource::<MapView>().zoom, zoomed_in);

        // A click on an entity without a MapAction is a no-op.
        let stray = app.world_mut().spawn_empty().observe(on_map_action).id();
        let before = *app.world().resource::<MapView>();
        trigger_click(&mut app, stray, PointerButton::Primary, Some(Vec3::ZERO));
        assert_eq!(*app.world().resource::<MapView>(), before);
    }

    #[test]
    fn navigate_map_pans_zooms_centers_and_ignores_non_regional_boards() {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        let mut app = App::new();
        app.insert_resource(BattleRuntime(battle))
            .insert_resource(blank_ui_assets())
            .init_resource::<Assets<Image>>()
            .init_resource::<InteractionState>()
            .init_resource::<AttackPreviewCells>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Time>()
            .add_message::<MouseWheel>()
            .add_systems(Startup, setup_mission_scene)
            .add_systems(Update, navigate_map);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (1920, 1080).into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        app.update();

        // Arrow keys pan with scaled delta time.
        let before = *app.world().resource::<MapView>();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.update();
        assert_ne!(
            *app.world().resource::<MapView>(),
            before,
            "arrow keys pan the regional map",
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ArrowRight);

        // The cursor inside the stage enables middle-drag and wheel zoom.
        let stage_center = battle_stage_rect().min + BATTLE_STAGE_SIZE * 0.5;
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(stage_center));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Middle);
        app.update();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(stage_center + Vec2::new(40.0, 20.0)));
        let dragged_from = *app.world().resource::<MapView>();
        app.update();
        let after_drag = *app.world().resource::<MapView>();
        assert_ne!(
            after_drag.center, dragged_from.center,
            "middle-drag pans by the cursor delta",
        );

        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window,
            phase: TouchPhase::Moved,
        });
        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: 12.0,
            window,
            phase: TouchPhase::Moved,
        });
        let zoom_before = after_drag.zoom;
        app.update();
        assert_ne!(
            app.world().resource::<MapView>().zoom,
            zoom_before,
            "wheel events zoom around the cursor",
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Middle);

        // Home centers the first living player; a parked cursor outside the
        // stage clears pending wheel input.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Home);
        app.update();
        let home_view = *app.world().resource::<MapView>();
        let first_player = app
            .world()
            .resource::<BattleRuntime>()
            .0
            .units()
            .find(|unit| unit.faction == Faction::Player)
            .unwrap()
            .position;
        assert_eq!(
            home_view.cell_at(BATTLE_STAGE_SIZE * 0.5),
            Some(first_player)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Home);
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(4.0, 4.0)));
        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 5.0,
            window,
            phase: TouchPhase::Moved,
        });
        app.update();
        assert_eq!(
            *app.world().resource::<MapView>(),
            home_view,
            "a cursor outside the stage drops wheel input",
        );

        // Non-regional boards ignore navigation entirely.
        let mut flat = App::new();
        flat.insert_resource(BattleRuntime(BattleState::viability_fixture()))
            .insert_resource(MapView::new(BattleState::viability_fixture().board()))
            .init_resource::<InteractionState>()
            .init_resource::<AttackPreviewCells>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Time>()
            .add_message::<MouseWheel>()
            .add_systems(Update, navigate_map);
        let flat_window = flat
            .world_mut()
            .spawn((
                Window {
                    resolution: (1920, 1080).into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        flat.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 3.0,
            window: flat_window,
            phase: TouchPhase::Moved,
        });
        flat.update();
        assert_eq!(
            *flat.world().resource::<MapView>(),
            MapView::new(BattleState::viability_fixture().board()),
            "flat boards keep the authored view",
        );
    }

    #[test]
    fn sync_map_view_projects_transform_edges_units_and_readout() {
        let mut battle = mission_one(7);
        battle.apply_direct_damage(
            ids::RIFLEMAN_LEFT,
            99,
            DamageSource::PlayerWeapon(ids::PILE_LANCE),
        );
        let mut app = scene_app(battle);
        // A marker for a unit that is not on the board keeps its last paint.
        app.world_mut().spawn((
            MinimapUnit(UnitId(99)),
            Node::default(),
            Visibility::Visible,
        ));
        app.add_systems(Update, sync_map_view);
        app.update();

        let view = *app.world().resource::<MapView>();
        let mut maps = app.world_mut().query::<(&BattleMap, &UiTransform)>();
        let (_, transform) = maps.single(app.world()).unwrap();
        assert_eq!(*transform, view.transform());

        // A corner focus pushes one minimap edge entirely out of the clip.
        app.world_mut().resource_mut::<MapView>().zoom = 1.5;
        app.world_mut()
            .resource_mut::<MapView>()
            .focus(GridPos::new(0, 0));
        app.update();
        let mut edges = app.world_mut().query::<(&MinimapEdge, &Node)>();
        assert!(
            edges
                .iter(app.world())
                .any(|(_, node)| node.display == Display::None),
            "an off-map stage edge is clipped out of the minimap",
        );

        let mut readouts = app.world_mut().query::<(&MapReadout, &Text)>();
        let (_, text) = readouts.single(app.world()).unwrap();
        assert_eq!(text.0, "128 × 128  ·  150%");

        // Knocked-out units hide their minimap dot.
        let marker = app
            .world_mut()
            .query::<(&MinimapUnit, &Visibility)>()
            .iter(app.world())
            .find(|(marker, _)| marker.0 == ids::RIFLEMAN_LEFT)
            .map(|(_, visibility)| *visibility)
            .expect("the downed rifleman still has a minimap marker");
        assert_eq!(marker, Visibility::Hidden);

        // The readout appends hovered terrain name and movement cost.
        app.world_mut()
            .resource_mut::<InteractionState>()
            .hovered_cell = Some(GridPos::new(20, 8));
        app.update();
        let mut readouts = app.world_mut().query::<(&MapReadout, &Text)>();
        let (_, text) = readouts.single(app.world()).unwrap();
        assert!(
            text.0.contains("Road") && text.0.contains("Move 1"),
            "readout reports road terrain: {}",
            text.0,
        );
        app.world_mut()
            .resource_mut::<InteractionState>()
            .hovered_cell = Some(GridPos::new(0, 40));
        app.update();
        let mut readouts = app.world_mut().query::<(&MapReadout, &Text)>();
        let (_, text) = readouts.single(app.world()).unwrap();
        assert!(
            text.0.contains("Sea") && text.0.contains("Impassable"),
            "readout reports impassable sea: {}",
            text.0,
        );
    }

    #[test]
    fn reconcile_visible_cells_diffs_spawned_and_despawned_tiles() {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(mission_one(7)))
            .insert_resource(blank_ui_assets())
            .init_resource::<Assets<Image>>()
            .init_resource::<InteractionState>()
            .add_systems(Startup, setup_mission_scene)
            .add_systems(Update, reconcile_visible_cells);
        app.update();
        let initial: BTreeSet<GridPos> = app
            .world_mut()
            .query::<&CellVisual>()
            .iter(app.world())
            .map(|cell| cell.0)
            .collect();
        assert!(!initial.is_empty());

        // A second update with the same view early-returns.
        app.update();

        app.world_mut()
            .resource_mut::<MapView>()
            .focus(GridPos::new(120, 120));
        app.update();
        let moved: BTreeSet<GridPos> = app
            .world_mut()
            .query::<&CellVisual>()
            .iter(app.world())
            .map(|cell| cell.0)
            .collect();
        assert!(moved.contains(&GridPos::new(120, 120)));
        assert!(
            initial.is_disjoint(&moved),
            "the culled window follows the focused corner",
        );

        // Without a BattleMap the reconcile step exits cleanly.
        let battle = mission_one(7);
        let view = MapView::new(battle.board());
        let mut bare = App::new();
        bare.insert_resource(view)
            .insert_resource(BattleRuntime(battle))
            .insert_resource(blank_ui_assets())
            .add_systems(Update, reconcile_visible_cells);
        bare.update();
        let mut cells = bare.world_mut().query::<&CellVisual>();
        assert_eq!(cells.iter(bare.world()).count(), 0);
    }

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
        // An axis-aligned segment outside the clip rejects before any
        // interpolation happens.
        assert!(clip_minimap_segment(Vec2::new(0.0, 10.0), Vec2::new(0.0, 50.0)).is_none());
    }

    #[test]
    fn terrain_color_distinguishes_every_regional_tile() {
        assert_eq!(
            terrain_color(Terrain::Plain, GridPos::new(0, 0)),
            Color::srgb_u8(78, 105, 75)
        );
        assert_eq!(
            terrain_color(Terrain::Plain, GridPos::new(0, 1)),
            Color::srgb_u8(72, 98, 70)
        );
        assert_eq!(
            terrain_color(Terrain::Road, GridPos::new(3, 3)),
            Color::srgb_u8(156, 141, 103)
        );
        assert_eq!(
            terrain_color(Terrain::Forest, GridPos::new(0, 0)),
            Color::srgb_u8(34, 76, 51)
        );
        assert_eq!(
            terrain_color(Terrain::Forest, GridPos::new(1, 0)),
            Color::srgb_u8(30, 69, 46)
        );
        assert_eq!(
            terrain_color(Terrain::Sea, GridPos::new(0, 0)),
            Color::srgb_u8(25, 81, 116)
        );
        assert_eq!(
            terrain_color(Terrain::Sea, GridPos::new(2, 1)),
            Color::srgb_u8(22, 75, 110)
        );
        assert_eq!(
            terrain_color(Terrain::Mountain, GridPos::new(4, 4)),
            Color::srgb_u8(105, 110, 115)
        );
    }
}
