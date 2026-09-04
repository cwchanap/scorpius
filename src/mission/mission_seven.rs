use crate::campaign::model::SquadUpgrades;
use crate::domain::{
    battle::BattleState,
    board::{BoardState, ExplosiveState, GridPos},
    model::{
        EnemyOpening, Faction, MissionRules, OptionalObjective, PrimaryObjective, UnitArchetype,
        UnitState, WeaponShape, WeaponSpec,
    },
};
use crate::mission::enemies;
use crate::mission::squad::{SquadDeployment, build_player_squad, stats, unit, weapon};
use crate::mission::{DialogueLine, DialogueScene, MissionDefinition, MissionId};

pub mod ids {
    pub use crate::mission::squad::ids::{GUNNER, INTERCEPTOR, VANGUARD};

    use crate::domain::model::{UnitId, WeaponId};

    pub const REGENT: UnitId = UnitId(71);
    pub const ARTILLERY: UnitId = UnitId(72);
    pub const CONTROLLER: UnitId = UnitId(73);
    pub const BULWARK: UnitId = UnitId(74);
    pub const FLANKER: UnitId = UnitId(75);
    pub const COMMAND_BARRAGE: WeaponId = WeaponId(209);
    pub const RUPTURE_BEAM: WeaponId = WeaponId(210);
}

const MISSION_SEVEN_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

/// Authored opening: the Regent steps up and locks its Command Barrage over
/// the Vanguard's deployment column — centred so the footprint covers both
/// the live explosive at `(3,7)` and the push lane at `(5,7)` — while the
/// escorts close into their spec destinations.
static MISSION_SEVEN_OPENING: [EnemyOpening; 5] = [
    EnemyOpening {
        unit: ids::REGENT,
        destination: GridPos::new(4, 2),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::ARTILLERY,
        destination: GridPos::new(2, 2),
        target: Some(ids::GUNNER),
    },
    EnemyOpening {
        unit: ids::CONTROLLER,
        destination: GridPos::new(6, 7),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::BULWARK,
        destination: GridPos::new(2, 6),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::FLANKER,
        destination: GridPos::new(1, 8),
        target: Some(ids::GUNNER),
    },
];

const MISSION_SEVEN_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateTarget {
        target: ids::REGENT,
    },
    optional: OptionalObjective::VictoryByRound { round: 6 },
    opening_plan: &MISSION_SEVEN_OPENING,
};

pub fn mission_seven(seed: u64) -> BattleState {
    mission_seven_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_seven_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_SEVEN_DEPLOYMENT);
    units.extend(mission_seven_enemy_units());
    weapons.extend(mission_seven_enemy_weapons());
    BattleState::new(
        mission_seven_board(),
        units,
        weapons,
        MISSION_SEVEN_RULES,
        seed,
    )
}

fn mission_seven_board() -> BoardState {
    BoardState::new(
        9,
        9,
        [
            GridPos::new(2, 4),
            GridPos::new(6, 4),
            GridPos::new(2, 5),
            GridPos::new(6, 5),
        ],
        [GridPos::new(3, 5), GridPos::new(5, 5)],
        [ExplosiveState {
            position: GridPos::new(3, 7),
            hp: 4,
            exploded: false,
        }],
    )
}

fn mission_seven_enemy_units() -> Vec<UnitState> {
    vec![
        unit(
            ids::REGENT,
            "Regent",
            UnitArchetype::Regent,
            Faction::Enemy,
            stats(52, 4, 2, 92, 8, 0),
            GridPos::new(4, 1),
            vec![ids::COMMAND_BARRAGE, ids::RUPTURE_BEAM],
        ),
        enemies::artillery(ids::ARTILLERY, "Artillery", GridPos::new(2, 1)),
        enemies::controller(ids::CONTROLLER, "Controller", GridPos::new(8, 7)),
        enemies::bulwark(ids::BULWARK, "Bulwark", GridPos::new(2, 7)),
        enemies::flanker(ids::FLANKER, "Flanker", GridPos::new(0, 7)),
    ]
}

