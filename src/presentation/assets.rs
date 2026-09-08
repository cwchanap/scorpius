use std::array;

use bevy::prelude::*;

use super::theme;

pub const MISSION_ONE_GLTF: &str = "models/mission_one.gltf";
pub const MISSION_ONE_GLTF_DISPLAY_PATH: &str = "assets/models/mission_one.gltf";
pub const MISSION_ONE_SCENE_COUNT: usize = 15;

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

pub const KEY_ART_PATH: &str = "ui/key_art.png";
pub const BRIEFING_ART_PATH: &str = "ui/briefing.png";
pub const VANGUARD_ART_PATH: &str = "ui/vanguard.png";
pub const GUNNER_ART_PATH: &str = "ui/gunner.png";
pub const INTERCEPTOR_ART_PATH: &str = "ui/interceptor.png";

/// Handles for the native UI assets. `AssetLoadStatus` remains the single
/// readiness gate; this resource only owns the typed handles it monitors.
#[derive(Resource)]
pub struct UiAssets {
    pub key_art: Handle<Image>,
    pub briefing_art: Handle<Image>,
    pub vanguard_art: Handle<Image>,
    pub gunner_art: Handle<Image>,
    pub interceptor_art: Handle<Image>,
    pub icons: Handle<Image>,
    pub board: Handle<Image>,
    pub fonts: [Handle<Font>; 7],
}

impl FromWorld for UiAssets {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self {
            key_art: asset_server.load(KEY_ART_PATH),
            briefing_art: asset_server.load(BRIEFING_ART_PATH),
            vanguard_art: asset_server.load(VANGUARD_ART_PATH),
            gunner_art: asset_server.load(GUNNER_ART_PATH),
            interceptor_art: asset_server.load(INTERCEPTOR_ART_PATH),
            icons: asset_server.load(theme::ICON_ATLAS_PATH),
            board: asset_server.load(theme::BOARD_ATLAS_PATH),
            fonts: [
                asset_server.load(theme::CHAKRA_PETCH_400_PATH),
                asset_server.load(theme::CHAKRA_PETCH_500_PATH),
                asset_server.load(theme::CHAKRA_PETCH_600_PATH),
                asset_server.load(theme::CHAKRA_PETCH_700_PATH),
                asset_server.load(theme::IBM_PLEX_MONO_400_PATH),
                asset_server.load(theme::IBM_PLEX_MONO_500_PATH),
                asset_server.load(theme::IBM_PLEX_MONO_600_PATH),
            ],
        }
    }
}

