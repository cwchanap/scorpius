use crate::campaign::model::SquadUpgrades;
use crate::domain::{
    battle::BattleState,
    board::{BoardState, GridPos},
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

    pub const ARTILLERY_A: UnitId = UnitId(51);
    pub const ARTILLERY_B: UnitId = UnitId(52);
    pub const BULWARK: UnitId = UnitId(53);
    pub const CONTROLLER: UnitId = UnitId(54);
    pub const FLANKER: UnitId = UnitId(55);
}

const MISSION_FIVE_DEPLOYMENT: SquadDeployment = SquadDeployment {
    vanguard: GridPos::new(4, 7),
    gunner: GridPos::new(3, 8),
    interceptor: GridPos::new(5, 8),
};

/// Authored opening: both Artillery batteries stay put and lock Cross1 firing
/// solutions whose footprints share `(3,7)`, while the Bulwark, Controller,
/// and Flanker step into their spec destinations before the player phase.
static MISSION_FIVE_OPENING: [EnemyOpening; 5] = [
    EnemyOpening {
        unit: ids::ARTILLERY_A,
        destination: GridPos::new(3, 0),
        target: Some(ids::GUNNER),
    },
    EnemyOpening {
        unit: ids::ARTILLERY_B,
        destination: GridPos::new(7, 2),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::BULWARK,
        destination: GridPos::new(1, 7),
        target: Some(ids::VANGUARD),
    },
    EnemyOpening {
        unit: ids::CONTROLLER,
        destination: GridPos::new(3, 6),
        target: Some(ids::GUNNER),
    },
    EnemyOpening {
        unit: ids::FLANKER,
        destination: GridPos::new(6, 7),
        target: Some(ids::INTERCEPTOR),
    },
];

const MISSION_FIVE_RULES: MissionRules = MissionRules {
    primary: PrimaryObjective::EliminateAllEnemies,
    optional: OptionalObjective::VictoryByRound { round: 4 },
    opening_plan: &MISSION_FIVE_OPENING,
};

pub fn mission_five(seed: u64) -> BattleState {
    mission_five_for_campaign(seed, &SquadUpgrades::default())
}

pub fn mission_five_for_campaign(seed: u64, upgrades: &SquadUpgrades) -> BattleState {
    let (mut units, mut weapons) = build_player_squad(upgrades, MISSION_FIVE_DEPLOYMENT);
    units.extend(mission_five_enemy_units());
    weapons.extend(mission_five_enemy_weapons());
    BattleState::new(
        mission_five_board(),
        units,
        weapons,
        MISSION_FIVE_RULES,
        seed,
    )
}

fn mission_five_board() -> BoardState {
    BoardState::new(
        9,
        9,
        [
            GridPos::new(1, 4),
            GridPos::new(7, 4),
            GridPos::new(1, 5),
            GridPos::new(7, 5),
        ],
        [],
        [],
    )
}

fn mission_five_enemy_units() -> Vec<UnitState> {
    vec![
        enemies::artillery(ids::ARTILLERY_A, "Siege Artillery A", GridPos::new(3, 0)),
        enemies::artillery(ids::ARTILLERY_B, "Siege Artillery B", GridPos::new(7, 2)),
        enemies::bulwark(ids::BULWARK, "Bulwark", GridPos::new(0, 7)),
        enemies::controller(ids::CONTROLLER, "Controller", GridPos::new(3, 5)),
        enemies::flanker(ids::FLANKER, "Flanker", GridPos::new(8, 7)),
    ]
}

fn mission_five_enemy_weapons() -> Vec<WeaponSpec> {
    vec![
        enemies::siege_mortar(),
        enemies::bastion_cannon(),
        enemies::impulse_projector(),
        enemies::skirmish_carbine(),
    ]
}

static PRE_MISSION_LINES: [DialogueLine; 3] = [
    DialogueLine {
        speaker: "Control",
        text: "Two siege batteries have already locked firing solutions. Their shots will not retarget.",
        portrait: "vn/control_neutral.png",
    },
    DialogueLine {
        speaker: "Vanguard",
        text: "Then every red footprint is also a weapon we can aim.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Exactly. Break the assault before they settle into a second firing line.",
        portrait: "vn/control_alert.png",
    },
];

static AFTERMATH_LINES: [DialogueLine; 2] = [
    DialogueLine {
        speaker: "Vanguard",
        text: "Both batteries are down. Their crossfire did half the work for us.",
        portrait: "vn/vanguard_neutral.png",
    },
    DialogueLine {
        speaker: "Control",
        text: "Regular forces are broken. What comes next is heavier.",
        portrait: "vn/control_neutral.png",
    },
];

