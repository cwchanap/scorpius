use crate::campaign::model::SquadUpgrades;
use crate::domain::{
    battle::BattleState,
    board::{BoardState, ExplosiveState, GridPos},
    model::{
        EnemyOpening, MissionRules, OptionalObjective, PrimaryObjective, UnitState, WeaponSpec,
    },
};
use crate::mission::enemies;
use crate::mission::squad::{SquadDeployment, build_player_squad};
use crate::mission::{DialogueLine, DialogueScene, MissionDefinition, MissionId};

pub mod ids {
    pub use crate::mission::squad::ids::{GUNNER, INTERCEPTOR, VANGUARD};

    use crate::domain::model::UnitId;

    pub const BULWARK: UnitId = UnitId(41);
    pub const CONTROLLER: UnitId = UnitId(42);
    pub const RIFLEMAN: UnitId = UnitId(43);
}

const MISSION_FOUR_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

/// Authored opening: the Gate Bulwark steps into the breach while the
/// Controller and Rifleman lock their destinations and intended victims
/// before the player phase, exactly as the spec pins them.
static MISSION_FOUR_OPENING: [EnemyOpening; 3] = [
    EnemyOpening {
        unit: ids::BULWARK,
        destination: GridPos::new(4, 4),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::CONTROLLER,
        destination: GridPos::new(1, 7),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::RIFLEMAN,
        destination: GridPos::new(6, 6),
        target: Some(ids::INTERCEPTOR),
    },
];

const MISSION_FOUR_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateTarget {
        target: ids::BULWARK,
    },
    optional: OptionalObjective::Turnabout,
    opening_plan: &MISSION_FOUR_OPENING,
};

pub fn mission_four(seed: u64) -> BattleState {
    mission_four_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_four_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_FOUR_DEPLOYMENT);
    units.extend(mission_four_enemy_units());
    weapons.extend(mission_four_enemy_weapons());
    BattleState::new(
        mission_four_board(),
        units,
        weapons,
        MISSION_FOUR_RULES,
        seed,
    )
}

fn mission_four_board() -> BoardState {
    BoardState::new(
        9,
        9,
        [
            GridPos::new(2, 4),
            GridPos::new(6, 4),
            GridPos::new(2, 5),
            GridPos::new(6, 5),
        ],
        [GridPos::new(4, 3)],
        [
            ExplosiveState {
                position: GridPos::new(3, 4),
                hp: 4,
                exploded: false,
            },
            ExplosiveState {
                position: GridPos::new(5, 4),
                hp: 4,
                exploded: false,
            },
        ],
    )
}

fn mission_four_enemy_units() -> Vec<UnitState> {
    vec![
        enemies::bulwark(ids::BULWARK, "Gate Bulwark", GridPos::new(4, 5)),
        enemies::controller(ids::CONTROLLER, "Controller", GridPos::new(0, 7)),
        enemies::rifleman(ids::RIFLEMAN, "Rifleman", GridPos::new(8, 6)),
    ]
}

fn mission_four_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
        enemies::bastion_cannon(),
        enemies::impulse_projector(),
        enemies::service_rifle(),
    ]
}

static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "The ridge gate is sealed by a Bulwark. Its armor is built for direct fire; the fuel cells and hazard trench around it are not.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "So we stop treating the battlefield like scenery.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Breach the Bulwark. Ignore the escorts if you can make the board do the work.",
        portrait: "vn/control_alert.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 2] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "Gate's open. Their own position did more damage than our guns.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Good. Long-range batteries are waiting on the far side.",
        portrait: "vn/control_neutral.png",
    },
];

