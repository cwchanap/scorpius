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

    pub const COURIER: UnitId = UnitId(31);
    pub const RIFLEMAN: UnitId = UnitId(32);
    pub const STRIKER: UnitId = UnitId(33);
}

/// The extraction point the Courier races toward; pinned by the spec board.
pub const EXTRACTION: GridPos = GridPos::new(8, 0);

const MISSION_THREE_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

/// Authored opening: the Courier stays put at (0,6) while its escorts lock
/// their destinations and intended victims before the player phase.
static MISSION_THREE_OPENING: [EnemyOpening; 3] = [
    EnemyOpening {
        unit: ids::COURIER,
        destination: GridPos::new(0, 6),
        target: None,
    },
    EnemyOpening {
        unit: ids::RIFLEMAN,
        destination: GridPos::new(3, 4),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::STRIKER,
        destination: GridPos::new(5, 7),
        target: Some(ids::INTERCEPTOR),
    },
];

const MISSION_THREE_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::InterceptBeforeEscape {
        target: ids::COURIER,
        escape: EXTRACTION,
        deadline_round: 5,
    },
    optional: OptionalObjective::VictoryByRound { round: 2 },
    opening_plan: &MISSION_THREE_OPENING,
};

pub fn mission_three(seed: u64) -> BattleState {
    mission_three_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_three_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_THREE_DEPLOYMENT);
    units.extend(mission_three_enemy_units());
    weapons.extend(mission_three_enemy_weapons());
    BattleState::new(
        mission_three_board(),
        units,
        weapons,
        MISSION_THREE_RULES,
        seed,
    )
}

fn mission_three_board() -> BoardState {
    BoardState::new(
        9,
        9,
        [GridPos::new(4, 3), GridPos::new(4, 4), GridPos::new(4, 5)],
        [GridPos::new(2, 5)],
        [ExplosiveState {
            position: GridPos::new(6, 3),
            hp: 4,
            exploded: false,
        }],
    )
}

fn mission_three_enemy_units() -> Vec<UnitState> {
    vec![
        enemies::flanker(ids::COURIER, "Courier", GridPos::new(0, 6)),
        enemies::rifleman(ids::RIFLEMAN, "Rifleman", GridPos::new(3, 2)),
        enemies::striker(ids::STRIKER, "Striker", GridPos::new(6, 6)),
    ]
}

fn mission_three_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
        enemies::skirmish_carbine(),
        enemies::service_rifle(),
        enemies::shock_claw(),
    ]
}

static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "A courier is sprinting for the extraction point on the far ridge. Whatever it carries cannot reach their line — intercept it before the end of Round 5.",
        portrait: "vn/control_alert.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Two escorts and a runner. We cut through them and take the courier down.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Its escorts will buy time at any cost. The faster the courier falls, the better — strike fast.",
        portrait: "vn/control_neutral.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 2] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "The courier is down and its payload is ours. The escorts broke once the runner fell.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Confirmed — nothing reached extraction. Salvage recovered; spend it before the next drop.",
        portrait: "vn/control_neutral.png",
    },
];

