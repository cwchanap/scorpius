use crate::campaign::model::SquadUpgrades;
use crate::domain::{
    battle::BattleState,
    board::{BoardState, GridPos},
    model::{
        EnemyOpening, MissionRules, OptionalObjective, PrimaryObjective, UnitArchetype, UnitState,
        WeaponSpec,
    },
};
use crate::mission::enemies;
use crate::mission::squad::{SquadDeployment, build_player_squad, stats, unit, weapon};
use crate::mission::{DialogueLine, DialogueScene, MissionDefinition, MissionId};

pub mod ids {
    pub use crate::mission::squad::ids::{GUNNER, INTERCEPTOR, VANGUARD};

    use crate::domain::model::{UnitId, WeaponId};

    pub const DREADNOUGHT: UnitId = UnitId(61);
    pub const BULWARK: UnitId = UnitId(62);
    pub const CONTROLLER: UnitId = UnitId(63);
    pub const RIFLEMAN: UnitId = UnitId(64);
    pub const GRAVITON_SALVO: WeaponId = WeaponId(207);
    pub const OVERLOAD_SALVO: WeaponId = WeaponId(208);
}

const MISSION_SIX_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

/// Authored opening: the Dreadnought steps up and locks its Graviton salvo on
/// the Vanguard while the escorts close into their spec destinations.
static MISSION_SIX_OPENING: [EnemyOpening; 4] = [
    EnemyOpening {
        unit: ids::DREADNOUGHT,
        destination: GridPos::new(4, 2),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::BULWARK,
        destination: GridPos::new(1, 7),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::CONTROLLER,
        destination: GridPos::new(6, 7),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::RIFLEMAN,
        destination: GridPos::new(6, 6),
        target: Some(ids::INTERCEPTOR),
    },
];

const MISSION_SIX_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateTarget {
        target: ids::DREADNOUGHT,
    },
    optional: OptionalObjective::Turnabout,
    opening_plan: &MISSION_SIX_OPENING,
};

pub fn mission_six(seed: u64) -> BattleState {
    mission_six_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_six_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_SIX_DEPLOYMENT);
    units.extend(mission_six_enemy_units());
    weapons.extend(mission_six_enemy_weapons());
    BattleState::new(mission_six_board(), units, weapons, MISSION_SIX_RULES, seed)
}

fn mission_six_board() -> BoardState {
    BoardState::new(
        9,
        9,
        [
            GridPos::new(2, 4),
            GridPos::new(6, 4),
            GridPos::new(2, 5),
            GridPos::new(6, 5),
        ],
        [],
        [],
    )
}

fn mission_six_enemy_units() -> Vec<UnitState> {
    vec![
        unit(
            ids::DREADNOUGHT,
            "Dreadnought",
            UnitArchetype::Dreadnought,
            crate::domain::model::Faction::Enemy,
            stats(40, 3, 1, 90, 5, 0),
            GridPos::new(4, 1),
            vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO],
        ),
        enemies::bulwark(ids::BULWARK, "Bulwark", GridPos::new(0, 7)),
        enemies::controller(ids::CONTROLLER, "Controller", GridPos::new(8, 7)),
        enemies::rifleman(ids::RIFLEMAN, "Rifleman", GridPos::new(8, 6)),
    ]
}

fn mission_six_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
        weapon(
            ids::GRAVITON_SALVO,
            "Graviton Salvo",
            3,
            6,
            crate::domain::model::WeaponShape::Cross1,
            8,
            10,
            5,
            0,
            false,
            false,
        ),
        weapon(
            ids::OVERLOAD_SALVO,
            "Overload Salvo",
            2,
            4,
            crate::domain::model::WeaponShape::Cross1,
            10,
            10,
            10,
            0,
            false,
            false,
        ),
        enemies::bastion_cannon(),
        enemies::impulse_projector(),
        enemies::service_rifle(),
    ]
}

static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "A Dreadnought is anchoring the line. Its main battery commits before we move.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Then the escorts are ammunition.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Exactly. Below half integrity the battery overloads and the Dreadnought will close in.",
        portrait: "vn/control_alert.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 2] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "Dreadnought down. Their line is collapsing.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "One command unit remains. Mission 7 is the final push.",
        portrait: "vn/control_neutral.png",
    },
];

