use crate::domain::model::UnitArchetype;
use bevy::prelude::*;

use super::theme;

pub const KEY_ART_PATH: &str = "ui/key_art.png";
pub const BRIEFING_ART_PATH: &str = "ui/briefing.png";
pub const VANGUARD_ART_PATH: &str = "ui/vanguard.png";
pub const GUNNER_ART_PATH: &str = "ui/gunner.png";
pub const INTERCEPTOR_ART_PATH: &str = "ui/interceptor.png";
pub const VANGUARD_MAP_PATH: &str = "ui/map/vanguard.png";
pub const GUNNER_MAP_PATH: &str = "ui/map/gunner.png";
pub const INTERCEPTOR_MAP_PATH: &str = "ui/map/interceptor.png";
pub const ENEMY_MAP_PATH: &str = "ui/map/enemy.png";

/// Handles for every asset used by the native campaign and battle UI.
/// `AssetLoadStatus` is the single readiness/error gate for presentation.
#[derive(Resource)]
pub struct UiAssets {
    pub key_art: Handle<Image>,
    pub briefing_art: Handle<Image>,
    pub vanguard_art: Handle<Image>,
    pub gunner_art: Handle<Image>,
    pub interceptor_art: Handle<Image>,
    pub vanguard_map: Handle<Image>,
    pub gunner_map: Handle<Image>,
    pub interceptor_map: Handle<Image>,
    pub enemy_map: Handle<Image>,
    pub icons: Handle<Image>,
    pub board: Handle<Image>,
    pub terrain: Handle<Image>,
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
            vanguard_map: asset_server.load(VANGUARD_MAP_PATH),
            gunner_map: asset_server.load(GUNNER_MAP_PATH),
            interceptor_map: asset_server.load(INTERCEPTOR_MAP_PATH),
            enemy_map: asset_server.load(ENEMY_MAP_PATH),
            icons: asset_server.load(theme::ICON_ATLAS_PATH),
            board: asset_server.load(theme::BOARD_ATLAS_PATH),
            terrain: asset_server.load(theme::TERRAIN_ATLAS_PATH),
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

    fn images(&self) -> [(&'static str, &Handle<Image>); 12] {
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
            (theme::TERRAIN_ATLAS_PATH, &self.terrain),
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
            .find(|(_, handle)| asset_failed(asset_server, handle))
            .map(|(path, _)| path)
            .or_else(|| {
                self.fonts()
                    .into_iter()
                    .find(|(_, handle)| asset_failed(asset_server, handle))
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

fn asset_failed<T: Asset>(asset_server: &AssetServer, handle: &Handle<T>) -> bool {
    asset_server.load_state(handle.id()).is_failed()
        || asset_server.dependency_load_state(handle.id()).is_failed()
        || asset_server
            .recursive_dependency_load_state(handle.id())
            .is_failed()
}

#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub enum AssetLoadStatus {
    #[default]
    Loading,
    Ready,
    Failed(&'static str),
}

/// Poll the one UI asset catalog. The optional resource keeps small headless
/// fixtures able to run this system without constructing the full app.
pub fn monitor_mission_assets(
    asset_server: Res<AssetServer>,
    ui_assets: Option<Res<UiAssets>>,
    mut status: ResMut<AssetLoadStatus>,
) {
    if !matches!(*status, AssetLoadStatus::Loading) {
        return;
    }
    let Some(ui_assets) = ui_assets else {
        return;
    };
    if let Some(path) = ui_assets.failed_path(&asset_server) {
        error!("Failed to load {path}");
        *status = AssetLoadStatus::Failed(path);
    } else if ui_assets.is_ready(&asset_server) {
        *status = AssetLoadStatus::Ready;
    }
}

pub fn mission_assets_ready(status: &AssetLoadStatus) -> bool {
    matches!(*status, AssetLoadStatus::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            terrain: Handle::default(),
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
            assert!(
                std::ptr::eq(assets.map_sprite(archetype), &assets.enemy_map),
                "{archetype:?} must return the shared enemy map handle"
            );
        }
        assert!(
            std::ptr::eq(
                assets.map_sprite(UnitArchetype::Vanguard),
                &assets.vanguard_map
            ),
            "Vanguard must return its own map handle"
        );
        assert!(
            std::ptr::eq(assets.map_sprite(UnitArchetype::Gunner), &assets.gunner_map),
            "Gunner must return its own map handle"
        );
        assert!(
            std::ptr::eq(
                assets.map_sprite(UnitArchetype::Interceptor),
                &assets.interceptor_map
            ),
            "Interceptor must return its own map handle"
        );
    }

    #[test]
    fn image_readiness_gate_covers_all_twelve_images() {
        let assets = catalog();
        assert_eq!(assets.images().len(), 12);
    }

    #[test]
    fn from_world_loads_every_catalog_entry_through_the_asset_server() {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut().init_resource::<UiAssets>();
        let assets = app.world().resource::<UiAssets>();
        assert_eq!(assets.images().len(), 12);
        for (path, handle) in assets.images() {
            assert_eq!(
                handle
                    .path()
                    .map(|asset_path| asset_path.path().to_string_lossy().into_owned()),
                Some(path.to_owned()),
                "{path} must round-trip through the asset server",
            );
        }
    }
}