pub const MISSION_FOUR_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Four,
    unlocks: MissionId::Five,
    build: mission_four_for_campaign,
    title: "Mission 4 — Breach the Gate",
    primary_objective: "Destroy the Gate Bulwark; escorts may be ignored.",
    optional_objective: "Chain Reaction: damage any enemy with enemy fire, collision, hazard, or explosion.",
    base_reward: 600,
    optional_reward: 150,
    pre_mission: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &PRE_MISSION_LINES,
    },
    aftermath: DialogueScene {
        background: "vn/relay_nine_bg.png",
        lines: &AFTERMATH_LINES,
    },
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::model::UpgradeLevels;
    use crate::domain::combat::DamageSource;
    use crate::domain::model::{BattleEvent, BattlePhase, Faction};
    use crate::mission::assert_opening_plan_is_legal;
    use crate::mission::mission_definition;
    use crate::mission::squad::ids::RAIL_RIFLE;

    #[test]
    fn mission_four_authors_the_spec_board_and_roster() {
        let battle = mission_four(7);
        assert_eq!(battle.board().width(), 9);
        assert_eq!(battle.board().height(), 9);
        assert_eq!(
            battle.board().blocking_cells().collect::<Vec<_>>(),
            vec![
                GridPos::new(2, 4),
                GridPos::new(6, 4),
                GridPos::new(2, 5),
                GridPos::new(6, 5),
            ]
        );
        assert_eq!(
            battle.board().hazard_cells().collect::<Vec<_>>(),
            vec![GridPos::new(4, 3)]
        );
        assert_eq!(
            battle.board().explosive_at(GridPos::new(3, 4)).unwrap().hp,
            4
        );
        assert_eq!(
            battle.board().explosive_at(GridPos::new(5, 4)).unwrap().hp,
            4
        );

        assert_eq!(
            battle.unit(ids::VANGUARD).unwrap().position,
            GridPos::new(4, 7)
        );
        assert_eq!(
            battle.unit(ids::GUNNER).unwrap().position,
            GridPos::new(3, 8)
        );
        assert_eq!(
            battle.unit(ids::INTERCEPTOR).unwrap().position,
            GridPos::new(5, 8)
        );
        assert_eq!(
            battle.unit(ids::BULWARK).unwrap().position,
            GridPos::new(4, 5)
        );
        assert_eq!(
            battle.unit(ids::CONTROLLER).unwrap().position,
            GridPos::new(0, 7)
        );
        assert_eq!(
            battle.unit(ids::RIFLEMAN).unwrap().position,
            GridPos::new(8, 6)
        );
        assert_eq!(
            battle
                .units()
                .filter(|unit| unit.faction == Faction::Player)
                .count(),
            3
        );
        assert_eq!(
            battle
                .units()
                .filter(|unit| unit.faction == Faction::Enemy)
                .count(),
            3
        );
    }

    #[test]
    fn mission_four_rules_and_opening_match_the_spec() {
        let battle = mission_four(7);
        let rules = battle.rules();
        assert_eq!(
            rules.primary,
            PrimaryObjective::EliminateTarget {
                target: ids::BULWARK
            }
        );
        assert_eq!(rules.optional, OptionalObjective::Turnabout);

        let expected = [
            (ids::BULWARK, GridPos::new(4, 4), Some(ids::VANGUARD)),
            (ids::CONTROLLER, GridPos::new(1, 7), Some(ids::VANGUARD)),
            (ids::RIFLEMAN, GridPos::new(6, 6), Some(ids::INTERCEPTOR)),
        ];
        assert_eq!(rules.opening_plan.len(), expected.len());
        for (opening, (unit, destination, target)) in rules.opening_plan.iter().zip(expected) {
            assert_eq!(opening.unit, unit);
            assert_eq!(opening.destination, destination);
            assert_eq!(opening.target, target);
        }
    }

    #[test]
    fn mission_four_opening_rows_reference_legal_units_and_destinations() {
        let battle = mission_four(7);
        assert_opening_plan_is_legal(&battle);
    }

    #[test]
    fn mission_four_projects_campaign_upgrades_into_the_squad() {
        let upgrades = SquadUpgrades {
            vanguard: UpgradeLevels {
                hp: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let battle = mission_four_for_campaign(7, &upgrades);
        assert_eq!(battle.unit(ids::VANGUARD).unwrap().stats.max_hp, 23);
    }

    #[test]
    fn gunner_can_detonate_the_west_explosive_and_the_blast_reaches_the_bulwark() {
        let mut battle = mission_four(7);
        battle.begin_round().unwrap();

        battle.begin_activation(ids::GUNNER).unwrap();
        let preview = battle
            .preview_attack(ids::GUNNER, RAIL_RIFLE, GridPos::new(3, 4))
            .unwrap();
        assert_eq!(preview.target, GridPos::new(3, 4));

        // The shot is proven legal; detonate the fuel cell directly to remove
        // RNG from the environment assertion.
        let events = battle
            .damage_explosive(
                GridPos::new(3, 4),
                4,
                DamageSource::PlayerWeapon(RAIL_RIFLE),
            )
            .unwrap();
        let explosion = events
            .iter()
            .find(|event| matches!(event, BattleEvent::ExplosionTriggered { .. }))
            .expect("a spent fuel cell must explode");
        let BattleEvent::ExplosionTriggered { footprint, .. } = explosion else {
            unreachable!("matched above");
        };
        assert!(
            footprint.contains(&GridPos::new(4, 4)),
            "the Bulwark's opening cell must be in the blast: {footprint:?}"
        );
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::DamageApplied {
                target,
                source: DamageSource::Explosion,
                ..
            } if *target == ids::BULWARK
        )));
    }

    #[test]
    fn vanguard_can_ram_the_bulwark_into_the_hazard_trench() {
        let mut battle = mission_four(7);
        battle.begin_round().unwrap();

        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 5)).unwrap();
        let events = battle.resolve_push(ids::VANGUARD, ids::BULWARK).unwrap();
        assert_eq!(
            battle.unit(ids::BULWARK).unwrap().position,
            GridPos::new(4, 3)
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BattleEvent::HazardTriggered { .. }))
        );
    }

    #[test]
    fn destroying_only_the_bulwark_wins_with_escorts_standing() {
        let mut battle = mission_four(7);
        battle.begin_round().unwrap();

        battle.apply_direct_damage(ids::BULWARK, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));

        assert_eq!(battle.phase(), BattlePhase::Victory);
        let result = battle.result().unwrap();
        assert!(result.victory);
        assert_eq!(result.rounds, 1);
        assert!(!battle.unit(ids::CONTROLLER).unwrap().is_knocked_out());
        assert!(!battle.unit(ids::RIFLEMAN).unwrap().is_knocked_out());
    }

    #[test]
    fn turnabout_completes_from_qualifying_environmental_damage() {
        // Explosion damage to an enemy is a qualifying Turnabout trigger.
        let mut battle = mission_four(7);
        battle.begin_round().unwrap();
        let events = battle.apply_direct_damage(ids::RIFLEMAN, 3, DamageSource::Explosion);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BattleEvent::OptionalObjectiveCompleted))
        );
        battle.apply_direct_damage(ids::BULWARK, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        assert!(
            battle
                .result()
                .is_some_and(|result| result.victory && result.optional_complete)
        );

        // Player fire alone never qualifies.
        let mut battle = mission_four(7);
        battle.begin_round().unwrap();
        battle.apply_direct_damage(ids::RIFLEMAN, 3, DamageSource::PlayerWeapon(RAIL_RIFLE));
        battle.apply_direct_damage(ids::BULWARK, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        assert!(
            battle
                .result()
                .is_some_and(|result| result.victory && !result.optional_complete)
        );
    }

    #[test]
    fn mission_ids_resolve_only_for_authored_missions() {
        assert_eq!(
            mission_definition(MissionId::Four)
                .map(|definition| (definition.id, definition.unlocks)),
            Some((MissionId::Four, MissionId::Five))
        );
        assert!(mission_definition(MissionId::Six).is_none());
    }

    #[test]
    fn mission_four_definition_carries_the_spec_copy_and_rewards() {
        let definition = mission_definition(MissionId::Four).unwrap();
        assert_eq!(definition.title, "Mission 4 — Breach the Gate");
        assert_eq!(
            definition.primary_objective,
            "Destroy the Gate Bulwark; escorts may be ignored."
        );
        assert_eq!(
            definition.optional_objective,
            "Chain Reaction: damage any enemy with enemy fire, collision, hazard, or explosion."
        );
        assert_eq!(
            (definition.base_reward, definition.optional_reward),
            (600, 150)
        );
        assert_eq!(definition.unlocks, MissionId::Five);

        // Dialogue reuses only existing VN assets with the spec's exact lines.
        assert_eq!(definition.pre_mission.background, "vn/relay_nine_bg.png");
        assert_eq!(definition.aftermath.background, "vn/relay_nine_bg.png");
        let pre = [
            ("Control", "vn/control_neutral.png"),
            ("Vanguard", "vn/vanguard_neutral.png"),
            ("Control", "vn/control_alert.png"),
        ];
        assert_eq!(definition.pre_mission.lines.len(), pre.len());
        for (line, (speaker, portrait)) in definition.pre_mission.lines.iter().zip(pre) {
            assert_eq!(line.speaker, speaker);
            assert_eq!(line.portrait, portrait);
        }
        assert_eq!(
            definition.pre_mission.lines[0].text,
            "The ridge gate is sealed by a Bulwark. Its armor is built for direct fire; the fuel cells and hazard trench around it are not."
        );
        assert_eq!(
            definition.pre_mission.lines[1].text,
            "So we stop treating the battlefield like scenery."
        );
        assert_eq!(
            definition.pre_mission.lines[2].text,
            "Breach the Bulwark. Ignore the escorts if you can make the board do the work."
        );
        assert_eq!(definition.aftermath.lines.len(), 2);
        assert_eq!(
            definition.aftermath.lines[0],
            DialogueLine {
                speaker: "Vanguard",
                text: "Gate's open. Their own position did more damage than our guns.",
                portrait: "vn/vanguard_neutral.png",
            }
        );
        assert_eq!(
            definition.aftermath.lines[1],
            DialogueLine {
                speaker: "Control",
                text: "Good. Long-range batteries are waiting on the far side.",
                portrait: "vn/control_neutral.png",
            }
        );

        // The definition builds the same battle as the direct constructor.
        let battle = (definition.build)(7, &SquadUpgrades::default());
        assert_eq!(battle.board().width(), 9);
        assert!(battle.unit(ids::BULWARK).is_some());
    }
}
