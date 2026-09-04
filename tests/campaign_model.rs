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
    assert_eq!(definition.unlocks, Some(MissionId::Two));
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
    assert_eq!(two.unlocks, Some(MissionId::Three));
    assert_eq!(two.base_reward, 400);
    let three = mission_definition(MissionId::Three).unwrap();
    assert_eq!(three.unlocks, Some(MissionId::Four));
    assert_eq!(three.base_reward, 500);
    let four = mission_definition(MissionId::Four).unwrap();
    assert_eq!(four.id, MissionId::Four);
    assert_eq!(four.unlocks, Some(MissionId::Five));
    assert_eq!(four.title, "Mission 4 — Breach the Gate");
    assert_eq!(four.base_reward, 600);
    assert_eq!(four.optional_reward, 150);
    let five = mission_definition(MissionId::Five).unwrap();
    assert_eq!(five.id, MissionId::Five);
    assert_eq!(five.unlocks, Some(MissionId::Six));
    assert_eq!(five.title, "Mission 5 — Crossfire Break");
    assert_eq!(five.base_reward, 700);
    assert_eq!(five.optional_reward, 200);
    let six = mission_definition(MissionId::Six).unwrap();
    assert_eq!(six.unlocks, Some(MissionId::Seven));
    assert_eq!((six.base_reward, six.optional_reward), (800, 250));
    let seven = mission_definition(MissionId::Seven).unwrap();
    assert_eq!(seven.id, MissionId::Seven);
    assert_eq!(seven.unlocks, None);
    assert_eq!(seven.title, "Mission 7 - Last Command");
    assert_eq!(
        seven.primary_objective,
        "Destroy the Regent and break the command net."
    );
    assert_eq!(
        seven.optional_objective,
        "Final Push: destroy the Regent by the end of Round 6."
    );
    assert_eq!((seven.base_reward, seven.optional_reward), (1000, 300));

    // Base credits before Seven: 300 + 400 + 500 + 600 + 700 + 800 = 3300.
    let base_before_seven: u32 = [
        MissionId::One,
        MissionId::Two,
        MissionId::Three,
        MissionId::Four,
        MissionId::Five,
        MissionId::Six,
    ]
    .into_iter()
    .map(|id| mission_definition(id).unwrap().base_reward)
    .sum();
    assert_eq!(base_before_seven, 3300);
    // Max optional through Six: 100 + 100 + 150 + 150 + 200 + 250 = 950.
    let optional_through_six: u32 = [
        MissionId::One,
        MissionId::Two,
        MissionId::Three,
        MissionId::Four,
        MissionId::Five,
        MissionId::Six,
    ]
    .into_iter()
    .map(|id| mission_definition(id).unwrap().optional_reward)
    .sum();
    assert_eq!(optional_through_six, 950);
    assert_eq!(base_before_seven + optional_through_six, 4250);
}
