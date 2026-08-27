use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    board::{BoardState, GridPos},
    combat::DamageSource,
    enemy::AttackIntent,
    model::{
        ActivationState, BattleError, BattleEvent, BattlePhase, Faction, MissionResult,
        ObjectiveProgress, PilotSkillState, Reaction, UnitArchetype, UnitId, UnitState, UnitStats,
        WeaponId, WeaponSpec,
    },
    rng::BattleRng,
};

#[derive(Clone, Debug)]
pub struct BattleState {
    board: BoardState,
    units: BTreeMap<UnitId, UnitState>,
    weapons: BTreeMap<WeaponId, WeaponSpec>,
    pub(super) phase: BattlePhase,
    pub(super) round: u16,
    pub(super) active_unit: Option<UnitId>,
    pub(super) intents: Vec<AttackIntent>,
    objectives: ObjectiveProgress,
    pilot_skills: PilotSkillState,
    result: Option<MissionResult>,
    rng: BattleRng,
}

impl BattleState {
    pub(crate) fn new(
        board: BoardState,
        units: impl IntoIterator<Item = UnitState>,
        weapons: impl IntoIterator<Item = WeaponSpec>,
        seed: u64,
    ) -> Self {
        Self {
            board,
            units: units.into_iter().map(|unit| (unit.id, unit)).collect(),
            weapons: weapons
                .into_iter()
                .map(|weapon| (weapon.id, weapon))
                .collect(),
            phase: BattlePhase::EnemyPlanning,
            round: 0,
            active_unit: None,
            intents: Vec::new(),
            objectives: ObjectiveProgress::default(),
            pilot_skills: PilotSkillState::default(),
            result: None,
            rng: BattleRng::seeded(seed),
        }
    }

    pub fn viability_fixture() -> Self {
        let stats = UnitStats {
            max_hp: 20,
            armor: 3,
            movement: 3,
            accuracy: 78,
            evasion: 5,
            max_en: 7,
        };
        let mut battle = Self::new(
            BoardState::empty(3, 3),
            [UnitState {
                id: UnitId(1),
                name: "Vanguard",
                archetype: UnitArchetype::Vanguard,
                faction: Faction::Player,
                stats,
                hp: stats.max_hp,
                en: stats.max_en,
                position: GridPos::new(1, 1),
                weapons: Vec::new(),
                activation: ActivationState::default(),
                reaction: None,
            }],
            [],
            0,
        );
        battle.phase = BattlePhase::Player;
        battle.round = 1;
        battle.active_unit = Some(UnitId(1));
        battle
    }

    pub const fn board(&self) -> &BoardState {
        &self.board
    }

    pub fn units(&self) -> impl Iterator<Item = &UnitState> {
        self.units.values()
    }

    pub fn unit(&self, id: UnitId) -> Option<&UnitState> {
        self.units.get(&id)
    }

    pub fn occupant_at(&self, position: GridPos) -> Option<UnitId> {
        self.units
            .values()
            .find(|unit| !unit.is_knocked_out() && unit.position == position)
            .map(|unit| unit.id)
    }

    pub fn weapons(&self) -> impl Iterator<Item = &WeaponSpec> {
        self.weapons.values()
    }

    pub fn weapon(&self, id: WeaponId) -> Option<&WeaponSpec> {
        self.weapons.get(&id)
    }

    pub const fn phase(&self) -> BattlePhase {
        self.phase
    }

    pub const fn round(&self) -> u16 {
        self.round
    }

    pub const fn active_unit(&self) -> Option<UnitId> {
        self.active_unit
    }

    pub const fn objectives(&self) -> ObjectiveProgress {
        self.objectives
    }

    pub const fn pilot_skills(&self) -> PilotSkillState {
        self.pilot_skills
    }

    pub const fn result(&self) -> Option<MissionResult> {
        self.result
    }

    pub fn begin_activation(&mut self, id: UnitId) -> Result<(), BattleError> {
        self.require_player_phase()?;
        if let Some(active) = self.active_unit {
            return Err(BattleError::ActivationInProgress(active));
        }

        let unit = self.units.get(&id).ok_or(BattleError::UnknownUnit(id))?;
        if unit.faction != Faction::Player {
            return Err(BattleError::UnitNotPlayer(id));
        }
        if unit.is_knocked_out() {
            return Err(BattleError::UnitKnockedOut(id));
        }
        if unit.activation.finished {
            return Err(BattleError::ActivationAlreadyFinished(id));
        }

        self.active_unit = Some(id);
        Ok(())
    }