pub const MISSION_FIVE_DEFINITION: MissionDefinition = MissionDefinition {
    id: MissionId::Five,
    unlocks: MissionId::Six,
    build: mission_five_for_campaign,
    title: "Mission 5 — Crossfire Break",
    primary_objective: "Break the assault and destroy all enemies.",
    optional_objective: "Rapid Break: win by the end of Round 4.",
    base_reward: 700,
    optional_reward: 200,
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
    use crate::domain::model::{BattleEvent, BattlePhase, Faction, Reaction, UnitId};
    use crate::mission::assert_opening_plan_is_legal;
    use crate::mission::mission_definition;
    use crate::mission::squad;
    use crate::mission::squad::ids::RAIL_RIFLE;

    #[test]
    fn mission_five_authors_the_spec_board_and_roster() {
        let battle = mission_five(7);
        assert_eq!(battle.board().width(), 9);
        assert_eq!(battle.board().height(), 9);
        assert_eq!(
            battle.board().blocking_cells().collect::<Vec<_>>(),
            vec![
                GridPos::new(1, 4),
                GridPos::new(7, 4),
                GridPos::new(1, 5),
                GridPos::new(7, 5),
            ]
        );
        assert_eq!(
            battle.board().hazard_cells().collect::<Vec<_>>(),
            Vec::<GridPos>::new(),
            "no hazard exists on this board"
        );
        assert_eq!(
            battle.board().explosives().count(),
            0,
            "no explosive props exist on this board"
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
            battle.unit(ids::ARTILLERY_A).unwrap().position,
            GridPos::new(3, 0)
        );
        assert_eq!(
            battle.unit(ids::ARTILLERY_B).unwrap().position,
            GridPos::new(7, 2)
        );
        assert_eq!(
            battle.unit(ids::BULWARK).unwrap().position,
            GridPos::new(0, 7)
        );
        assert_eq!(
            battle.unit(ids::CONTROLLER).unwrap().position,
            GridPos::new(3, 5)
        );
        assert_eq!(
            battle.unit(ids::FLANKER).unwrap().position,
            GridPos::new(8, 7)
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
            5
        );

        // Authored payoff baseline: the Controller is exactly one Ram plus one
        // Mortar hit away from KO.
        let controller = battle.unit(ids::CONTROLLER).unwrap();
        assert_eq!(controller.stats.max_hp, 9);
        assert_eq!(controller.hp, 9);
        assert_eq!(controller.stats.armor, 1);
    }

    #[test]
    fn mission_five_rules_and_opening_match_the_spec() {
        let battle = mission_five(7);
        let rules = battle.rules();
        assert_eq!(rules.primary, PrimaryObjective::EliminateAllEnemies);
        assert_eq!(
            rules.optional,
            OptionalObjective::VictoryByRound { round: 4 }
        );

        let expected = [
            (ids::ARTILLERY_A, GridPos::new(3, 0), Some(ids::GUNNER)),
            (ids::ARTILLERY_B, GridPos::new(7, 2), Some(ids::VANGUARD)),
            (ids::BULWARK, GridPos::new(1, 7), Some(ids::VANGUARD)),
            (ids::CONTROLLER, GridPos::new(3, 6), Some(ids::GUNNER)),
            (ids::FLANKER, GridPos::new(6, 7), Some(ids::INTERCEPTOR)),
        ];
        assert_eq!(rules.opening_plan.len(), expected.len());
        for (opening, (unit, destination, target)) in rules.opening_plan.iter().zip(expected) {
            assert_eq!(opening.unit, unit);
            assert_eq!(opening.destination, destination);
            assert_eq!(opening.target, target);
        }
    }

    #[test]
    fn mission_five_opening_rows_reference_legal_units_and_destinations() {
        let battle = mission_five(7);
        assert_opening_plan_is_legal(&battle);
    }

    /// The load-bearing crossfire line: committed Artillery footprints share
    /// `(3,7)`, the exact-fit public movement paths vacate both artillery
    /// targets, and the Ram displaces the Controller onto the shared cell.
    #[test]
    fn committed_crossfire_geometry_survives_the_real_player_line() {
        let mut battle = mission_five(7);
        battle.begin_round().unwrap();

        let artillery_a = battle.intent_for(ids::ARTILLERY_A).unwrap();
        let artillery_b = battle.intent_for(ids::ARTILLERY_B).unwrap();
        assert!(artillery_a.footprint.contains(&GridPos::new(3, 7)));
        assert!(artillery_b.footprint.contains(&GridPos::new(3, 7)));
        assert_eq!(
            battle.unit(ids::CONTROLLER).unwrap().position,
            GridPos::new(3, 6)
        );

        battle.begin_activation(ids::GUNNER).unwrap();
        battle.move_unit(ids::GUNNER, GridPos::new(2, 7)).unwrap();
        battle
            .choose_reaction(ids::GUNNER, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::GUNNER).unwrap();

        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(3, 5)).unwrap();
        let ram = battle
            .preview_attack(ids::VANGUARD, squad::ids::REPULSOR_RAM, GridPos::new(3, 6))
            .unwrap();
        assert_eq!(ram.normal_damage, 4);
        assert_eq!(ram.push_destination, Some(GridPos::new(3, 7)));

        let push_events = battle.resolve_push(ids::VANGUARD, ids::CONTROLLER).unwrap();
        assert_eq!(
            battle.unit(ids::CONTROLLER).unwrap().position,
            GridPos::new(3, 7)
        );
        assert!(
            push_events
                .iter()
                .any(|event| matches!(event, BattleEvent::UnitPushed { .. }))
        );
        assert_eq!(
            battle.unit(ids::GUNNER).unwrap().position,
            GridPos::new(2, 7)
        );
        assert_eq!(
            battle.unit(ids::VANGUARD).unwrap().position,
            GridPos::new(3, 5)
        );

        // The Controller's committed push footprint on Gunner's original cell
        // resolves into empty space: initiative 35 fires first, hits nothing.
        let controller_events = battle.resolve_intent_for_test(ids::CONTROLLER).unwrap();
        assert!(controller_events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackHitEmpty {
                attacker,
                cell,
                ..
            } if *attacker == ids::CONTROLLER && *cell == GridPos::new(3, 8)
        )));

        // Both committed batteries still roll against the displaced Controller
        // on the shared `(3,7)` cell — no retargeting, whatever the dice say.
        for battery in [ids::ARTILLERY_A, ids::ARTILLERY_B] {
            let events = battle.resolve_intent_for_test(battery).unwrap();
            assert!(events.iter().any(|event| matches!(
                event,
                BattleEvent::AttackRolled {
                    attacker,
                    target,
                    ..
                } if *attacker == battery && *target == ids::CONTROLLER
            )));
        }
    }

    fn crossfire_setup(seed: u64) -> BattleState {
        let mut battle = mission_five(seed);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::GUNNER).unwrap();
        battle.move_unit(ids::GUNNER, GridPos::new(2, 7)).unwrap();
        battle
            .choose_reaction(ids::GUNNER, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::GUNNER).unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(3, 5)).unwrap();
        battle.resolve_push(ids::VANGUARD, ids::CONTROLLER).unwrap();
        battle
    }

    fn battery_hits_controller_without_crit(
        battery: UnitId,
        seed: u64,
    ) -> Option<Vec<BattleEvent>> {
        let mut battle = crossfire_setup(seed);
        battle.resolve_intent_for_test(ids::CONTROLLER).unwrap();
        let events = battle.resolve_intent_for_test(battery).unwrap();
        events
            .iter()
            .any(|event| {
                matches!(
                    event,
                    BattleEvent::AttackRolled {
                        attacker,
                        target,
                        hit: true,
                        critical: false,
                        ..
                    } if *attacker == battery && *target == ids::CONTROLLER
                )
            })
            .then_some(events)
    }

    #[test]
    fn mortar_hits_deal_exactly_five_through_armor_one_for_each_battery() {
        for battery in [ids::ARTILLERY_A, ids::ARTILLERY_B] {
            // Bounded sweep for one deterministic seed where the battery lands
            // a non-critical hit (same pattern as the Aegis sweep).
            let events = (0..64)
                .find_map(|seed| battery_hits_controller_without_crit(battery, seed))
                .unwrap_or_else(|| {
                    panic!("no seed in 0..64 lands battery {battery:?} on the Controller")
                });
            assert!(events.iter().any(|event| matches!(
                event,
                BattleEvent::DamageApplied {
                    target,
                    amount: 5,
                    source: DamageSource::EnemyWeapon(attacker, weapon),
                    remaining_hp: 4,
                    ..
                } if *target == ids::CONTROLLER
                    && *attacker == battery
                    && *weapon == enemies::ids::SIEGE_MORTAR
            )));
        }
    }

    #[test]
    fn siege_mortar_preview_pins_five_normal_damage_against_the_controller() {
        let mut battle = mission_five(7);
        battle.begin_round().unwrap();
        let intent = battle.intent_for(ids::ARTILLERY_A).unwrap();
        let preview = intent
            .intended_preview
            .as_ref()
            .expect("opening locks a preview");
        assert_eq!(preview.normal_damage, 5);
        assert_eq!(preview.target, GridPos::new(3, 8));
    }

    /// Rounds step with every player guarding so the lone surviving Flanker
    /// cannot end the battle early; used to walk to the Round-4/5 boundary.
    fn play_one_round(battle: &mut BattleState) {
        for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(player).unwrap();
            battle.choose_reaction(player, Reaction::Guard).unwrap();
            battle.finish_activation(player).unwrap();
        }
        battle.resolve_enemy_phase().unwrap();
    }

    /// Opening round plus the four non-Flanker enemies cleared with
    /// player-fire damage, which never trips an optional trigger.
    fn rapid_break_after_opening() -> BattleState {
        let mut battle = mission_five(7);
        battle.begin_round().unwrap();
        for enemy in [
            ids::ARTILLERY_A,
            ids::ARTILLERY_B,
            ids::BULWARK,
            ids::CONTROLLER,
        ] {
            battle.apply_direct_damage(enemy, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        }
        assert!(battle.result().is_none());
        battle
    }

    #[test]
    fn rapid_break_tracks_the_round_four_boundary() {
        // A round-4 victory is exactly on the boundary: bonus granted.
        let mut inside = rapid_break_after_opening();
        for _ in 1..=3 {
            play_one_round(&mut inside);
        }
        assert_eq!(inside.round(), 4);
        inside.apply_direct_damage(ids::FLANKER, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        assert!(
            inside
                .result()
                .is_some_and(|result| result.victory && result.optional_complete)
        );

        // A round-5 victory misses the boundary: no bonus.
        let mut outside = rapid_break_after_opening();
        for _ in 1..=4 {
            play_one_round(&mut outside);
        }
        assert_eq!(outside.round(), 5);
        outside.apply_direct_damage(ids::FLANKER, 99, DamageSource::PlayerWeapon(RAIL_RIFLE));
        assert!(
            outside
                .result()
                .is_some_and(|result| result.victory && !result.optional_complete)
        );
    }

    #[test]
    fn a_lone_flanker_cannot_end_the_battle_while_players_guard() {
        let mut battle = rapid_break_after_opening();
        for _ in 1..=5 {
            play_one_round(&mut battle);
            assert!(battle.result().is_none(), "the battle continues");
            assert_eq!(battle.phase(), BattlePhase::Player);
            assert!(!battle.unit(ids::FLANKER).unwrap().is_knocked_out());
            assert!(
                !battle
                    .units()
                    .filter(|unit| unit.faction == Faction::Player)
                    .any(|unit| unit.is_knocked_out()),
                "guarding must keep the squad alive against the lone Flanker"
            );
        }
    }

    #[test]
    fn mission_ids_resolve_only_for_authored_missions() {
        assert_eq!(
            mission_definition(MissionId::Five)
                .map(|definition| (definition.id, definition.unlocks)),
            Some((MissionId::Five, MissionId::Six))
        );
        assert!(mission_definition(MissionId::Six).is_none());
    }

    #[test]
    fn mission_five_definition_carries_the_spec_copy_and_rewards() {
        let definition = mission_definition(MissionId::Five).unwrap();
        assert_eq!(definition.title, "Mission 5 — Crossfire Break");
        assert_eq!(
            definition.primary_objective,
            "Break the assault and destroy all enemies."
        );
        assert_eq!(
            definition.optional_objective,
            "Rapid Break: win by the end of Round 4."
        );
        assert_eq!(
            (definition.base_reward, definition.optional_reward),
            (700, 200)
        );
        assert_eq!(definition.unlocks, MissionId::Six);

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
            "Two siege batteries have already locked firing solutions. Their shots will not retarget."
        );
        assert_eq!(
            definition.pre_mission.lines[1].text,
            "Then every red footprint is also a weapon we can aim."
        );
        assert_eq!(
            definition.pre_mission.lines[2].text,
            "Exactly. Break the assault before they settle into a second firing line."
        );
        assert_eq!(definition.aftermath.lines.len(), 2);
        assert_eq!(
            definition.aftermath.lines[0],
            DialogueLine {
                speaker: "Vanguard",
                text: "Both batteries are down. Their crossfire did half the work for us.",
                portrait: "vn/vanguard_neutral.png",
            }
        );
        assert_eq!(
            definition.aftermath.lines[1],
            DialogueLine {
                speaker: "Control",
                text: "Regular forces are broken. What comes next is heavier.",
                portrait: "vn/control_neutral.png",
            }
        );

        // The definition builds the same battle as the direct constructor.
        let battle = (definition.build)(7, &SquadUpgrades::default());
        assert_eq!(battle.board().width(), 9);
        assert!(battle.unit(ids::ARTILLERY_A).is_some());
    }
}
