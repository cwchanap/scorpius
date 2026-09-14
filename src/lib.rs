pub mod app;
pub mod campaign;
pub mod domain;
pub mod mission;
pub mod presentation;

use bevy::prelude::*;

pub fn run() {
    let mut app = App::new();
    app.add_plugins(app::ScorpiusPlugin);
    #[cfg(feature = "e2e")]
    app.add_plugins(bevy_e2e::BevyE2EPlugin);
    app.run();
}
