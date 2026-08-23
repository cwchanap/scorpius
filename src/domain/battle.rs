use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    board::{BoardState, GridPos},
    enemy::AttackIntent,
    model::{
        ActivationState, BattleError, BattleEvent, BattlePhase, Faction, Reaction, UnitArchetype,
        UnitId, UnitState, UnitStats, WeaponId, WeaponSpec,
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

    pub fn reachable_cells(&self, id: UnitId) -> Result<BTreeSet<GridPos>, BattleError> {
        let unit = self.units.get(&id).ok_or(BattleError::UnknownUnit(id))?;
        if unit.is_knocked_out() {
            return Err(BattleError::UnitKnockedOut(id));
        }

        let origin = unit.position;
        let movement = unit.stats.movement;
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
        domain::model::{ActivationState, Reaction},
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
}
