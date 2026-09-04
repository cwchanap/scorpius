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

    pub const RIFLEMAN: UnitId = UnitId(21);
    pub const STRIKER: UnitId = UnitId(22);
    pub const ARTILLERY: UnitId = UnitId(23);
    pub const FLANKER: UnitId = UnitId(24);
}

const MISSION_TWO_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(3, 7),
    gunner: GridPos::new(4, 6),
    interceptor: GridPos::new(5, 7),
};

/// Authored opening: each attacker locks its destination and intended victim
/// before the player phase, exactly as the spec pins them.
static MISSION_TWO_OPENING: [EnemyOpening; 4] = [
    EnemyOpening {
        unit: ids::RIFLEMAN,
        destination: GridPos::new(2, 4),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::STRIKER,
        destination: GridPos::new(4, 5),
        target: Some(ids::GUNNER),
    },
    EnemyOpening {
        unit: ids::ARTILLERY,
        destination: GridPos::new(4, 0),
        target: Some(ids::GUNNER),
    },
    EnemyOpening {
        unit: ids::FLANKER,
        destination: GridPos::new(5, 5),
        target: Some(ids::INTERCEPTOR),
    },
];

const MISSION_TWO_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::ProtectThroughRound {
        target: ids::GUNNER,
        round: 3,
    },
    optional: OptionalObjective::ProtectTargetAtHalfHp {
        target: ids::GUNNER,
    },
    opening_plan: &MISSION_TWO_OPENING,
};

/// Mission 2's Gunner is the protect target: a different authored unit from
/// Mission 1's, with 15 max HP where the shared squad base is 12. Campaign HP
/// upgrades still add +3 per level on top of the authored value.
const GUNNER_PROTECT_BONUS_HP: i16 = 3;

pub fn mission_two(seed: u64) -> BattleState {
    mission_two_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_two_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_TWO_DEPLOYMENT);
    for unit in &mut units {
        if unit.id == ids::GUNNER {
            unit.stats.max_hp += GUNNER_PROTECT_BONUS_HP;
            unit.hp = unit.stats.max_hp;
        }
    }
    units.extend(mission_two_enemy_units());
    weapons.extend(mission_two_enemy_weapons());
    BattleState::new(mission_two_board(), units, weapons, MISSION_TWO_RULES, seed)
}

fn mission_two_board() -> BoardState {
    BoardState::new(
        9,
        9,
        [
            GridPos::new(3, 3),
            GridPos::new(5, 3),
            GridPos::new(2, 6),
            GridPos::new(6, 6),
        ],
        [GridPos::new(1, 5), GridPos::new(7, 5)],
        [ExplosiveState {
            position: GridPos::new(6, 4),
            hp: 4,
            exploded: false,
        }],
    )
}

fn mission_two_enemy_units() -> Vec<UnitState> {
    vec![
        enemies::rifleman(ids::RIFLEMAN, "Rifleman", GridPos::new(2, 2)),
        enemies::striker(ids::STRIKER, "Striker", GridPos::new(4, 3)),
        enemies::artillery(ids::ARTILLERY, "Artillery", GridPos::new(4, 0)),
        enemies::flanker(ids::FLANKER, "Flanker", GridPos::new(8, 4)),
    ]
}

fn mission_two_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
        enemies::service_rifle(),
        enemies::shock_claw(),
        enemies::siege_mortar(),
        enemies::skirmish_carbine(),
    ]
}

static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "Their counterattack has arrived. Enemy forces are pushing on Relay Nine — Gunner's fire support must survive the assault.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Then we hold the line. Nothing gets through to Gunner.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Their striker and artillery are already zeroing in on Gunner. Hold for three rounds — or wipe them out.",
        portrait: "vn/control_alert.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 2] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "Relay Nine still stands. Their assault broke against the line and Gunner is intact.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Confirmed — hold maintained. Salvage recovered; spend it before the next drop.",
        portrait: "vn/control_neutral.png",
    },
];