fn mission_seven_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
        command_barrage(),
        rupture_beam(),
        enemies::siege_mortar(),
        enemies::bastion_cannon(),
        enemies::impulse_projector(),
        enemies::skirmish_carbine(),
    ]
}

const fn command_barrage() -> WeaponSpec {
    weapon(
        ids::COMMAND_BARRAGE,
        "Command Barrage",
        3,
        6,
        WeaponShape::Cross1,
        9,
        10,
        5,
        0,
        false,
        false,
    )
}

const fn rupture_beam() -> WeaponSpec {
    weapon(
        ids::RUPTURE_BEAM,
        "Rupture Beam",
        2,
        4,
        WeaponShape::Single,
        12,
        15,
        10,
        0,
        false,
        false,
    )
}

static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "The last command node is ahead. The Regent is broadcasting firing solutions to everything still standing.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Then we make its final order point the wrong way.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Break the Regent. Once the command net drops, Relay Nine is ours.",
        portrait: "vn/control_alert.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "Regent down. The remaining signatures are scattering.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Relay Nine is secure. Bring everyone home.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Copy. Mission complete.",
        portrait: "vn/vanguard_neutral.png",
    },
];

pub const MISSION_SEVEN_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Seven,
    unlocks: None,
    build: mission_seven_for_campaign,
    title: "Mission 7 - Last Command",
    primary_objective: "Destroy the Regent and break the command net.",
    optional_objective: "Final Push: destroy the Regent by the end of Round 6.",
    base_reward: 1000,
    optional_reward: 300,
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
    use crate::domain::model::{BattleEvent, Reaction};
    use crate::mission::assert_opening_plan_is_legal;
    use crate::mission::mission_definition;
    use crate::mission::squad;

    #[test]
    fn mission_seven_authors_the_spec_board_boss_and_rules() {
        let battle = mission_seven(7);
        assert_eq!((battle.board().width(), battle.board().height()), (9, 9));
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
            vec![GridPos::new(3, 5), GridPos::new(5, 5)]
        );
        assert_eq!(
            battle.board().explosive_at(GridPos::new(3, 7)).unwrap().hp,
            4
        );
        assert_eq!(battle.unit(ids::REGENT).unwrap().stats.max_hp, 52);
        assert_eq!(battle.unit(ids::REGENT).unwrap().stats.armor, 4);
        assert_eq!(battle.unit(ids::REGENT).unwrap().stats.movement, 2);
        assert_eq!(
            battle.unit(ids::REGENT).unwrap().weapons,
            vec![ids::COMMAND_BARRAGE, ids::RUPTURE_BEAM]
        );
        assert_eq!(
            battle.rules().primary,
            PrimaryObjective::EliminateTarget {
                target: ids::REGENT
            }
        );
        assert_eq!(
            battle.rules().optional,
            OptionalObjective::VictoryByRound { round: 6 }
        );
    }

    #[test]
    fn mission_seven_pins_the_enemy_roster_and_regent_weapon_profiles() {
        let battle = mission_seven(1);

        let roster = [
            (ids::REGENT, GridPos::new(4, 1), stats(52, 4, 2, 92, 8, 0)),
            (
                ids::ARTILLERY,
                GridPos::new(2, 1),
                stats(10, 1, 1, 90, 0, 0),
            ),
            (
                ids::CONTROLLER,
                GridPos::new(8, 7),
                stats(9, 1, 2, 82, 15, 0),
            ),
            (ids::BULWARK, GridPos::new(2, 7), stats(16, 4, 1, 76, 0, 0)),
            (ids::FLANKER, GridPos::new(0, 7), stats(8, 0, 4, 82, 30, 0)),
        ];
        for (id, position, unit_stats) in roster {
            let enemy = battle.unit(id).unwrap();
            assert_eq!(enemy.position, position);
            assert_eq!(enemy.stats, unit_stats);
        }

        assert_eq!(
            battle.weapon(ids::COMMAND_BARRAGE),
            Some(&command_barrage())
        );
        assert_eq!(battle.weapon(ids::RUPTURE_BEAM), Some(&rupture_beam()));
    }

    #[test]
    fn mission_seven_opening_rows_match_the_spec() {
        let battle = mission_seven(7);
        let expected = [
            (ids::REGENT, GridPos::new(4, 2), Some(ids::VANGUARD)),
            (ids::ARTILLERY, GridPos::new(2, 2), Some(ids::GUNNER)),
            (ids::CONTROLLER, GridPos::new(6, 7), Some(ids::VANGUARD)),
            (ids::BULWARK, GridPos::new(2, 6), Some(ids::VANGUARD)),
            (ids::FLANKER, GridPos::new(1, 8), Some(ids::GUNNER)),
        ];
        assert_eq!(battle.rules().opening_plan.len(), expected.len());
        for (opening, (unit, destination, target)) in
            battle.rules().opening_plan.iter().zip(expected)
        {
            assert_eq!(opening.unit, unit);
            assert_eq!(opening.destination, destination);
            assert_eq!(opening.target, target);
        }
    }

    #[test]
    fn mission_seven_opening_rows_are_legal() {
        assert_opening_plan_is_legal(&mission_seven(1));
    }

    /// Seed 2, all three player activations spent pulling the Controller off
    /// its committed push lane and onto the Regent's committed Command Barrage
    /// footprint cell `(5,7)` while the Vanguard vacates the footprint center
    /// `(4,7)`. The Interceptor fires the real Vector Pulse action through
    /// `BattleState::attack` — applying its damage and push and consuming the
    /// two RNG rolls (hit, crit) the player action actually spends — so the
    /// RNG call order matches a state the player can create. The Gunner steps
    /// aside between the Vanguard and Interceptor, the one public deviation
    /// from Mission 6's redirect.
    fn redirected_opening_ready_to_resolve() -> BattleState {
        let mut battle = mission_seven(2);
        battle.begin_round().unwrap();

        let regent_intent = battle.intent_for(ids::REGENT).unwrap();
        assert_eq!(regent_intent.profile.weapon, ids::COMMAND_BARRAGE);
        assert!(regent_intent.footprint.contains(&GridPos::new(3, 7)));
        assert!(regent_intent.footprint.contains(&GridPos::new(5, 7)));
        let explosive = battle.board().explosive_at(GridPos::new(3, 7)).unwrap();
        assert!(!explosive.exploded);
        assert!(battle.board().explosive_at(GridPos::new(5, 7)).is_none());

        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 5)).unwrap();
        battle
            .choose_reaction(ids::VANGUARD, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::VANGUARD).unwrap();

        battle.begin_activation(ids::GUNNER).unwrap();
        battle.move_unit(ids::GUNNER, GridPos::new(2, 8)).unwrap();
        battle
            .choose_reaction(ids::GUNNER, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::GUNNER).unwrap();

        battle.begin_activation(ids::INTERCEPTOR).unwrap();
        battle
            .move_unit(ids::INTERCEPTOR, GridPos::new(7, 7))
            .unwrap();
        // Real Vector Pulse: damage then push through `attack`, spending the
        // hit and crit rolls. Seed 2 pins a normal hit (roll 11, crit roll 27)
        // that deals 3 damage (9 -> 6) and pushes the Controller onto the
        // Regent's barrage footprint.
        let vp_events = battle
            .attack(
                ids::INTERCEPTOR,
                squad::ids::VECTOR_PULSE,
                GridPos::new(6, 7),
            )
            .unwrap();
        assert!(vp_events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackRolled {
                attacker,
                target,
                roll: 11,
                hit: true,
                critical_roll: Some(27),
                critical: false,
                ..
            } if *attacker == ids::INTERCEPTOR && *target == ids::CONTROLLER
        )));
        assert!(vp_events.iter().any(|event| matches!(
            event,
            BattleEvent::UnitPushed { unit, to, .. }
                if *unit == ids::CONTROLLER && *to == GridPos::new(5, 7)
        )));
        battle
            .choose_reaction(ids::INTERCEPTOR, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::INTERCEPTOR).unwrap();

        assert_eq!(
            battle.unit(ids::CONTROLLER).unwrap().position,
            GridPos::new(5, 7)
        );
        // Vector Pulse damage applied: Controller is at 6 HP, not its start 9.
        assert_eq!(battle.unit(ids::CONTROLLER).unwrap().hp, 6);
        battle
    }

    #[test]
    fn redirected_command_barrage_detonates_the_explosive_and_cancels_the_pushed_controller() {
        let mut battle = redirected_opening_ready_to_resolve();
        let events = battle.resolve_enemy_phase().unwrap();

        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackRolled {
                attacker,
                weapon,
                target,
                roll: 52,
                hit: true,
                critical_roll: Some(37),
                critical: false,
                ..
            } if *attacker == ids::REGENT
                && *weapon == ids::COMMAND_BARRAGE
                && *target == ids::CONTROLLER
        )));

        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::ExplosiveDamaged { position, .. }
                if *position == GridPos::new(3, 7)
        )));

        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::ExplosionTriggered { position, .. }
                if *position == GridPos::new(3, 7)
        )));

        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::IntentCanceled { attacker }
                if *attacker == ids::CONTROLLER
        )));
    }

    #[test]
    fn mission_seven_definition_is_the_authored_terminal_mission() {
        let definition = mission_definition(MissionId::Seven).unwrap();
        assert_eq!(definition.id, MissionId::Seven);
        assert_eq!(definition.unlocks, None);
        assert_eq!(definition.title, "Mission 7 - Last Command");
        assert_eq!(
            definition.primary_objective,
            "Destroy the Regent and break the command net."
        );
        assert_eq!(
            definition.optional_objective,
            "Final Push: destroy the Regent by the end of Round 6."
        );
        assert_eq!(
            (definition.base_reward, definition.optional_reward),
            (1000, 300)
        );

        // Dialogue uses only existing VN assets with the spec's exact lines.
        assert_eq!(definition.pre_mission.background, "vn/relay_nine_bg.png");
        assert_eq!(definition.pre_mission.lines.len(), 3);
        assert_eq!(
            definition.pre_mission.lines[0],
            DialogueLine {
                speaker: "Control",
                text: "The last command node is ahead. The Regent is broadcasting firing solutions to everything still standing.",
                portrait: "vn/control_neutral.png",
            }
        );
        assert_eq!(
            definition.pre_mission.lines[1],
            DialogueLine {
                speaker: "Vanguard",
                text: "Then we make its final order point the wrong way.",
                portrait: "vn/vanguard_neutral.png",
            }
        );
        assert_eq!(
            definition.pre_mission.lines[2],
            DialogueLine {
                speaker: "Control",
                text: "Break the Regent. Once the command net drops, Relay Nine is ours.",
                portrait: "vn/control_alert.png",
            }
        );
        assert_eq!(definition.aftermath.background, "vn/relay_nine_bg.png");
        assert_eq!(definition.aftermath.lines.len(), 3);
        assert_eq!(
            definition.aftermath.lines[0],
            DialogueLine {
                speaker: "Vanguard",
                text: "Regent down. The remaining signatures are scattering.",
                portrait: "vn/vanguard_neutral.png",
            }
        );
        assert_eq!(
            definition.aftermath.lines[1],
            DialogueLine {
                speaker: "Control",
                text: "Relay Nine is secure. Bring everyone home.",
                portrait: "vn/control_neutral.png",
            }
        );
        assert_eq!(
            definition.aftermath.lines[2],
            DialogueLine {
                speaker: "Vanguard",
                text: "Copy. Mission complete.",
                portrait: "vn/vanguard_neutral.png",
            }
        );

        // The definition builds the same battle as the direct constructor.
        let battle = (definition.build)(7, &SquadUpgrades::default());
        assert_eq!(battle.unit(ids::REGENT).unwrap().stats.max_hp, 52);
    }
}
