use std::array;

use bevy::prelude::*;

pub const MISSION_ONE_GLTF: &str = "models/mission_one.gltf";
pub const MISSION_ONE_GLTF_DISPLAY_PATH: &str = "assets/models/mission_one.gltf";
pub const MISSION_ONE_SCENE_COUNT: usize = 13;

#[derive(Resource)]
pub struct MissionAssets {
    scenes: [Handle<WorldAsset>; MISSION_ONE_SCENE_COUNT],
}

impl FromWorld for MissionAssets {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self {
            scenes: array::from_fn(|index| {
                asset_server.load(GltfAssetLabel::Scene(index).from_asset(MISSION_ONE_GLTF))
            }),
        }
    }
}

impl MissionAssets {
    pub fn scene(&self, index: usize) -> Handle<WorldAsset> {
        self.scenes[index].clone()
    }

    fn iter(&self) -> impl Iterator<Item = &Handle<WorldAsset>> {
        self.scenes.iter()
    }
}

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub enum AssetLoadStatus {
    #[default]
    Loading,
    Ready,
    Failed(&'static str),
}

pub fn monitor_mission_assets(
    asset_server: Res<AssetServer>,
    assets: Res<MissionAssets>,
    mut status: ResMut<AssetLoadStatus>,
) {
    if !matches!(*status, AssetLoadStatus::Loading) {
        return;
    }

    let failed = assets.iter().any(|handle| {
        asset_server.load_state(handle.id()).is_failed()
            || asset_server.dependency_load_state(handle.id()).is_failed()
            || asset_server
                .recursive_dependency_load_state(handle.id())
                .is_failed()
    });
    if failed {
        error!("Failed to load {MISSION_ONE_GLTF_DISPLAY_PATH}");
        *status = AssetLoadStatus::Failed(MISSION_ONE_GLTF_DISPLAY_PATH);
        return;
    }

    if assets
        .iter()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.id()))
    {
        *status = AssetLoadStatus::Ready;
    }
}

pub fn mission_assets_ready(status: Res<AssetLoadStatus>) -> bool {
    matches!(*status, AssetLoadStatus::Ready)
}

#[cfg(test)]
mod tests {
    fn mission_gltf() -> serde_json::Value {
        serde_json::from_str(include_str!("../../assets/models/mission_one.gltf"))
            .expect("mission glTF must be valid JSON")
    }

    /// The Flanker scene is authored directly in the checked-in glTF: it gains
    /// its own mesh/material instead of borrowing another unit's scene, and
    /// everything reuses the one embedded buffer — no new bin file.
    #[test]
    fn flanker_scene_is_authored_with_own_mesh_material_and_root_scale() {
        let gltf = mission_gltf();

        let scenes = gltf["scenes"].as_array().unwrap();
        assert_eq!(scenes.len(), 13);
        assert_eq!(scenes[10]["name"], "Flanker");

        let nodes = gltf["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 70);
        // Node 49 is the Flanker root carrying the authored 0.72 scale; the
        // part nodes 50-55 hang beneath it and all reuse the shared mesh.
        assert_eq!(scenes[10]["nodes"], serde_json::json!([49]));
        let root = &nodes[49];
        assert_eq!(root["scale"], serde_json::json!([0.72, 0.72, 0.72]));
        assert_eq!(
            root["children"],
            serde_json::json!([50, 51, 52, 53, 54, 55])
        );
        for (index, part) in nodes.iter().enumerate().skip(50).take(6) {
            assert_eq!(part["mesh"], 10, "node {index} must use mesh 10");
        }

        let meshes = gltf["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 13);
        assert_eq!(meshes[10]["name"], "Flanker Magenta");
        let primitive = &meshes[10]["primitives"][0];
        assert_eq!(primitive["material"], 10);
        // Existing shared cube accessors, exactly like every other mesh.
        assert_eq!(primitive["attributes"]["POSITION"], 0);
        assert_eq!(primitive["attributes"]["NORMAL"], 1);

        let materials = gltf["materials"].as_array().unwrap();
        assert_eq!(materials.len(), 13);
        assert_eq!(materials[10]["name"], "Flanker Magenta");

        assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
    }

    /// Bulwark and Controller scenes are appended to the same single-buffer
    /// glTF: each gains its own root (with an authored scale), six part
    /// children, mesh, and material, reusing the shared cube accessors.
    #[test]
    fn bulwark_and_controller_scenes_are_authored_with_own_meshes_and_roots() {
        let gltf = mission_gltf();

        let scenes = gltf["scenes"].as_array().unwrap();
        assert_eq!(scenes.len(), 13);
        assert_eq!(scenes[11]["name"], "Bulwark");
        assert_eq!(scenes[11]["nodes"], serde_json::json!([56]));

        let nodes = gltf["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 70);
        assert_eq!(nodes[56]["scale"], serde_json::json!([0.88, 0.88, 0.88]));
        assert_eq!(
            nodes[56]["children"],
            serde_json::json!([57, 58, 59, 60, 61, 62])
        );
        for (index, part) in nodes.iter().enumerate().skip(57).take(6) {
            assert_eq!(part["mesh"], 11, "node {index} must use mesh 11");
        }

        assert_eq!(scenes[12]["name"], "Controller");
        assert_eq!(scenes[12]["nodes"], serde_json::json!([63]));
        assert_eq!(nodes[63]["scale"], serde_json::json!([0.72, 0.72, 0.72]));
        assert_eq!(
            nodes[63]["children"],
            serde_json::json!([64, 65, 66, 67, 68, 69])
        );
        for (index, part) in nodes.iter().enumerate().skip(64) {
            assert_eq!(part["mesh"], 12, "node {index} must use mesh 12");
        }

        let meshes = gltf["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 13);
        for mesh_index in [11, 12] {
            let primitive = &meshes[mesh_index]["primitives"][0];
            // Existing shared cube accessors, exactly like every other mesh.
            assert_eq!(primitive["attributes"]["POSITION"], 0);
            assert_eq!(primitive["attributes"]["NORMAL"], 1);
        }

        let materials = gltf["materials"].as_array().unwrap();
        assert_eq!(materials.len(), 13);
        assert_eq!(materials[11]["name"], "Bulwark Ochre");
        assert_eq!(
            materials[11]["pbrMetallicRoughness"]["baseColorFactor"],
            serde_json::json!([0.78, 0.38, 0.08, 1.0])
        );
        assert_eq!(materials[12]["name"], "Controller Cyan");
        assert_eq!(
            materials[12]["pbrMetallicRoughness"]["baseColorFactor"],
            serde_json::json!([0.08, 0.72, 0.86, 1.0])
        );

        assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
    }
}
