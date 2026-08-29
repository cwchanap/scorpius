use scorpius::{
    campaign::model::{CampaignState, SquadUpgrades, UpgradeLevels},
    mission::{MissionId, mission_definition},
};

#[test]
fn new_game_and_mission_one_definition_are_locked() {
    let state = CampaignState::new_game();
    assert_eq!(state.next_mission, MissionId::One);
    assert_eq!(state.credits, 0);
    assert_eq!(state.upgrades.vanguard, UpgradeLevels::default());
    assert_eq!(state.upgrades.gunner, UpgradeLevels::default());
    assert_eq!(state.upgrades.interceptor, UpgradeLevels::default());

    let definition = mission_definition(MissionId::One).unwrap();
    assert_eq!(definition.id, MissionId::One);
    assert_eq!(definition.unlocks, MissionId::Two);
    assert_eq!(definition.title, "Mission 1 — Turnabout at Relay Nine");
    assert_eq!(definition.primary_objective, "Eliminate all enemies.");
    assert_eq!(definition.base_reward, 300);
    assert_eq!(definition.optional_reward, 100);
    assert_eq!(definition.pre_mission.lines.len(), 3);
    assert_eq!(definition.aftermath.lines.len(), 2);

    let battle = (definition.build)(7, &SquadUpgrades::default());
    assert_eq!(battle.board().width(), 9);
    assert_eq!(battle.board().height(), 9);
    let two = mission_definition(MissionId::Two).unwrap();
    assert_eq!(two.unlocks, MissionId::Three);
    assert_eq!(two.base_reward, 400);
    let three = mission_definition(MissionId::Three).unwrap();
    assert_eq!(three.unlocks, MissionId::Four);
    assert_eq!(three.base_reward, 500);
    assert_eq!(mission_definition(MissionId::Four), None);
}