pub const MISSION_SIX_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Six,
    unlocks: MissionId::Seven,
    build: mission_six_for_campaign,
    title: "Mission 6 — Break the Dreadnought",
    primary_objective: "Destroy the Dreadnought; escorts may be ignored.",
    optional_objective: "Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion.",
    base_reward: 800,
    optional_reward: 250,
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
    use crate::domain::combat::DamageSource;
    use crate::domain::model::{BattleEvent, Reaction, WeaponShape};
    use crate::mission::assert_opening_plan_is_legal;
    use crate::mission::mission_definition;
    use crate::mission::squad;
    use crate::mission::squad::ids::RAIL_RIFLE;

    #[test]
    fn mission_six_authors_the_spec_board_boss_and_rules() {
        let battle = mission_six(7);
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
        assert_eq!(battle.board().hazard_cells().count(), 0);
        assert_eq!(battle.board().explosives().count(), 0);
        assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.max_hp, 40);
        assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.armor, 3);
        assert_eq!(battle.unit(ids::DREADNOUGHT).unwrap().stats.movement, 1);
        assert_eq!(
            battle.unit(ids::DREADNOUGHT).unwrap().weapons,
            vec![ids::GRAVITON_SALVO, ids::OVERLOAD_SALVO]
        );
        assert_eq!(
            battle.rules().primary,
            PrimaryObjective::EliminateTarget {
                target: ids::DREADNOUGHT
            }
        );
        assert_eq!(battle.rules().optional, OptionalObjective::Turnabout);
    }

    #[test]
    fn mission_six_pins_the_enemy_roster_stats_and_salvo_profiles() {
        let battle = mission_six(1);

        let roster = [
            (
                ids::DREADNOUGHT,
                GridPos::new(4, 1),
                stats(40, 3, 1, 90, 5, 0),
            ),
            (ids::BULWARK, GridPos::new(0, 7), stats(16, 4, 1, 76, 0, 0)),
            (
                ids::CONTROLLER,
                GridPos::new(8, 7),
                stats(9, 1, 2, 82, 15, 0),
            ),
            (ids::RIFLEMAN, GridPos::new(8, 6), stats(9, 1, 2, 72, 5, 0)),
        ];
        for (id, position, unit_stats) in roster {
            let enemy = battle.unit(id).unwrap();
            assert_eq!(enemy.position, position);
            assert_eq!(enemy.stats, unit_stats);
        }

        assert_eq!(
            battle.weapon(ids::GRAVITON_SALVO),
            Some(&weapon(
                ids::GRAVITON_SALVO,
                "Graviton Salvo",
                3,
                6,
                WeaponShape::Cross1,
                8,
                10,
                5,
                0,
                false,
                false,
            ))
        );
        assert_eq!(
            battle.weapon(ids::OVERLOAD_SALVO),
            Some(&weapon(
                ids::OVERLOAD_SALVO,
                "Overload Salvo",
                2,
                4,
                WeaponShape::Cross1,
                10,
                10,
                10,
                0,
                false,
                false,
            ))
        );
    }

    #[test]
    fn mission_six_opening_rows_match_the_spec() {
        let battle = mission_six(7);
        let expected = [
            (ids::DREADNOUGHT, GridPos::new(4, 2), Some(ids::VANGUARD)),
            (ids::BULWARK, GridPos::new(1, 7), Some(ids::VANGUARD)),
            (ids::CONTROLLER, GridPos::new(6, 7), Some(ids::VANGUARD)),
            (ids::RIFLEMAN, GridPos::new(6, 6), Some(ids::INTERCEPTOR)),
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
    fn mission_six_opening_rows_are_legal() {
        assert_opening_plan_is_legal(&mission_six(1));
    }

    /// Seed 2, all three player activations spent displacing the Controller
    /// onto the boss's committed Graviton footprint cell `(5,7)` while the
    /// Vanguard vacates the footprint center `(4,7)`. The Interceptor fires
    /// the real Vector Pulse action through `BattleState::attack` — applying
    /// its damage and push and consuming the two RNG rolls (hit, crit) the
    /// player action actually spends — so the RNG call order matches a state
    /// the player can create.
    fn redirected_opening_ready_to_resolve() -> BattleState {
        let mut battle = mission_six(2);
        battle.begin_round().unwrap();

        assert_eq!(battle.intents()[0].attacker, ids::DREADNOUGHT);
        assert_eq!(battle.intents()[1].attacker, ids::CONTROLLER);
        let boss_intent = battle.intent_for(ids::DREADNOUGHT).unwrap();
        assert_eq!(boss_intent.profile.weapon, ids::GRAVITON_SALVO);
        assert!(boss_intent.footprint.contains(&GridPos::new(5, 7)));

        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 5)).unwrap();
        battle
            .choose_reaction(ids::VANGUARD, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::VANGUARD).unwrap();

        battle.begin_activation(ids::INTERCEPTOR).unwrap();
        battle
            .move_unit(ids::INTERCEPTOR, GridPos::new(7, 7))
            .unwrap();
        // Real Vector Pulse: damage then push through `attack`, spending the
        // hit and crit rolls. Seed 2 pins a normal hit (roll 11) that deals 3
        // damage (9 -> 6) and pushes the Controller onto the boss footprint.
        let vp_events = battle
            .attack(
                ids::INTERCEPTOR,
                squad::ids::VECTOR_PULSE,
                GridPos::new(6, 7),
            )
            .unwrap();
        assert!(vp_events.iter().any(|event| {
            matches!(
                event,
                BattleEvent::AttackRolled {
                    attacker,
                    target,
                    roll: 11,
                    hit: true,
                    critical: false,
                    ..
                } if *attacker == ids::INTERCEPTOR && *target == ids::CONTROLLER
            )
        }));
        assert!(vp_events.iter().any(|event| {
            matches!(
                event,
                BattleEvent::UnitPushed { unit, to, .. }
                    if *unit == ids::CONTROLLER && *to == GridPos::new(5, 7)
            )
        }));
        battle
            .choose_reaction(ids::INTERCEPTOR, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::INTERCEPTOR).unwrap();

        battle.begin_activation(ids::GUNNER).unwrap();
        battle
            .choose_reaction(ids::GUNNER, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::GUNNER).unwrap();

        assert_eq!(
            battle.unit(ids::CONTROLLER).unwrap().position,
            GridPos::new(5, 7)
        );
        assert_eq!(
            battle.unit(ids::VANGUARD).unwrap().position,
            GridPos::new(4, 5)
        );
        // Vector Pulse damage applied: Controller is at 6 HP, not its start 9.
        assert_eq!(battle.unit(ids::CONTROLLER).unwrap().hp, 6);
        battle
    }

    #[test]
    fn redirected_graviton_completes_turnabout_and_cancels_the_knocked_out_controller() {
        let mut battle = redirected_opening_ready_to_resolve();
        let events = battle.resolve_enemy_phase().unwrap();

        // Seed 2: the redirected boss Graviton hits the Controller (roll 52,
        // no crit). The Controller is at 6 HP from Vector Pulse, so the 7
        // Graviton damage knocks it out.
        let boss_hit = events
            .iter()
            .position(|event| {
                matches!(
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
                    } if *attacker == ids::DREADNOUGHT
                        && *weapon == ids::GRAVITON_SALVO
                        && *target == ids::CONTROLLER
                )
            })
            .expect("seed 2 pins a normal Graviton hit on the redirected Controller");

        let turnabout = events
            .iter()
            .position(|event| matches!(event, BattleEvent::OptionalObjectiveCompleted))
            .expect("redirected enemy fire completes Turnabout");

        let controller_canceled = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    BattleEvent::IntentCanceled { attacker } if *attacker == ids::CONTROLLER
                )
            })
            .expect("knocked-out Controller intent is canceled");

        // The Controller was knocked out by the redirected Graviton, so it
        // never fires into the vacated cell.
        assert!(
            !events.iter().any(|event| {
                matches!(
                    event,
                    BattleEvent::AttackHitEmpty { attacker, .. } if *attacker == ids::CONTROLLER
                )
            }),
            "knocked-out Controller does not fire"
        );

        assert!(boss_hit < turnabout);
        assert!(turnabout < controller_canceled);
        assert!(battle.unit(ids::CONTROLLER).unwrap().is_knocked_out());
        assert_eq!(battle.unit(ids::CONTROLLER).unwrap().hp, 0);
    }

    #[test]
    fn dreadnought_ko_grants_victory_while_escorts_still_stand() {
        let mut battle = mission_six(7);
        battle.begin_round().unwrap();
        battle.apply_direct_damage(ids::DREADNOUGHT, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        let result = battle.result().unwrap();
        assert!(result.victory);
        assert!(
            !result.optional_complete,
            "no Turnabout trigger was tripped"
        );
        assert!(!battle.unit(ids::BULWARK).unwrap().is_knocked_out());
    }

    #[test]
    fn dreadnought_is_pushable_with_the_existing_push_primitive() {
        let mut battle = mission_six(7);
        battle.begin_round().unwrap();
        battle.unit_mut_for_test(ids::VANGUARD).unwrap().position = GridPos::new(3, 3);
        battle.unit_mut_for_test(ids::DREADNOUGHT).unwrap().position = GridPos::new(4, 3);
        let events = battle
            .resolve_push(ids::VANGUARD, ids::DREADNOUGHT)
            .unwrap();
        assert_eq!(
            battle.unit(ids::DREADNOUGHT).unwrap().position,
            GridPos::new(5, 3)
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BattleEvent::UnitPushed { .. }))
        );
    }

    #[test]
    fn mission_ids_resolve_only_for_authored_missions() {
        assert_eq!(
            mission_definition(MissionId::Six)
                .map(|definition| (definition.id, definition.unlocks)),
            Some((MissionId::Six, MissionId::Seven))
        );
        assert!(mission_definition(MissionId::Seven).is_none());
    }
}