impl UiAssets {
    fn images(&self) -> [(&'static str, &Handle<Image>); 7] {
        [
            (KEY_ART_PATH, &self.key_art),
            (BRIEFING_ART_PATH, &self.briefing_art),
            (VANGUARD_ART_PATH, &self.vanguard_art),
            (GUNNER_ART_PATH, &self.gunner_art),
            (INTERCEPTOR_ART_PATH, &self.interceptor_art),
            (theme::ICON_ATLAS_PATH, &self.icons),
            (theme::BOARD_ATLAS_PATH, &self.board),
        ]
    }

    fn fonts(&self) -> [(&'static str, &Handle<Font>); 7] {
        [
            (theme::CHAKRA_PETCH_400_PATH, &self.fonts[0]),
            (theme::CHAKRA_PETCH_500_PATH, &self.fonts[1]),
            (theme::CHAKRA_PETCH_600_PATH, &self.fonts[2]),
            (theme::CHAKRA_PETCH_700_PATH, &self.fonts[3]),
            (theme::IBM_PLEX_MONO_400_PATH, &self.fonts[4]),
            (theme::IBM_PLEX_MONO_500_PATH, &self.fonts[5]),
            (theme::IBM_PLEX_MONO_600_PATH, &self.fonts[6]),
        ]
    }

    fn failed_path(&self, asset_server: &AssetServer) -> Option<&'static str> {
        self.images()
            .into_iter()
            .find(|(_, handle)| {
                asset_server.load_state(handle.id()).is_failed()
                    || asset_server.dependency_load_state(handle.id()).is_failed()
                    || asset_server
                        .recursive_dependency_load_state(handle.id())
                        .is_failed()
            })
            .map(|(path, _)| path)
            .or_else(|| {
                self.fonts()
                    .into_iter()
                    .find(|(_, handle)| {
                        asset_server.load_state(handle.id()).is_failed()
                            || asset_server.dependency_load_state(handle.id()).is_failed()
                            || asset_server
                                .recursive_dependency_load_state(handle.id())
                                .is_failed()
                    })
                    .map(|(path, _)| path)
            })
    }

    fn is_ready(&self, asset_server: &AssetServer) -> bool {
        self.images()
            .into_iter()
            .all(|(_, handle)| asset_server.is_loaded_with_dependencies(handle.id()))
            && self
                .fonts()
                .into_iter()
                .all(|(_, handle)| asset_server.is_loaded_with_dependencies(handle.id()))
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
    ui_assets: Option<Res<UiAssets>>,
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

    if let Some(ui_assets) = ui_assets.as_deref() {
        if let Some(path) = ui_assets.failed_path(&asset_server) {
            error!("Failed to load {path}");
            *status = AssetLoadStatus::Failed(path);
            return;
        }
        if !ui_assets.is_ready(&asset_server) {
            return;
        }
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
        assert_eq!(scenes.len(), 15);
        assert_eq!(scenes[10]["name"], "Flanker");

        let nodes = gltf["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 84);
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
        assert_eq!(meshes.len(), 15);
        assert_eq!(meshes[10]["name"], "Flanker Magenta");
        let primitive = &meshes[10]["primitives"][0];
        assert_eq!(primitive["material"], 10);
        // Existing shared cube accessors, exactly like every other mesh.
        assert_eq!(primitive["attributes"]["POSITION"], 0);
        assert_eq!(primitive["attributes"]["NORMAL"], 1);

        let materials = gltf["materials"].as_array().unwrap();
        assert_eq!(materials.len(), 15);
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
        assert_eq!(scenes.len(), 15);
        assert_eq!(scenes[11]["name"], "Bulwark");
        assert_eq!(scenes[11]["nodes"], serde_json::json!([56]));

        let nodes = gltf["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 84);
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
        for (index, part) in nodes.iter().enumerate().skip(64).take(6) {
            assert_eq!(part["mesh"], 12, "node {index} must use mesh 12");
        }

        let meshes = gltf["meshes"].as_array().unwrap();
        assert_eq!(meshes.len(), 15);
        for mesh_index in [11, 12] {
            let primitive = &meshes[mesh_index]["primitives"][0];
            // Existing shared cube accessors, exactly like every other mesh.
            assert_eq!(primitive["attributes"]["POSITION"], 0);
            assert_eq!(primitive["attributes"]["NORMAL"], 1);
        }

        let materials = gltf["materials"].as_array().unwrap();
        assert_eq!(materials.len(), 15);
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

    /// The Dreadnought scene is appended to the same single-buffer glTF: a
    /// larger crimson unit with its own root (authored 1.12 scale), six part
    /// children, mesh, and material, reusing the shared cube accessors.
    #[test]
    fn dreadnought_scene_is_authored_as_a_larger_crimson_unit() {
        let gltf = mission_gltf();
        let scenes = gltf["scenes"].as_array().unwrap();
        let nodes = gltf["nodes"].as_array().unwrap();
        let meshes = gltf["meshes"].as_array().unwrap();
        let materials = gltf["materials"].as_array().unwrap();

        assert_eq!(scenes.len(), 15);
        assert_eq!(scenes[13]["name"], "Dreadnought");
        assert_eq!(scenes[13]["nodes"], serde_json::json!([70]));
        assert_eq!(nodes.len(), 84);
        assert_eq!(nodes[70]["scale"], serde_json::json!([1.12, 1.12, 1.12]));
        assert_eq!(
            nodes[70]["children"],
            serde_json::json!([71, 72, 73, 74, 75, 76])
        );
        for (index, part) in nodes.iter().enumerate().skip(71).take(6) {
            assert_eq!(part["mesh"], 13, "node {index} must use mesh 13");
        }
        assert_eq!(meshes.len(), 15);
        assert_eq!(meshes[13]["name"], "Dreadnought Crimson");
        assert_eq!(meshes[13]["primitives"][0]["material"], 13);
        assert_eq!(materials.len(), 15);
        assert_eq!(materials[13]["name"], "Dreadnought Crimson");
        assert_eq!(
            materials[13]["pbrMetallicRoughness"]["baseColorFactor"],
            serde_json::json!([0.55, 0.08, 0.12, 1.0])
        );
        assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
    }

    /// The Regent scene is appended to the same single-buffer glTF as the
    /// final violet boss: its own root (authored 1.20 scale), six part
    /// children, mesh, and material, reusing the shared cube accessors.
    #[test]
    fn regent_scene_is_authored_as_the_final_violet_boss() {
        let gltf = mission_gltf();
        let scenes = gltf["scenes"].as_array().unwrap();
        let nodes = gltf["nodes"].as_array().unwrap();
        let meshes = gltf["meshes"].as_array().unwrap();
        let materials = gltf["materials"].as_array().unwrap();

        assert_eq!(scenes.len(), 15);
        assert_eq!(scenes[14]["name"], "Regent");
        assert_eq!(scenes[14]["nodes"], serde_json::json!([77]));
        assert_eq!(nodes.len(), 84);
        assert_eq!(nodes[77]["scale"], serde_json::json!([1.20, 1.20, 1.20]));
        assert_eq!(
            nodes[77]["children"],
            serde_json::json!([78, 79, 80, 81, 82, 83])
        );
        for (index, part) in nodes.iter().enumerate().skip(78).take(6) {
            assert_eq!(part["mesh"], 14, "node {index} must use mesh 14");
        }
        assert_eq!(meshes.len(), 15);
        assert_eq!(meshes[14]["name"], "Regent Violet");
        assert_eq!(meshes[14]["primitives"][0]["material"], 14);
        assert_eq!(materials.len(), 15);
        assert_eq!(materials[14]["name"], "Regent Violet");
        assert_eq!(
            materials[14]["pbrMetallicRoughness"]["baseColorFactor"],
            serde_json::json!([0.42, 0.14, 0.78, 1.0])
        );
        assert_eq!(gltf["buffers"].as_array().unwrap().len(), 1);
    }
}