    pub fn movement_allowance(&self, id: UnitId) -> Result<u8, BattleError> {
        let unit = self.units.get(&id).ok_or(BattleError::UnknownUnit(id))?;
        if unit.is_knocked_out() {
            return Err(BattleError::UnitKnockedOut(id));
        }
        let overdrive = self.pilot_skills.overdrive_active
            && unit.archetype == UnitArchetype::Interceptor
            && self.active_unit == Some(id);
        Ok(unit.stats.movement + u8::from(overdrive) * 2)
    }

    pub fn reachable_cells(&self, id: UnitId) -> Result<BTreeSet<GridPos>, BattleError> {
        let origin = self
            .units
            .get(&id)
            .ok_or(BattleError::UnknownUnit(id))?
            .position;
        let movement = self.movement_allowance(id)?;
        let mut reachable = BTreeSet::new();
        let mut visited = BTreeSet::from([origin]);
        let mut frontier = VecDeque::from([(origin, 0_u8)]);

        while let Some((position, distance)) = frontier.pop_front() {
            if distance == movement {
                continue;
            }
            for neighbor in position.orthogonal_neighbors(self.board.width(), self.board.height()) {
                if visited.contains(&neighbor) || !self.is_open_for(id, neighbor) {
                    continue;
                }
                visited.insert(neighbor);
                reachable.insert(neighbor);
                frontier.push_back((neighbor, distance + 1));
            }
        }

        Ok(reachable)
    }

    pub fn move_unit(&mut self, id: UnitId, to: GridPos) -> Result<Vec<BattleEvent>, BattleError> {
        self.require_player_phase()?;
        self.require_active(id)?;
        if !self.board.contains(to) {
            return Err(BattleError::OutOfBounds(to));
        }

        let unit = self.units.get(&id).ok_or(BattleError::UnknownUnit(id))?;
        if unit.activation.moved {
            return Err(BattleError::MoveAlreadySpent(id));
        }
        if self.units.values().any(|occupant| {
            occupant.id != id && !occupant.is_knocked_out() && occupant.position == to
        }) {
            return Err(BattleError::DestinationOccupied(to));
        }
        if !self.reachable_cells(id)?.contains(&to) {
            return Err(BattleError::DestinationUnreachable {
                unit: id,
                destination: to,
            });
        }

        let unit = self
            .units
            .get_mut(&id)
            .expect("unit existence validated above");
        let from = unit.position;
        unit.position = to;
        unit.activation.moved = true;

        let mut events = vec![BattleEvent::UnitMoved { unit: id, from, to }];
        events.extend(self.resolve_hazard_if_present(id)?);
        Ok(events)
    }

    pub fn choose_reaction(&mut self, id: UnitId, reaction: Reaction) -> Result<(), BattleError> {
        self.require_player_phase()?;
        self.require_active(id)?;
        self.units
            .get_mut(&id)
            .expect("active unit must exist")
            .reaction = Some(reaction);
        Ok(())
    }

    /// Aegis (Vanguard): the next enemy resolution applies one non-stacking
    /// Guard-equivalent reduction to this living orthogonally adjacent ally.
    pub fn use_aegis(&mut self, ally: UnitId) -> Result<(), BattleError> {
        self.require_player_phase()?;
        let active = self.active_unit.ok_or(BattleError::NoUnitSelected)?;
        let vanguard = self
            .units
            .get(&active)
            .ok_or(BattleError::UnknownUnit(active))?;
        if vanguard.archetype != UnitArchetype::Vanguard {
            return Err(BattleError::PilotSkillWrongUnit(active));
        }
        if self.pilot_skills.aegis_used {
            return Err(BattleError::PilotSkillAlreadyUsed);
        }
        let ally_unit = self
            .units
            .get(&ally)
            .ok_or(BattleError::InvalidAegisTarget(ally))?;
        let shieldable = ally_unit.faction == Faction::Player
            && !ally_unit.is_knocked_out()
            && vanguard.position.manhattan(ally_unit.position) == 1;
        if !shieldable {
            return Err(BattleError::InvalidAegisTarget(ally));
        }

        self.pilot_skills.aegis_used = true;
        self.pilot_skills.aegis_target = Some(ally);
        Ok(())
    }

