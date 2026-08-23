pub mod app;
pub mod domain;
pub mod mission;
pub mod presentation;

use bevy::prelude::*;

pub fn run() {
    App::new().add_plugins(app::ScorpiusPlugin).run();
}