pub const MISSION_TWO_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Two,
    unlocks: Some(MissionId::Three),
    build: mission_two_for_campaign,
    title: "Mission 2 — Hold Relay Nine",
    primary_objective: "Protect Gunner through the end of Round 3, or eliminate all attackers.",
    optional_objective: "Hold Fast: finish with Gunner at or above 50% HP.",
    base_reward: 400,
    optional_reward: 100,
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
    use crate::domain::model::{BattleEvent, BattlePhase, Faction, Reaction};
    use crate::mission::assert_opening_plan_is_legal;
    use crate::mission::mission_definition;
    use crate::mission::squad::ids::RAIL_RIFLE;

    #[test]
    fn mission_two_authors_the_spec_board_and_roster() {
        let battle = mission_two(7);
        assert_eq!(battle.board().width(), 9);
        assert_eq!(battle.board().height(), 9);
        assert_eq!(
            battle.board().blocking_cells().collect::<Vec<_>>(),
            vec![
                GridPos::new(3, 3),
                GridPos::new(5, 3),
                GridPos::new(2, 6),
                GridPos::new(6, 6),
            ]
        );
        assert_eq!(
            battle.board().hazard_cells().collect::<Vec<_>>(),
            vec![GridPos::new(1, 5), GridPos::new(7, 5)]
        );
        assert_eq!(
            battle.board().explosive_at(GridPos::new(6, 4)).unwrap().hp,
            4
        );

        assert_eq!(
            battle.unit(ids::VANGUARD).unwrap().position,
            GridPos::new(3, 7)
        );
        assert_eq!(
            battle.unit(ids::INTERCEPTOR).unwrap().position,
            GridPos::new(5, 7)
        );
        assert_eq!(
            battle.unit(ids::RIFLEMAN).unwrap().position,
            GridPos::new(2, 2)
        );
        assert_eq!(
            battle.unit(ids::STRIKER).unwrap().position,
            GridPos::new(4, 3)
        );
        assert_eq!(
            battle.unit(ids::ARTILLERY).unwrap().position,
            GridPos::new(4, 0)
        );
        assert_eq!(
            battle.unit(ids::FLANKER).unwrap().position,
            GridPos::new(8, 4)
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
            4
        );
    }

    #[test]
    fn mission_two_gunner_is_the_authored_protect_target_with_fifteen_hp() {
        let battle = mission_two(7);
        let gunner = battle.unit(ids::GUNNER).unwrap();
        assert_eq!(gunner.position, GridPos::new(4, 6));
        assert_eq!(gunner.stats.max_hp, 15);
        assert_eq!(gunner.hp, 15);
    }

    #[test]
    fn mission_two_gunner_still_projects_campaign_hp_upgrades() {
        let upgrades = SquadUpgrades {
            gunner: UpgradeLevels {
                hp: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let battle = mission_two_for_campaign(7, &upgrades);
        assert_eq!(battle.unit(ids::GUNNER).unwrap().stats.max_hp, 18);
    }

    #[test]
    fn mission_two_rules_and_opening_match_the_spec() {
        let battle = mission_two(7);
        let rules = battle.rules();
        assert_eq!(
            rules.primary,
            PrimaryObjective::ProtectThroughRound {
                target: ids::GUNNER,
                round: 3,
            }
        );
        assert_eq!(
            rules.optional,
            OptionalObjective::ProtectTargetAtHalfHp {
                target: ids::GUNNER,
            }
        );

        let expected = [
            (ids::RIFLEMAN, GridPos::new(2, 4), ids::VANGUARD),
            (ids::STRIKER, GridPos::new(4, 5), ids::GUNNER),
            (ids::ARTILLERY, GridPos::new(4, 0), ids::GUNNER),
            (ids::FLANKER, GridPos::new(5, 5), ids::INTERCEPTOR),
        ];
        assert_eq!(rules.opening_plan.len(), expected.len());
        for (opening, (unit, destination, target)) in rules.opening_plan.iter().zip(expected) {
            assert_eq!(opening.unit, unit);
            assert_eq!(opening.destination, destination);
            assert_eq!(opening.target, Some(target));
        }
    }

    #[test]
    fn mission_two_opening_rows_reference_legal_units_and_destinations() {
        let battle = mission_two(7);
        assert_opening_plan_is_legal(&battle);

        // Mission-specific: every Mission 2 opening row locks a target.
        for opening in battle.rules().opening_plan {
            assert!(opening.target.is_some(), "M2 opening rows lock a target");
        }
    }

    #[test]
    fn clearing_every_attacker_wins_immediately_before_round_three() {
        let mut battle = mission_two(7);
        battle.begin_round().unwrap();
        assert_eq!(battle.round(), 1);

        for attacker in [ids::RIFLEMAN, ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
            battle.apply_direct_damage(attacker, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        }

        assert_eq!(battle.phase(), BattlePhase::Victory);
        let result = battle.result().unwrap();
        assert!(result.victory);
        assert_eq!(result.rounds, 1);
        assert!(!battle.unit(ids::GUNNER).unwrap().is_knocked_out());
    }

    /// Resolve real full rounds until the round-3 planning boundary fires.
    fn finish_all_player_activations(battle: &mut BattleState) {
        for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(player).unwrap();
            battle.choose_reaction(player, Reaction::Guard).unwrap();
            battle.finish_activation(player).unwrap();
        }
    }

    #[test]
    fn surviving_to_round_three_wins_with_an_attacker_still_alive() {
        let mut battle = mission_two(7);
        battle.begin_round().unwrap();

        // Leave the Rifleman as the only attacker: it keeps aiming at the
        // higher-priority Vanguard from outside Vanguard's counter ranges, so
        // Gunner survives the resolved rounds and the attacker is still alive
        // at the boundary.
        for attacker in [ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
            battle.apply_direct_damage(attacker, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        }
        assert!(battle.result().is_none(), "three of four must not win yet");

        for round in 1..=3 {
            finish_all_player_activations(&mut battle);
            battle.resolve_enemy_phase().unwrap();
            if round < 3 {
                assert!(battle.result().is_none(), "round {round} must not end it");
            }
        }

        assert_eq!(battle.phase(), BattlePhase::Victory);
        let result = battle.result().unwrap();
        assert!(result.victory);
        assert_eq!(result.rounds, 3);
        assert!(!battle.unit(ids::RIFLEMAN).unwrap().is_knocked_out());
        assert!(!battle.unit(ids::GUNNER).unwrap().is_knocked_out());
    }

    #[test]
    fn gunner_ko_fails_the_mission() {
        let mut battle = mission_two(7);
        battle.begin_round().unwrap();

        let events = battle.apply_direct_damage(ids::GUNNER, 99, DamageSource::Hazard);

        assert_eq!(battle.phase(), BattlePhase::Defeat);
        let result = battle.result().unwrap();
        assert!(!result.victory);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BattleEvent::MissionFailed { .. }))
        );
    }

    #[test]
    fn hold_fast_bonus_tracks_the_fifteen_hp_half_boundary() {
        // 8/15 is above half (8 * 2 = 16 >= 15): bonus granted at victory.
        let mut above_half = mission_two(7);
        above_half.begin_round().unwrap();
        above_half
            .unit_mut_for_test(ids::GUNNER)
            .expect("gunner must exist")
            .hp = 8;
        for attacker in [ids::RIFLEMAN, ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
            above_half.apply_direct_damage(attacker, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        }
        assert!(
            above_half
                .result()
                .is_some_and(|result| result.victory && result.optional_complete)
        );

        // 7/15 is below half (7 * 2 = 14 < 15): no bonus.
        let mut below_half = mission_two(7);
        below_half.begin_round().unwrap();
        below_half
            .unit_mut_for_test(ids::GUNNER)
            .expect("gunner must exist")
            .hp = 7;
        for attacker in [ids::RIFLEMAN, ids::STRIKER, ids::ARTILLERY, ids::FLANKER] {
            below_half.apply_direct_damage(attacker, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        }
        assert!(
            below_half
                .result()
                .is_some_and(|result| result.victory && !result.optional_complete)
        );
    }

    #[test]
    fn mission_ids_resolve_only_for_authored_missions() {
        assert_eq!(
            mission_definition(MissionId::One).unwrap().title,
            "Mission 1 — Turnabout at Relay Nine"
        );
        let two = mission_definition(MissionId::Two).unwrap();
        assert_eq!(two.id, MissionId::Two);
        assert_eq!(two.unlocks, Some(MissionId::Three));
        let three = mission_definition(MissionId::Three).unwrap();
        assert_eq!(three.id, MissionId::Three);
        assert_eq!(three.unlocks, Some(MissionId::Four));
        assert!(mission_definition(MissionId::Seven).is_some());
    }

    #[test]
    fn mission_two_definition_carries_the_spec_copy_and_rewards() {
        let definition = mission_definition(MissionId::Two).unwrap();
        assert_eq!(definition.title, "Mission 2 — Hold Relay Nine");
        assert_eq!(
            definition.primary_objective,
            "Protect Gunner through the end of Round 3, or eliminate all attackers."
        );
        assert_eq!(
            definition.optional_objective,
            "Hold Fast: finish with Gunner at or above 50% HP."
        );
        assert_eq!(
            (definition.base_reward, definition.optional_reward),
            (400, 100)
        );
        assert_eq!(definition.unlocks, Some(MissionId::Three));
    }
}