    /// Focus (Gunner): the Gunner's next committed player Action attack hits.
    pub fn use_focus(&mut self) -> Result<(), BattleError> {
        self.require_player_phase()?;
        let active = self.active_unit.ok_or(BattleError::NoUnitSelected)?;
        let gunner = self
            .units
            .get(&active)
            .ok_or(BattleError::UnknownUnit(active))?;
        if gunner.archetype != UnitArchetype::Gunner {
            return Err(BattleError::PilotSkillWrongUnit(active));
        }
        if self.pilot_skills.focus_used {
            return Err(BattleError::PilotSkillAlreadyUsed);
        }

        self.pilot_skills.focus_used = true;
        self.pilot_skills.focus_pending = true;
        Ok(())
    }

    /// Overdrive (Interceptor): +2 movement for this activation only.
    pub fn use_overdrive(&mut self) -> Result<(), BattleError> {
        self.require_player_phase()?;
        let active = self.active_unit.ok_or(BattleError::NoUnitSelected)?;
        let interceptor = self
            .units
            .get(&active)
            .ok_or(BattleError::UnknownUnit(active))?;
        if interceptor.archetype != UnitArchetype::Interceptor {
            return Err(BattleError::PilotSkillWrongUnit(active));
        }
        if self.pilot_skills.overdrive_used {
            return Err(BattleError::PilotSkillAlreadyUsed);
        }
        if interceptor.activation.moved {
            return Err(BattleError::PilotSkillRequiresMoveAvailable(active));
        }

        self.pilot_skills.overdrive_used = true;
        self.pilot_skills.overdrive_active = true;
        Ok(())
    }

    pub fn finish_activation(&mut self, id: UnitId) -> Result<(), BattleError> {
        self.require_player_phase()?;
        self.require_active(id)?;
        let unit = self.units.get_mut(&id).expect("active unit must exist");
        if unit.reaction.is_none() {
            return Err(BattleError::ReactionRequired(id));
        }
        unit.activation = ActivationState {
            moved: true,
            acted: true,
            finished: true,
        };
        self.active_unit = None;
        self.pilot_skills.overdrive_active = false;
        Ok(())
    }

    pub fn ready_to_resolve(&self) -> bool {
        self.phase == BattlePhase::Player
            && self.active_unit.is_none()
            && self
                .units
                .values()
                .filter(|unit| unit.faction == Faction::Player)
                .all(|unit| unit.is_knocked_out() || unit.activation.finished)
    }

    fn require_player_phase(&self) -> Result<(), BattleError> {
        if self.phase != BattlePhase::Player {
            return Err(BattleError::WrongPhase {
                expected: BattlePhase::Player,
                actual: self.phase,
            });
        }
        Ok(())
    }

    fn require_active(&self, id: UnitId) -> Result<(), BattleError> {
        if self.active_unit != Some(id) {
            return Err(BattleError::UnitNotActive(id));
        }
        Ok(())
    }

    pub(super) fn is_open_for(&self, mover: UnitId, position: GridPos) -> bool {
        !self.board.is_blocking(position)
            && !self.board.has_live_explosive(position)
            && !self
                .units
                .values()
                .any(|unit| unit.id != mover && !unit.is_knocked_out() && unit.position == position)
    }

    pub(super) fn unit_mut(&mut self, id: UnitId) -> Option<&mut UnitState> {
        self.units.get_mut(&id)
    }

    pub(super) fn board_mut(&mut self) -> &mut BoardState {
        &mut self.board
    }

    pub(super) fn clear_active_unit_if(&mut self, id: UnitId) {
        if self.active_unit == Some(id) {
            self.active_unit = None;
        }
    }

    pub(super) fn clear_aegis_target(&mut self) {
        self.pilot_skills.aegis_target = None;
    }

    pub(super) fn clear_focus_pending(&mut self) {
        self.pilot_skills.focus_pending = false;
    }