pub const MISSION_THREE_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Three,
    unlocks: Some(MissionId::Four),
    build: mission_three_for_campaign,
    title: "Mission 3 — Intercept Courier",
    primary_objective: "Intercept Courier before extraction or the end of Round 5.",
    optional_objective: "Swift Intercept: victory by the end of Round 2.",
    base_reward: 500,
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
    use crate::domain::combat::DamageSource;
    use crate::domain::model::{BattleEvent, BattlePhase, Faction, Reaction};
    use crate::mission::assert_opening_plan_is_legal;
    use crate::mission::mission_definition;
    use crate::mission::squad::ids::RAIL_RIFLE;

    #[test]
    fn mission_three_authors_the_spec_board_and_roster() {
        let battle = mission_three(7);
        assert_eq!(battle.board().width(), 9);
        assert_eq!(battle.board().height(), 9);
        assert_eq!(
            battle.board().blocking_cells().collect::<Vec<_>>(),
            vec![GridPos::new(4, 3), GridPos::new(4, 4), GridPos::new(4, 5),]
        );
        assert_eq!(
            battle.board().hazard_cells().collect::<Vec<_>>(),
            vec![GridPos::new(2, 5)]
        );
        assert_eq!(
            battle.board().explosive_at(GridPos::new(6, 3)).unwrap().hp,
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
            battle.unit(ids::COURIER).unwrap().position,
            GridPos::new(0, 6)
        );
        assert_eq!(
            battle.unit(ids::RIFLEMAN).unwrap().position,
            GridPos::new(3, 2)
        );
        assert_eq!(
            battle.unit(ids::STRIKER).unwrap().position,
            GridPos::new(6, 6)
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
    fn mission_three_rules_and_opening_match_the_spec() {
        let battle = mission_three(7);
        let rules = battle.rules();
        assert_eq!(
            rules.primary,
            PrimaryObjective::InterceptBeforeEscape {
                target: ids::COURIER,
                escape: EXTRACTION,
                deadline_round: 5,
            }
        );
        assert_eq!(
            rules.optional,
            OptionalObjective::VictoryByRound { round: 2 }
        );

        let expected = [
            (ids::COURIER, GridPos::new(0, 6), None),
            (ids::RIFLEMAN, GridPos::new(3, 4), Some(ids::VANGUARD)),
            (ids::STRIKER, GridPos::new(5, 7), Some(ids::INTERCEPTOR)),
        ];
        assert_eq!(rules.opening_plan.len(), expected.len());
        for (opening, (unit, destination, target)) in rules.opening_plan.iter().zip(expected) {
            assert_eq!(opening.unit, unit);
            assert_eq!(opening.destination, destination);
            assert_eq!(opening.target, target);
        }
    }

    #[test]
    fn mission_three_opening_rows_reference_legal_units_and_destinations() {
        let battle = mission_three(7);
        assert_opening_plan_is_legal(&battle);
    }

    #[test]
    fn mission_three_extraction_is_a_legal_open_cell_and_the_race_is_manhattan_fourteen() {
        let battle = mission_three(7);
        assert!(battle.board().contains(EXTRACTION));
        assert!(!battle.board().is_blocking(EXTRACTION));
        assert!(!battle.board().is_hazard(EXTRACTION));
        assert!(!battle.board().has_live_explosive(EXTRACTION));
        assert!(
            battle.units().all(|unit| unit.position != EXTRACTION),
            "extraction must start unoccupied"
        );

        let courier = battle.unit(ids::COURIER).unwrap();
        assert_eq!(courier.stats.movement, 4);
        assert_eq!(courier.position.manhattan(EXTRACTION), 14);
    }

    #[test]
    fn killing_both_escorts_alone_does_not_end_the_battle() {
        let mut battle = mission_three(7);
        battle.begin_round().unwrap();

        for escort in [ids::RIFLEMAN, ids::STRIKER] {
            battle.apply_direct_damage(escort, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        }

        assert!(battle.result().is_none(), "escort clear alone is no result");
        assert_eq!(battle.phase(), BattlePhase::Player);
        assert!(!battle.unit(ids::COURIER).unwrap().is_knocked_out());

        play_one_round(&mut battle);
        assert!(battle.result().is_none(), "the battle continues");
        assert_eq!(battle.phase(), BattlePhase::Player);
    }

    #[test]
    fn courier_ko_wins_even_with_escorts_standing() {
        let mut battle = mission_three(7);
        battle.begin_round().unwrap();

        battle.apply_direct_damage(ids::COURIER, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));

        assert_eq!(battle.phase(), BattlePhase::Victory);
        let result = battle.result().unwrap();
        assert!(result.victory);
        assert_eq!(result.rounds, 1);
        assert!(!battle.unit(ids::RIFLEMAN).unwrap().is_knocked_out());
        assert!(!battle.unit(ids::STRIKER).unwrap().is_knocked_out());
    }

    #[test]
    fn swift_intercept_bonus_tracks_the_round_two_boundary() {
        // Round 1 victory is inside the boundary: bonus granted.
        let mut early = mission_three(7);
        early.begin_round().unwrap();
        early.apply_direct_damage(ids::COURIER, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        assert!(
            early
                .result()
                .is_some_and(|result| result.victory && result.optional_complete)
        );

        // A round-3 victory misses the boundary: no bonus.
        let mut late = mission_three(7);
        late.begin_round().unwrap();
        play_one_round(&mut late);
        play_one_round(&mut late);
        assert_eq!(late.round(), 3);
        late.apply_direct_damage(ids::COURIER, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        assert!(
            late.result()
                .is_some_and(|result| result.victory && !result.optional_complete)
        );
    }

    /// Durable timing step: guards every player activation, then resolves the
    /// enemy phase — intents, the next enemy move, and the following player
    /// phase — so timing tests read as sequence steps.
    fn play_one_round(battle: &mut BattleState) {
        for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(player).unwrap();
            battle.choose_reaction(player, Reaction::Guard).unwrap();
            battle.finish_activation(player).unwrap();
        }
        battle.resolve_enemy_phase().unwrap();
    }

    /// Opening round plus the escorts cleared with player-fire damage, which
    /// never trips an optional trigger; leaves the Courier racing alone.
    fn courier_race_after_opening() -> BattleState {
        let mut battle = mission_three(7);
        battle.begin_round().unwrap();
        for escort in [ids::RIFLEMAN, ids::STRIKER] {
            battle.apply_direct_damage(escort, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        }
        battle
    }

    #[test]
    fn the_couriers_first_three_moves_leave_player_phase_four_without_extraction() {
        let mut battle = courier_race_after_opening();

        for round in 1..=3 {
            play_one_round(&mut battle);
            assert_eq!(battle.round(), round + 1);
            assert_eq!(battle.phase(), BattlePhase::Player);
            assert!(
                battle.result().is_none(),
                "no result through the courier's move {round}"
            );
            assert_ne!(
                battle.unit(ids::COURIER).unwrap().position,
                EXTRACTION,
                "the courier cannot extract on move {round}"
            );
        }
        assert_eq!(battle.round(), 4, "player phase 4 must exist");
    }

    #[test]
    fn the_couriers_fourth_move_reaches_extraction_and_fails_the_mission() {
        let mut battle = courier_race_after_opening();
        for _ in 1..=3 {
            play_one_round(&mut battle);
        }

        play_one_round(&mut battle);

        assert_eq!(battle.phase(), BattlePhase::Defeat);
        let result = battle.result().unwrap();
        assert!(!result.victory, "extraction is a defeat");
        assert_eq!(result.rounds, 4);
        assert_eq!(battle.unit(ids::COURIER).unwrap().position, EXTRACTION);
    }

    #[test]
    fn extraction_ends_the_movement_pass_before_escorts_move() {
        // Regression: the Courier must terminal-check the moment it lands on
        // the extraction point, so the movement pass breaks before the escorts
        // (which iterate after the Courier in unit-id order) can move. The
        // escorts are left alive and parked far from the squad so they would
        // otherwise emit `UnitMoved` events after the Courier extracts.
        let mut battle = mission_three(7);
        battle.begin_round().unwrap();

        // One move (manhattan 4) from extraction.
        battle
            .unit_mut_for_test(ids::COURIER)
            .expect("courier must exist")
            .position = GridPos::new(4, 0);
        // Escorts alive and far from the squad so their planner must move them.
        battle
            .unit_mut_for_test(ids::RIFLEMAN)
            .expect("rifleman must exist")
            .position = GridPos::new(0, 0);
        battle
            .unit_mut_for_test(ids::STRIKER)
            .expect("striker must exist")
            .position = GridPos::new(8, 8);
        battle.set_phase_for_test(BattlePhase::EnemyPlanning);

        let events = battle.begin_round().unwrap();

        assert_eq!(battle.unit(ids::COURIER).unwrap().position, EXTRACTION);
        assert_eq!(battle.phase(), BattlePhase::Defeat);
        assert!(battle.result().is_some_and(|result| !result.victory));

        let courier_move = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    BattleEvent::UnitMoved { unit, to, .. }
                        if *unit == ids::COURIER && *to == EXTRACTION
                )
            })
            .expect("the courier must move onto extraction");
        // No escort may move after the Courier extracts.
        assert!(
            !events[courier_move + 1..]
                .iter()
                .any(|event| matches!(event, BattleEvent::UnitMoved { .. })),
            "no UnitMoved events after extraction: {events:?}"
        );
        assert!(
            events[courier_move + 1..]
                .iter()
                .any(|event| matches!(event, BattleEvent::MissionFailed { .. })),
            "MissionFailed must follow extraction: {events:?}"
        );
        // Sanity: the escorts were alive and would have moved without the fix.
        assert!(!battle.unit(ids::RIFLEMAN).unwrap().is_knocked_out());
        assert!(!battle.unit(ids::STRIKER).unwrap().is_knocked_out());
    }

    #[test]
    fn an_occupied_exit_stalls_to_the_round_five_deadline() {
        let mut battle = courier_race_after_opening();
        for _ in 1..=3 {
            play_one_round(&mut battle);
        }

        // Park the Interceptor on the extraction point: the courier's fourth
        // move cannot land there, so player phase 5 begins with no result.
        battle
            .unit_mut_for_test(ids::INTERCEPTOR)
            .expect("interceptor must exist")
            .position = EXTRACTION;
        play_one_round(&mut battle);

        assert_eq!(battle.round(), 5);
        assert_eq!(battle.phase(), BattlePhase::Player);
        assert!(battle.result().is_none());
        assert_ne!(battle.unit(ids::COURIER).unwrap().position, EXTRACTION);

        // Resolving player phase 5 hits the deadline boundary before any
        // further courier movement.
        play_one_round(&mut battle);

        assert_eq!(battle.phase(), BattlePhase::Defeat);
        let result = battle.result().unwrap();
        assert!(!result.victory);
        assert_eq!(result.rounds, 5);
        assert_ne!(battle.unit(ids::COURIER).unwrap().position, EXTRACTION);
    }

    #[test]
    fn pushing_the_courier_onto_extraction_fails_immediately() {
        let mut battle = mission_three(7);
        battle.begin_round().unwrap();

        // Author the shove geometry: Vanguard at (6,0) displaces the Courier
        // at (7,0) onto the extraction point (8,0).
        battle
            .unit_mut_for_test(ids::VANGUARD)
            .expect("vanguard must exist")
            .position = GridPos::new(6, 0);
        battle
            .unit_mut_for_test(ids::COURIER)
            .expect("courier must exist")
            .position = GridPos::new(7, 0);

        let events = battle.resolve_push(ids::VANGUARD, ids::COURIER).unwrap();

        assert_eq!(battle.unit(ids::COURIER).unwrap().position, EXTRACTION);
        assert!(matches!(
            events.first(),
            Some(BattleEvent::UnitPushed { to, .. }) if *to == EXTRACTION
        ));
        assert_eq!(battle.phase(), BattlePhase::Defeat);
        let result = battle.result().unwrap();
        assert!(!result.victory);
    }

    #[test]
    fn mission_ids_resolve_only_for_authored_missions() {
        assert_eq!(
            mission_definition(MissionId::Three)
                .map(|definition| (definition.id, definition.unlocks)),
            Some((MissionId::Three, Some(MissionId::Four)))
        );
        assert!(mission_definition(MissionId::Seven).is_some());
    }

    #[test]
    fn mission_three_definition_carries_the_spec_copy_and_rewards() {
        let definition = mission_definition(MissionId::Three).unwrap();
        assert_eq!(definition.title, "Mission 3 — Intercept Courier");
        assert_eq!(
            definition.primary_objective,
            "Intercept Courier before extraction or the end of Round 5."
        );
        assert_eq!(
            definition.optional_objective,
            "Swift Intercept: victory by the end of Round 2."
        );
        assert_eq!(
            (definition.base_reward, definition.optional_reward),
            (500, 150)
        );
        assert_eq!(definition.unlocks, Some(MissionId::Four));

        // The definition builds the same battle as the direct constructor.
        let battle = (definition.build)(7, &SquadUpgrades::default());
        assert_eq!(battle.board().width(), 9);
        assert!(battle.unit(ids::COURIER).is_some());
    }
}
