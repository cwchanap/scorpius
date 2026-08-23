use bevy::{camera::ScalingMode, prelude::*};

use crate::domain::{board::GridPos, model::UnitId};

use super::{
    CellVisual, UnitVisual, assets::vanguard_scene, grid_to_world,
    interaction::on_viability_cell_click,
};

pub fn viability_grid_cells() -> Vec<GridPos> {
    (0..3)
        .flat_map(|y| (0..3).map(move |x| GridPos::new(x, y)))
        .collect()
}

pub fn setup_viability_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        Camera3d::default(),
        MeshPickingCamera,
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 5.2,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(4.8, 5.5, 5.8).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 6_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.6, 0.0)),
    ));

    let light_tile = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.22, 0.28),
        perceptual_roughness: 0.78,
        ..default()
    });
    let dark_tile = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.12, 0.18),
        perceptual_roughness: 0.82,
        ..default()
    });
    let tile_mesh = meshes.add(Cuboid::new(0.92, 0.12, 0.92));

    for cell in viability_grid_cells() {
        let material = if (cell.x + cell.y) % 2 == 0 {
            light_tile.clone()
        } else {
            dark_tile.clone()
        };
        commands
            .spawn((
                Mesh3d(tile_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(grid_to_world(cell).x, 0.0, grid_to_world(cell).z),
                CellVisual(cell),
                Pickable::default(),
            ))
            .observe(on_viability_cell_click);
    }

    commands.spawn((
        WorldAssetRoot(vanguard_scene(&asset_server)),
        Transform::from_translation(grid_to_world(GridPos::new(1, 1)))
            .with_scale(Vec3::splat(0.72)),
        UnitVisual(UnitId(1)),
    ));
}