    pub(super) fn observe_damage_for_objectives(
        &mut self,
        target: UnitId,
        amount: i16,
        source: DamageSource,
    ) -> Vec<BattleEvent> {
        let damages_enemy = amount > 0
            && self
                .units
                .get(&target)
                .is_some_and(|unit| unit.faction == Faction::Enemy);
        let qualifies = matches!(
            source,
            DamageSource::EnemyWeapon(_, _)
                | DamageSource::Collision
                | DamageSource::Hazard
                | DamageSource::Explosion
        );
        if !damages_enemy || !qualifies || self.objectives.turnabout_complete {
            return Vec::new();
        }

        self.objectives.turnabout_complete = true;
        vec![BattleEvent::OptionalObjectiveCompleted]
    }

    pub(super) fn check_terminal_state(&mut self) -> Vec<BattleEvent> {
        if self.result.is_some() {
            return Vec::new();
        }

        let any_living_player = self
            .units
            .values()
            .any(|unit| unit.faction == Faction::Player && !unit.is_knocked_out());
        let any_living_enemy = self
            .units
            .values()
            .any(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out());
        let victory = if any_living_player && !any_living_enemy {
            Some(true)
        } else if !any_living_player {
            Some(false)
        } else {
            None
        };

        let Some(victory) = victory else {
            return Vec::new();
        };
        let result = MissionResult {
            victory,
            turnabout_complete: self.objectives.turnabout_complete,
            rounds: self.round,
        };
        self.phase = if victory {
            BattlePhase::Victory
        } else {
            BattlePhase::Defeat
        };
        self.active_unit = None;
        self.result = Some(result);

        vec![if victory {
            BattleEvent::MissionCompleted { result }
        } else {
            BattleEvent::MissionFailed { result }
        }]
    }

    pub(super) fn roll_percent(&mut self) -> u8 {
        self.rng.roll_percent()
    }

    #[cfg(test)]
    pub(crate) fn enter_player_phase_for_test(&mut self) {
        self.phase = BattlePhase::Player;
        self.active_unit = None;
    }

    #[cfg(test)]
    pub(crate) fn move_unit_direct_for_test(&mut self, id: UnitId, position: GridPos) {
        self.units
            .get_mut(&id)
            .expect("test unit must exist")
            .position = position;
    }

    #[cfg(test)]
    pub(crate) fn unit_mut_for_test(&mut self, id: UnitId) -> Option<&mut UnitState> {
        self.units.get_mut(&id)
    }

