mod shared;

pub mod aftermath;
pub mod briefing;
pub mod dialogue;
pub mod ending;
pub mod hangar;
pub mod mission_select;
pub mod title;

pub use aftermath::setup_aftermath_screen;
pub use briefing::setup_briefing_screen;
pub use dialogue::setup_pre_mission_story;
pub use ending::setup_ending_screen;
pub use hangar::setup_upgrade_screen;
pub use mission_select::setup_mission_select_screen;
pub use title::setup_title_screen;

pub use super::campaign_ui::update_upgrade_screen;