    #[cfg(test)]
    pub(crate) fn set_round_for_test(&mut self, round: u16) {
        self.round = round;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            combat::DamageSource,
            model::{ActivationState, MissionResult, Reaction},
        },
        mission::mission_one::{ids, mission_one},
    };

    #[test]
    fn pure_battle_state_moves_without_bevy() {
        let mut battle = BattleState::viability_fixture();
        let events = battle
            .move_unit(UnitId(1), GridPos::new(1, 2))
            .expect("adjacent open cell is legal");

        assert_eq!(battle.unit(UnitId(1)).unwrap().position, GridPos::new(1, 2));
        assert_eq!(
            events,
            vec![BattleEvent::UnitMoved {
                unit: UnitId(1),
                from: GridPos::new(1, 1),
                to: GridPos::new(1, 2),
            }]
        );
    }

    #[test]
    fn player_chooses_free_order_but_each_unit_moves_once() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        battle.begin_activation(ids::INTERCEPTOR).unwrap();
        battle
            .move_unit(ids::INTERCEPTOR, GridPos::new(5, 7))
            .unwrap();

        assert_eq!(
            battle.move_unit(ids::INTERCEPTOR, GridPos::new(6, 7)),
            Err(BattleError::MoveAlreadySpent(ids::INTERCEPTOR))
        );
        assert_eq!(
            battle.begin_activation(ids::VANGUARD),
            Err(BattleError::ActivationInProgress(ids::INTERCEPTOR))
        );

        battle
            .choose_reaction(ids::INTERCEPTOR, Reaction::Evade)
            .unwrap();
        battle.finish_activation(ids::INTERCEPTOR).unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
    }

    #[test]
    fn finishing_skips_unused_move_and_action() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        battle.begin_activation(ids::GUNNER).unwrap();
        battle
            .choose_reaction(ids::GUNNER, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::GUNNER).unwrap();

        let gunner = battle.unit(ids::GUNNER).unwrap();
        assert_eq!(
            gunner.activation,
            ActivationState {
                moved: true,
                acted: true,
                finished: true,
            }
        );
    }

    #[test]
    fn reachable_cells_respect_terrain_props_and_living_units() {
        let battle = mission_one(7);
        let reachable = battle.reachable_cells(ids::INTERCEPTOR).unwrap();

        assert!(reachable.contains(&GridPos::new(5, 7)));
        assert!(!reachable.contains(&GridPos::new(5, 5)));
        assert!(!reachable.contains(&GridPos::new(6, 6)));
        assert!(!reachable.contains(&GridPos::new(4, 7)));
        assert!(!reachable.contains(&GridPos::new(3, 8)));
        assert!(!reachable.contains(&GridPos::new(5, 8)));
    }

    #[test]
    fn every_living_player_needs_a_reaction_before_resolution() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();

        for id in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(id).unwrap();
            assert_eq!(
                battle.finish_activation(id),
                Err(BattleError::ReactionRequired(id))
            );
            battle.choose_reaction(id, Reaction::Counter).unwrap();
            battle.finish_activation(id).unwrap();
        }

        assert!(battle.ready_to_resolve());
    }

    #[test]
    fn enemy_or_environment_damage_to_enemy_completes_turnabout() {
        for source in [
            DamageSource::EnemyWeapon(ids::ARTILLERY, ids::SIEGE_MORTAR),
            DamageSource::Collision,
            DamageSource::Hazard,
            DamageSource::Explosion,
        ] {
            let mut battle = mission_one(7);
            let events = battle.apply_direct_damage(ids::STRIKER, 1, source);

            assert!(battle.objectives().turnabout_complete, "source {source:?}");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, BattleEvent::OptionalObjectiveCompleted))
                    .count(),
                1
            );
            let later_events = battle.apply_direct_damage(ids::STRIKER, 1, source);
            assert!(
                !later_events
                    .iter()
                    .any(|event| matches!(event, BattleEvent::OptionalObjectiveCompleted))
            );
        }
    }

    #[test]
    fn player_weapon_damage_alone_does_not_complete_turnabout() {
        let mut battle = mission_one(7);
        battle.apply_direct_damage(ids::STRIKER, 1, DamageSource::PlayerWeapon(ids::PILE_LANCE));

        assert!(!battle.objectives().turnabout_complete);
    }

    #[test]
    fn victory_failure_and_restart_are_clean() {
        let mut battle = mission_one(7);
        let victory_events = knock_out_all_enemies(&mut battle);
        assert_eq!(
            battle.result(),
            Some(MissionResult {
                victory: true,
                turnabout_complete: false,
                rounds: 0,
            })
        );
        assert_eq!(
            victory_events
                .iter()
                .filter(|event| matches!(event, BattleEvent::MissionCompleted { .. }))
                .count(),
            1
        );

        battle = mission_one(11);
        assert_eq!(battle.phase(), BattlePhase::EnemyPlanning);
        assert_eq!(battle.round(), 0);
        assert!(
            battle
                .units()
                .all(|unit| unit.hp == unit.stats.max_hp && unit.en == unit.stats.max_en)
        );
        assert!(battle.intents().is_empty());
        assert!(!battle.objectives().turnabout_complete);

        let failure_events = knock_out_all_players(&mut battle);
        assert_eq!(battle.phase(), BattlePhase::Defeat);
        assert_eq!(
            battle.result(),
            Some(MissionResult {
                victory: false,
                turnabout_complete: false,
                rounds: 0,
            })
        );
        assert_eq!(
            failure_events
                .iter()
                .filter(|event| matches!(event, BattleEvent::MissionFailed { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn aegis_requires_active_vanguard_and_living_adjacent_ally() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();

        battle.begin_activation(ids::GUNNER).unwrap();
        assert_eq!(
            battle.use_aegis(ids::VANGUARD),
            Err(BattleError::PilotSkillWrongUnit(ids::GUNNER))
        );
        battle
            .choose_reaction(ids::GUNNER, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::GUNNER).unwrap();

        battle.begin_activation(ids::VANGUARD).unwrap();
        // Deployment keeps Gunner two cells away from the Vanguard.
        assert_eq!(
            battle.use_aegis(ids::GUNNER),
            Err(BattleError::InvalidAegisTarget(ids::GUNNER))
        );
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 8)).unwrap();
        battle.apply_direct_damage(ids::INTERCEPTOR, 99, DamageSource::Hazard);
        assert_eq!(
            battle.use_aegis(ids::INTERCEPTOR),
            Err(BattleError::InvalidAegisTarget(ids::INTERCEPTOR))
        );

        battle.use_aegis(ids::GUNNER).unwrap();
        assert_eq!(battle.pilot_skills().aegis_target, Some(ids::GUNNER));
        assert!(battle.pilot_skills().aegis_used);
        assert_eq!(
            battle.use_aegis(ids::GUNNER),
            Err(BattleError::PilotSkillAlreadyUsed)
        );
    }

    #[test]
    fn focus_requires_active_gunner_and_sets_pending_with_used() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        assert_eq!(battle.use_focus(), Err(BattleError::NoUnitSelected));

        battle.begin_activation(ids::VANGUARD).unwrap();
        assert_eq!(
            battle.use_focus(),
            Err(BattleError::PilotSkillWrongUnit(ids::VANGUARD))
        );
        battle
            .choose_reaction(ids::VANGUARD, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::VANGUARD).unwrap();

        battle.begin_activation(ids::GUNNER).unwrap();
        battle.use_focus().unwrap();
        assert!(battle.pilot_skills().focus_used);
        assert!(battle.pilot_skills().focus_pending);
        assert_eq!(battle.use_focus(), Err(BattleError::PilotSkillAlreadyUsed));
    }

    #[test]
    fn overdrive_extends_interceptor_movement_once_per_mission() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();

        battle.begin_activation(ids::VANGUARD).unwrap();
        assert_eq!(
            battle.use_overdrive(),
            Err(BattleError::PilotSkillWrongUnit(ids::VANGUARD))
        );
        battle
            .choose_reaction(ids::VANGUARD, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::VANGUARD).unwrap();

        battle.begin_activation(ids::INTERCEPTOR).unwrap();
        assert_eq!(battle.movement_allowance(ids::INTERCEPTOR).unwrap(), 4);
        assert!(
            !battle
                .reachable_cells(ids::INTERCEPTOR)
                .unwrap()
                .contains(&GridPos::new(8, 6))
        );
        battle
            .move_unit(ids::INTERCEPTOR, GridPos::new(5, 7))
            .unwrap();
        assert_eq!(
            battle.use_overdrive(),
            Err(BattleError::PilotSkillRequiresMoveAvailable(
                ids::INTERCEPTOR
            ))
        );

        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        battle.begin_activation(ids::INTERCEPTOR).unwrap();
        battle.use_overdrive().unwrap();
        assert!(battle.pilot_skills().overdrive_used);
        assert!(battle.pilot_skills().overdrive_active);
        assert_eq!(
            battle.use_overdrive(),
            Err(BattleError::PilotSkillAlreadyUsed)
        );
        assert_eq!(battle.movement_allowance(ids::INTERCEPTOR).unwrap(), 6);
        assert_eq!(battle.movement_allowance(ids::GUNNER).unwrap(), 2);
        assert!(
            battle
                .reachable_cells(ids::INTERCEPTOR)
                .unwrap()
                .contains(&GridPos::new(8, 6))
        );
        battle
            .move_unit(ids::INTERCEPTOR, GridPos::new(8, 6))
            .unwrap();
        battle
            .choose_reaction(ids::INTERCEPTOR, Reaction::Evade)
            .unwrap();
        battle.finish_activation(ids::INTERCEPTOR).unwrap();
        assert!(!battle.pilot_skills().overdrive_active);
        assert!(battle.pilot_skills().overdrive_used);
        assert_eq!(battle.movement_allowance(ids::INTERCEPTOR).unwrap(), 4);
    }

    fn knock_out_all_enemies(battle: &mut BattleState) -> Vec<BattleEvent> {
        let mut events = Vec::new();
        for enemy in [
            ids::RIFLEMAN_LEFT,
            ids::RIFLEMAN_RIGHT,
            ids::STRIKER,
            ids::ARTILLERY,
        ] {
            events.extend(battle.apply_direct_damage(
                enemy,
                99,
                DamageSource::PlayerWeapon(ids::PILE_LANCE),
            ));
        }
        events
    }

    fn knock_out_all_players(battle: &mut BattleState) -> Vec<BattleEvent> {
        let mut events = Vec::new();
        for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            events.extend(battle.apply_direct_damage(player, 99, DamageSource::Hazard));
        }
        events
    }
}
