use std::{cmp::Reverse, collections::BTreeSet};

use crate::domain::{
    battle::BattleState,
    board::GridPos,
    combat::{AttackPreview, attack_footprint, preview_for_profile},
    model::{
        ActivationState, BattleError, BattleEvent, BattlePhase, Faction, PrimaryObjective,
        UnitArchetype, UnitId, UnitState, WeaponId, WeaponSpec,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttackProfile {
    pub weapon: WeaponId,
    pub base_damage: i16,
    pub accuracy: i16,
    pub hit_modifier: i16,
    pub crit_chance: u8,
    pub push: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttackIntent {
    pub attacker: UnitId,
    pub origin: GridPos,
    pub profile: AttackProfile,
    pub footprint: Vec<GridPos>,
    pub intended_occupant: Option<UnitId>,
    pub intended_preview: Option<AttackPreview>,
    pub initiative: i16,
}

struct TargetChoice {
    center: GridPos,
    footprint: Vec<GridPos>,
    intended_occupant: Option<UnitId>,
}

impl BattleState {
    pub fn intents(&self) -> &[AttackIntent] {
        &self.intents
    }

    pub fn intent_for(&self, attacker: UnitId) -> Option<&AttackIntent> {
        self.intents
            .iter()
            .find(|intent| intent.attacker == attacker)
    }

    pub fn begin_round(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
        if self.phase != BattlePhase::EnemyPlanning {
            return Err(BattleError::WrongPhase {
                expected: BattlePhase::EnemyPlanning,
                actual: self.phase,
            });
        }

        let mut events = self.check_terminal_state();
        if self.result().is_some() {
            return Ok(events);
        }

        self.active_unit = None;
        self.intents.clear();
        let player_ids: Vec<_> = self
            .units()
            .filter(|unit| unit.faction == Faction::Player)
            .map(|unit| unit.id)
            .collect();
        for id in player_ids {
            let player = self.unit_mut(id).expect("collected player must exist");
            player.activation = ActivationState::default();
            player.reaction = None;
        }

        let opening = self.round == 0;
        if opening {
            events.extend(self.apply_authored_opening_movement()?);
        } else {
            events.extend(self.apply_later_enemy_movement()?);
        }
        events.extend(self.check_terminal_state());
        if self.result().is_some() {
            return Ok(events);
        }
        events.extend(self.commit_enemy_intents(opening)?);
        self.round = self.round.saturating_add(1);
        self.phase = BattlePhase::Player;
        Ok(events)
    }

    pub fn resolve_enemy_phase(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
        self.resolve_enemy_phase_inner()
    }

    fn resolve_enemy_phase_inner(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
        if self.phase != BattlePhase::Player {
            return Err(BattleError::WrongPhase {
                expected: BattlePhase::Player,
                actual: self.phase,
            });
        }
        if !self.ready_to_resolve() {
            return Err(BattleError::EnemyResolutionNotReady);
        }

        let mut events = self.check_terminal_state();
        if self.result().is_some() {
            return Ok(events);
        }

        self.phase = BattlePhase::EnemyResolution;
        let intents = self.intents.clone();
        for intent in &intents {
            events.extend(self.resolve_intent(intent)?);
            if self.result().is_some() {
                return Ok(events);
            }
        }

        // The current round's Aegis shield has been applied during resolution
        // above. Expire it before planning the next round so `build_intent` does
        // not bake the now-expired shield into the next round's previews. The
        // terminal paths above already clear it via `check_terminal_state`.
        self.clear_aegis_target();
        self.phase = BattlePhase::EnemyPlanning;
        events.extend(self.begin_round()?);
        Ok(events)
    }

    fn resolve_intent(&mut self, intent: &AttackIntent) -> Result<Vec<BattleEvent>, BattleError> {
        let attacker = self
            .unit(intent.attacker)
            .ok_or(BattleError::UnknownUnit(intent.attacker))?;
        if attacker.is_knocked_out() {
            let mut events = vec![BattleEvent::IntentCanceled {
                attacker: intent.attacker,
            }];
            events.extend(self.check_terminal_state());
            return Ok(events);
        }

        let mut events = Vec::new();
        let mut seen_cells = BTreeSet::new();
        let mut counter_opportunities = BTreeSet::new();
        for cell in intent
            .footprint
            .iter()
            .copied()
            .filter(|cell| seen_cells.insert(*cell))
        {
            let occupant = self.occupant_at(cell);
            let has_explosive = self.board().has_live_explosive(cell);
            if occupant.is_none() && !has_explosive {
                events.push(BattleEvent::AttackHitEmpty {
                    attacker: intent.attacker,
                    weapon: intent.profile.weapon,
                    cell,
                });
                continue;
            }

            if let Some(target) = occupant {
                let target_faction = self
                    .unit(target)
                    .expect("living occupant must exist")
                    .faction;
                let (attack_events, hit) =
                    self.resolve_enemy_profile_against(intent.attacker, &intent.profile, target)?;
                events.extend(attack_events);

                if hit && target_faction == Faction::Player && counter_opportunities.insert(target)
                {
                    events.extend(self.resolve_counter(target, intent.attacker)?);
                }
            }

            if self.board().has_live_explosive(cell) {
                events.extend(self.damage_explosive_raw(
                    cell,
                    intent.profile.base_damage,
                    crate::domain::combat::DamageSource::EnemyWeapon(
                        intent.attacker,
                        intent.profile.weapon,
                    ),
                )?);
            }
        }
        events.extend(self.check_terminal_state());
        Ok(events)
    }

    #[cfg(test)]
    pub(crate) fn resolve_intent_for_test(
        &mut self,
        attacker: UnitId,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let intent = self
            .intent_for(attacker)
            .cloned()
            .ok_or(BattleError::UnknownUnit(attacker))?;
        self.resolve_intent(&intent)
    }

    fn apply_authored_opening_movement(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
        let plan: Vec<(UnitId, GridPos)> = self
            .rules()
            .opening_plan
            .iter()
            .map(|opening| (opening.unit, opening.destination))
            .collect();
        let mut events = Vec::new();

        for (id, destination) in plan {
            events.extend(self.move_enemy_to(id, destination)?);
            if self.result().is_some() {
                break;
            }
        }
        Ok(events)
    }

    fn apply_later_enemy_movement(&mut self) -> Result<Vec<BattleEvent>, BattleError> {
        let enemy_ids: Vec<_> = self
            .units()
            .filter(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out())
            .map(|unit| unit.id)
            .collect();
        let mut events = Vec::new();

        for id in enemy_ids {
            let destination = choose_enemy_destination(self, id)?;
            events.extend(self.move_enemy_to(id, destination)?);
            if self.result().is_some() {
                break;
            }
        }
        Ok(events)
    }

    fn move_enemy_to(
        &mut self,
        id: UnitId,
        destination: GridPos,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let origin = self.unit(id).ok_or(BattleError::UnknownUnit(id))?.position;
        if origin == destination {
            return Ok(Vec::new());
        }
        self.unit_mut(id)
            .expect("validated enemy must exist")
            .position = destination;
        let mut events = vec![BattleEvent::UnitMoved {
            unit: id,
            from: origin,
            to: destination,
        }];
        events.extend(self.resolve_hazard_if_present(id)?);
        Ok(events)
    }

    fn commit_enemy_intents(
        &mut self,
        authored_opening: bool,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let enemy_ids: Vec<_> = self
            .units()
            .filter(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out())
            .map(|unit| unit.id)
            .collect();
        let mut intents = Vec::with_capacity(enemy_ids.len());

        for attacker in enemy_ids {
            let forced_target = authored_opening
                .then(|| opening_target(self, attacker))
                .flatten();
            intents.push(build_intent(self, attacker, forced_target)?);
        }
        intents.sort_by(|left, right| {
            right
                .initiative
                .cmp(&left.initiative)
                .then_with(|| left.attacker.cmp(&right.attacker))
        });
        self.intents = intents;

        Ok(self
            .intents
            .iter()
            .map(|intent| BattleEvent::IntentCommitted {
                attacker: intent.attacker,
                weapon: intent.profile.weapon,
                footprint: intent.footprint.clone(),
                intended_occupant: intent.intended_occupant,
            })
            .collect())
    }
}

fn choose_enemy_destination(battle: &BattleState, id: UnitId) -> Result<GridPos, BattleError> {
    let unit = battle.unit(id).ok_or(BattleError::UnknownUnit(id))?;
    let players: Vec<_> = living_players(battle);
    if players.is_empty() {
        return Ok(unit.position);
    }

    let mut candidates: Vec<_> = battle.reachable_cells(id)?.into_iter().collect();
    candidates.push(unit.position);
    candidates.sort_by_key(|position| (position.y, position.x));

    match unit.archetype {
        UnitArchetype::Flanker => {
            let weapon = unit_weapon(battle, unit)?;
            Ok(flanker_destination(
                battle,
                unit,
                &candidates,
                &players,
                weapon,
            ))
        }
        UnitArchetype::Rifleman | UnitArchetype::Striker => {
            let weapon = unit_weapon(battle, unit)?;
            Ok(attack_band_destination(&candidates, &players, weapon))
        }
        UnitArchetype::Artillery => {
            let weapon = unit_weapon(battle, unit)?;
            if players.iter().any(|player| {
                let distance = unit.position.manhattan(player.position);
                distance >= weapon.min_range && distance <= weapon.max_range
            }) {
                return Ok(unit.position);
            }
            let lane: Vec<_> = candidates
                .iter()
                .copied()
                .filter(|position| position.x == 4)
                .collect();
            let candidates = if lane.is_empty() { &candidates } else { &lane };
            Ok(*candidates
                .iter()
                .min_by_key(|position| {
                    let nearest = players
                        .iter()
                        .map(|player| position.manhattan(player.position))
                        .min()
                        .unwrap_or(0);
                    (nearest, position.y, position.x)
                })
                .expect("origin is always a movement candidate"))
        }
        _ => Ok(unit.position),
    }
}

fn unit_weapon<'a>(
    battle: &'a BattleState,
    unit: &UnitState,
) -> Result<&'a WeaponSpec, BattleError> {
    unit.weapons
        .first()
        .and_then(|weapon| battle.weapon(*weapon))
        .ok_or(BattleError::InvalidTarget(unit.position))
}

fn attack_band_destination(
    candidates: &[GridPos],
    players: &[UnitState],
    weapon: &WeaponSpec,
) -> GridPos {
    *candidates
        .iter()
        .min_by_key(|position| attack_band_key(**position, players, weapon))
        .expect("origin is always a movement candidate")
}

fn attack_band_key(
    position: GridPos,
    players: &[UnitState],
    weapon: &WeaponSpec,
) -> (u8, u8, u8, u8) {
    let band_distance = players
        .iter()
        .map(|player| {
            distance_to_band(
                position.manhattan(player.position),
                weapon.min_range,
                weapon.max_range,
            )
        })
        .min()
        .unwrap_or(0);
    let nearest = players
        .iter()
        .map(|player| position.manhattan(player.position))
        .min()
        .unwrap_or(0);
    (band_distance, nearest, position.y, position.x)
}

/// Explicit Flanker branches keyed on the mission primary: protect pressure
/// hugs the protected unit's weapon band, the intercepted Courier races the
/// escape point, everything else falls back to the normal attack band. No
/// policy object, no RNG.
fn flanker_destination(
    battle: &BattleState,
    unit: &UnitState,
    candidates: &[GridPos],
    players: &[UnitState],
    weapon: &WeaponSpec,
) -> GridPos {
    match battle.rules().primary {
        PrimaryObjective::ProtectThroughRound { target, .. } => {
            let Some(protect) = players.iter().find(|player| player.id == target) else {
                return attack_band_destination(candidates, players, weapon);
            };
            *candidates
                .iter()
                .min_by_key(|position| {
                    let distance = position.manhattan(protect.position);
                    (
                        distance_to_band(distance, weapon.min_range, weapon.max_range),
                        distance,
                        position.y,
                        position.x,
                    )
                })
                .expect("origin is always a movement candidate")
        }
        PrimaryObjective::InterceptBeforeEscape { target, escape, .. } if target == unit.id => {
            *candidates
                .iter()
                .min_by_key(|position| (position.manhattan(escape), position.y, position.x))
                .expect("origin is always a movement candidate")
        }
        _ => attack_band_destination(candidates, players, weapon),
    }
}

fn build_intent(
    battle: &BattleState,
    attacker_id: UnitId,
    forced_target: Option<GridPos>,
) -> Result<AttackIntent, BattleError> {
    let attacker = battle
        .unit(attacker_id)
        .ok_or(BattleError::UnknownUnit(attacker_id))?;
    let weapon_id = attacker
        .weapons
        .first()
        .copied()
        .ok_or(BattleError::InvalidTarget(attacker.position))?;
    let weapon = battle
        .weapon(weapon_id)
        .ok_or(BattleError::UnknownWeapon(weapon_id))?;
    let choice = match forced_target {
        // Authored openings keep shared occupant selection inside the forced
        // footprint; only the protect-preference path overrides priority.
        Some(target) => choice_for_center(battle, weapon.shape, target, None),
        None => choose_target(battle, attacker, weapon)?,
    };
    let profile = AttackProfile {
        weapon: weapon_id,
        base_damage: weapon.base_damage,
        accuracy: attacker.stats.accuracy,
        hit_modifier: weapon.hit_modifier,
        crit_chance: weapon.crit_chance,
        push: weapon.push,
    };
    let intended_preview = choice.intended_occupant.and_then(|target_id| {
        battle.unit(target_id).map(|target| {
            preview_for_profile(
                attacker_id,
                &profile,
                choice.center,
                choice.footprint.clone(),
                target,
                battle.pilot_skills().aegis_target == Some(target_id),
            )
        })
    });

    Ok(AttackIntent {
        attacker: attacker_id,
        origin: attacker.position,
        profile,
        footprint: choice.footprint,
        intended_occupant: choice.intended_occupant,
        intended_preview,
        initiative: initiative(attacker),
    })
}

fn choose_target(
    battle: &BattleState,
    attacker: &UnitState,
    weapon: &crate::domain::model::WeaponSpec,
) -> Result<TargetChoice, BattleError> {
    let players = living_players(battle);
    // Flanker protect pressure: the mission's protected unit outranks the
    // shared priority when a legal footprint covers it (spec: "prefer Gunner
    // when legal").
    let preferred = match (attacker.archetype, battle.rules().primary) {
        (UnitArchetype::Flanker, PrimaryObjective::ProtectThroughRound { target, .. }) => players
            .iter()
            .find(|player| player.id == target)
            .map(|player| player.id),
        _ => None,
    };
    let mut choices: Vec<_> = (0..battle.board().height())
        .flat_map(|y| (0..battle.board().width()).map(move |x| GridPos::new(x, y)))
        .filter(|target| {
            let distance = attacker.position.manhattan(*target);
            distance >= weapon.min_range && distance <= weapon.max_range
        })
        .map(|target| choice_for_center(battle, weapon.shape, target, preferred))
        .collect();
    if choices.is_empty() {
        return Err(BattleError::InvalidTarget(attacker.position));
    }
    if let Some(preferred) = preferred {
        let covers_preferred = |choice: &TargetChoice| choice.intended_occupant == Some(preferred);
        if choices.iter().any(covers_preferred) {
            choices.retain(covers_preferred);
        }
    }

    if choices
        .iter()
        .any(|choice| choice.intended_occupant.is_some())
    {
        choices.sort_by_key(|choice| {
            let threatened = players
                .iter()
                .filter(|player| choice.footprint.contains(&player.position))
                .count();
            let priority = choice
                .intended_occupant
                .and_then(|id| battle.unit(id))
                .map(player_priority)
                .unwrap_or(u8::MAX);
            (
                Reverse(threatened),
                priority,
                choice.center.y,
                choice.center.x,
            )
        });
    } else {
        choices.sort_by_key(|choice| {
            let distance = choice
                .footprint
                .iter()
                .flat_map(|cell| {
                    players
                        .iter()
                        .map(move |player| cell.manhattan(player.position))
                })
                .min()
                .unwrap_or(0);
            (distance, choice.center.y, choice.center.x)
        });
    }

    Ok(choices.remove(0))
}

fn choice_for_center(
    battle: &BattleState,
    shape: crate::domain::model::WeaponShape,
    center: GridPos,
    preferred: Option<UnitId>,
) -> TargetChoice {
    let footprint = attack_footprint(battle, shape, center);
    let covered: Vec<_> = living_players(battle)
        .into_iter()
        .filter(|player| footprint.contains(&player.position))
        .collect();
    let intended_occupant = covered
        .iter()
        .find(|player| Some(player.id) == preferred)
        .or_else(|| covered.iter().min_by_key(|player| player_priority(player)))
        .map(|player| player.id);
    TargetChoice {
        center,
        footprint,
        intended_occupant,
    }
}

fn opening_target(battle: &BattleState, attacker: UnitId) -> Option<GridPos> {
    let opening = battle
        .rules()
        .opening_plan
        .iter()
        .find(|opening| opening.unit == attacker)?;
    let target = battle
        .unit(opening.target?)
        .filter(|unit| !unit.is_knocked_out())?;
    Some(target.position)
}

fn living_players(battle: &BattleState) -> Vec<UnitState> {
    battle
        .units()
        .filter(|unit| unit.faction == Faction::Player && !unit.is_knocked_out())
        .cloned()
        .collect()
}

fn player_priority(unit: &UnitState) -> u8 {
    match unit.archetype {
        UnitArchetype::Vanguard => 0,
        UnitArchetype::Gunner => 1,
        UnitArchetype::Interceptor => 2,
        _ => u8::MAX,
    }
}

fn initiative(unit: &UnitState) -> i16 {
    match unit.archetype {
        UnitArchetype::Striker => 30,
        UnitArchetype::Flanker => 25,
        UnitArchetype::Rifleman => 20,
        UnitArchetype::Artillery => 10,
        _ => 0,
    }
}

fn distance_to_band(distance: u8, min_range: u8, max_range: u8) -> u8 {
    if distance < min_range {
        min_range - distance
    } else {
        distance.saturating_sub(max_range)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            battle::BattleState,
            board::{BoardState, GridPos},
            combat::DamageSource,
            model::{
                BattleEvent, BattlePhase, Faction, MissionRules, OptionalObjective,
                PrimaryObjective, Reaction, UnitArchetype, UnitId, WeaponId, WeaponShape,
                WeaponSpec,
            },
        },
        mission::{
            enemies,
            mission_one::{ids, mission_one},
            squad,
        },
    };

    use super::initiative;

    const FLANKER_COURIER: UnitId = UnitId(21);
    const PROTECTED: UnitId = UnitId(2);
    const DECOY: UnitId = UnitId(3);
    const SHARED_VANGUARD: UnitId = UnitId(2);
    const PROTECT_GUNNER: UnitId = UnitId(3);

    #[test]
    fn authored_opening_places_four_locked_threats() {
        let mut battle = mission_one(7);
        let events = battle.begin_round().unwrap();

        assert_eq!(
            battle.unit(ids::RIFLEMAN_LEFT).unwrap().position,
            GridPos::new(2, 5)
        );
        assert_eq!(
            battle.unit(ids::RIFLEMAN_RIGHT).unwrap().position,
            GridPos::new(6, 5)
        );
        assert_eq!(
            battle.unit(ids::STRIKER).unwrap().position,
            GridPos::new(4, 6)
        );
        assert_eq!(
            battle.unit(ids::ARTILLERY).unwrap().position,
            GridPos::new(4, 0)
        );
        assert_eq!(battle.round(), 1);
        assert_eq!(battle.phase(), BattlePhase::Player);
        assert_eq!(battle.intents().len(), 4);
        assert_eq!(
            battle
                .intents()
                .iter()
                .map(|intent| intent.attacker)
                .collect::<Vec<_>>(),
            vec![
                ids::STRIKER,
                ids::RIFLEMAN_LEFT,
                ids::RIFLEMAN_RIGHT,
                ids::ARTILLERY,
            ]
        );
        // Every authored opening locks its intended victim.
        let intended: Vec<_> = battle
            .intents()
            .iter()
            .map(|intent| (intent.attacker, intent.intended_occupant))
            .collect();
        assert_eq!(
            intended,
            vec![
                (ids::STRIKER, Some(ids::VANGUARD)),
                (ids::RIFLEMAN_LEFT, Some(ids::GUNNER)),
                (ids::RIFLEMAN_RIGHT, Some(ids::INTERCEPTOR)),
                (ids::ARTILLERY, Some(ids::VANGUARD)),
            ]
        );
        assert!(
            battle
                .intents()
                .iter()
                .all(|intent| intent.intended_preview.is_some())
        );

        let mortar = battle.intent_for(ids::ARTILLERY).unwrap();
        assert_eq!(mortar.intended_occupant, Some(ids::VANGUARD));
        assert_eq!(
            mortar.footprint,
            vec![
                GridPos::new(4, 7),
                GridPos::new(4, 6),
                GridPos::new(3, 7),
                GridPos::new(5, 7),
                GridPos::new(4, 8),
            ]
        );
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::IntentCommitted { attacker, .. } if *attacker == ids::ARTILLERY
        )));
    }

    #[test]
    fn out_of_range_enemy_still_commits_a_legal_empty_footprint() {
        let mut battle = isolated_striker_fixture();
        battle.begin_round().unwrap();
        let intent = battle.intent_for(ids::STRIKER).unwrap();

        assert!(intent.intended_occupant.is_none());
        assert!(
            intent
                .footprint
                .iter()
                .all(|cell| battle.board().contains(*cell))
        );
    }

    #[test]
    fn moved_victim_is_not_retargeted_and_enemy_in_footprint_is_hit() {
        let mut battle = locked_mortar_fixture(2);
        battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(2, 7));
        let striker_hp = battle.unit(ids::STRIKER).unwrap().hp;

        let events = battle.resolve_intent_for_test(ids::ARTILLERY).unwrap();

        assert_eq!(battle.unit(ids::VANGUARD).unwrap().hp, 20);
        assert!(battle.unit(ids::STRIKER).unwrap().hp < striker_hp);
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackHitEmpty { cell, .. } if *cell == GridPos::new(4, 7)
        )));
    }

    #[test]
    fn knocking_out_attacker_cancels_its_pending_intent() {
        let mut battle = locked_mortar_fixture(2);
        battle.apply_direct_damage(
            ids::ARTILLERY,
            99,
            DamageSource::PlayerWeapon(ids::RAIL_RIFLE),
        );

        let events = battle.resolve_intent_for_test(ids::ARTILLERY).unwrap();

        assert_eq!(
            events,
            vec![BattleEvent::IntentCanceled {
                attacker: ids::ARTILLERY,
            }]
        );
    }

    #[test]
    fn moved_in_player_is_hit_without_changing_the_committed_footprint() {
        let mut battle = locked_mortar_fixture(2);
        let committed = battle.intent_for(ids::ARTILLERY).unwrap().footprint.clone();
        battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(2, 7));
        battle.move_unit_direct_for_test(ids::GUNNER, GridPos::new(4, 7));

        battle.resolve_intent_for_test(ids::ARTILLERY).unwrap();

        assert!(battle.unit(ids::GUNNER).unwrap().hp < 12);
        assert_eq!(
            battle.intent_for(ids::ARTILLERY).unwrap().footprint,
            committed
        );
    }

    #[test]
    fn enemy_footprint_hits_the_explosive_once() {
        let mut battle = mission_one(2);
        battle.set_round_for_test(1);
        battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(6, 5));
        battle.move_unit_direct_for_test(ids::GUNNER, GridPos::new(7, 5));
        battle.move_unit_direct_for_test(ids::INTERCEPTOR, GridPos::new(6, 4));
        battle.begin_round().unwrap();
        assert!(
            battle
                .intent_for(ids::ARTILLERY)
                .unwrap()
                .footprint
                .contains(&GridPos::new(6, 6))
        );

        let events = battle.resolve_intent_for_test(ids::ARTILLERY).unwrap();

        assert!(
            battle
                .board()
                .explosive_at(GridPos::new(6, 6))
                .unwrap()
                .exploded
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, BattleEvent::ExplosionTriggered { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn full_enemy_phase_requires_finished_players_and_preserves_intent_order() {
        let mut battle = mission_one(2);
        battle.begin_round().unwrap();
        assert_eq!(
            battle.resolve_enemy_phase(),
            Err(crate::domain::model::BattleError::EnemyResolutionNotReady)
        );

        for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(player).unwrap();
            battle
                .choose_reaction(player, crate::domain::model::Reaction::Guard)
                .unwrap();
            battle.finish_activation(player).unwrap();
        }

        let events = battle.resolve_enemy_phase().unwrap();
        let mut resolved_attackers = Vec::new();
        for event in &events {
            let attacker = match event {
                BattleEvent::AttackRolled { attacker, .. }
                | BattleEvent::AttackHitEmpty { attacker, .. }
                | BattleEvent::IntentCanceled { attacker } => *attacker,
                _ => continue,
            };
            if resolved_attackers.last() != Some(&attacker) {
                resolved_attackers.push(attacker);
            }
        }

        assert_eq!(
            &resolved_attackers[..4],
            &[
                ids::STRIKER,
                ids::RIFLEMAN_LEFT,
                ids::RIFLEMAN_RIGHT,
                ids::ARTILLERY,
            ]
        );
        assert_eq!(battle.round(), 2);
        assert_eq!(battle.phase(), BattlePhase::Player);
    }

    #[test]
    fn terminal_combatant_state_does_not_begin_another_round() {
        let mut battle = locked_mortar_fixture(2);
        for player in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            battle.apply_direct_damage(player, 99, DamageSource::Hazard);
        }

        assert_eq!(battle.phase(), BattlePhase::Defeat);
        assert_eq!(battle.round(), 1);
        assert_eq!(
            battle.resolve_enemy_phase(),
            Err(crate::domain::model::BattleError::WrongPhase {
                expected: BattlePhase::Player,
                actual: BattlePhase::Defeat,
            })
        );
    }

    #[test]
    fn successful_enemy_phase_clears_aegis_target_exactly_once() {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();

        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 8)).unwrap();
        battle.use_aegis(ids::GUNNER).unwrap();
        battle
            .choose_reaction(ids::VANGUARD, crate::domain::model::Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::VANGUARD).unwrap();

        // Errors keep the shielded target.
        assert_eq!(
            battle.resolve_enemy_phase(),
            Err(crate::domain::model::BattleError::EnemyResolutionNotReady)
        );
        assert_eq!(battle.pilot_skills().aegis_target, Some(ids::GUNNER));

        for player in [ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(player).unwrap();
            battle
                .choose_reaction(player, crate::domain::model::Reaction::Counter)
                .unwrap();
            battle.finish_activation(player).unwrap();
        }
        battle.resolve_enemy_phase().unwrap();

        assert_eq!(battle.pilot_skills().aegis_target, None);
        assert!(battle.pilot_skills().aegis_used);
    }

    #[test]
    fn next_round_preview_is_unshielded_after_aegis_expires() {
        // Round 0 commits RIFLEMAN_LEFT onto GUNNER (authored opening target).
        // Aegis is cast on GUNNER, then VANGUARD is knocked out so GUNNER is the
        // only living player in RIFLEMAN_LEFT's range next round. After the
        // enemy phase resolves, the just-expired Aegis must not be baked into
        // the next round's intended preview.
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();

        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(3, 7)).unwrap();
        battle.use_aegis(ids::GUNNER).unwrap();
        battle
            .choose_reaction(ids::VANGUARD, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::VANGUARD).unwrap();
        assert_eq!(battle.pilot_skills().aegis_target, Some(ids::GUNNER));

        // Remove VANGUARD from the next-round target pool so GUNNER (priority 1)
        // becomes the top living target.
        battle.apply_direct_damage(ids::VANGUARD, 99, DamageSource::Hazard);
        assert!(battle.unit(ids::VANGUARD).unwrap().is_knocked_out());

        for player in [ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(player).unwrap();
            battle.choose_reaction(player, Reaction::Counter).unwrap();
            battle.finish_activation(player).unwrap();
        }
        battle.resolve_enemy_phase().unwrap();

        assert_eq!(battle.pilot_skills().aegis_target, None);
        let gunner = battle.unit(ids::GUNNER).unwrap();
        let next = battle
            .intents()
            .iter()
            .find(|intent| intent.intended_occupant == Some(ids::GUNNER))
            .expect("next round must target the formerly shielded GUNNER");
        let preview = next
            .intended_preview
            .as_ref()
            .expect("targeted intent carries a preview");
        let unshielded = (next.profile.base_damage - gunner.stats.armor).max(1);
        assert_eq!(
            preview.normal_damage, unshielded,
            "next-round preview must reflect the expired Aegis, not a 3-point reduction"
        );
        assert_ne!(
            preview.normal_damage,
            (unshielded - 3).max(0),
            "preview must differ from the stale shielded value"
        );
    }

    #[test]
    fn initiative_is_fixed_per_archetype_without_position() {
        assert_eq!(
            initiative(&enemies::striker(UnitId(13), "Striker", GridPos::new(4, 4))),
            30
        );
        assert_eq!(
            initiative(&enemies::flanker(UnitId(21), "Flanker", GridPos::new(0, 6))),
            25
        );
        assert_eq!(
            initiative(&enemies::rifleman(
                UnitId(11),
                "Rifleman L",
                GridPos::new(0, 3)
            )),
            20
        );
        assert_eq!(
            initiative(&enemies::rifleman(
                UnitId(12),
                "Rifleman R",
                GridPos::new(8, 3)
            )),
            20
        );
        assert_eq!(
            initiative(&enemies::artillery(
                UnitId(14),
                "Artillery",
                GridPos::new(4, 0)
            )),
            10
        );
    }

    #[test]
    fn flanker_initiative_slots_between_striker_and_rifleman() {
        let mut battle = squad_fixture(
            eliminate_rules(),
            [
                (UnitId(11), UnitArchetype::Rifleman, GridPos::new(0, 3)),
                (FLANKER_COURIER, UnitArchetype::Flanker, GridPos::new(4, 4)),
                (UnitId(12), UnitArchetype::Striker, GridPos::new(4, 3)),
                (UnitId(13), UnitArchetype::Artillery, GridPos::new(4, 0)),
            ],
            [(PROTECTED, GridPos::new(8, 8))],
        );
        battle.set_round_for_test(1);
        battle.begin_round().unwrap();

        let order: Vec<_> = battle
            .intents()
            .iter()
            .map(|intent| (intent.attacker, intent.initiative))
            .collect();

        // The Flanker commits before the lower-id Rifleman: initiative, not id,
        // drives the order.
        assert_eq!(
            order,
            vec![
                (UnitId(12), 30),
                (FLANKER_COURIER, 25),
                (UnitId(11), 20),
                (UnitId(13), 10),
            ]
        );
    }

    #[test]
    fn flanker_protect_pressure_hugs_the_protected_units_weapon_band() {
        // The protected unit sits at (4,4); the nearer decoy at (1,5) must be
        // ignored. Band 0 of (4,4) with Manhattan 1 is (3,4); fallback band
        // logic would instead hug the decoy at (0,5).
        let mut battle = pressure_fixture(
            protect_rules(PROTECTED),
            GridPos::new(0, 4),
            [(PROTECTED, GridPos::new(4, 4)), (DECOY, GridPos::new(1, 5))],
            &[],
        );
        advance_a_later_round(&mut battle);

        assert_eq!(
            battle.unit(FLANKER_COURIER).unwrap().position,
            GridPos::new(3, 4)
        );
    }

    #[test]
    fn flanker_protect_tie_breaks_band_then_manhattan_then_row_then_column() {
        let rules = protect_rules(PROTECTED);

        // Manhattan level: (2,1), (3,1), and (1,2) are all in band with
        // Manhattan 1 to the protected unit at (2,2); the lower row wins.
        let mut battle = pressure_fixture(
            rules,
            GridPos::new(0, 0),
            [(PROTECTED, GridPos::new(2, 2))],
            &[],
        );
        advance_a_later_round(&mut battle);
        assert_eq!(
            battle.unit(FLANKER_COURIER).unwrap().position,
            GridPos::new(2, 1)
        );

        // Manhattan level: with (2,1) blocked, (1,2) is the only Manhattan-1
        // cell and wins even though lower-row Manhattan-2 cells exist.
        let mut battle = pressure_fixture(
            rules,
            GridPos::new(0, 0),
            [(PROTECTED, GridPos::new(2, 2))],
            &[GridPos::new(2, 1)],
        );
        advance_a_later_round(&mut battle);
        assert_eq!(
            battle.unit(FLANKER_COURIER).unwrap().position,
            GridPos::new(1, 2)
        );

        // Column level: with the Manhattan-1 cells and row-0 (2,0) blocked,
        // (1,1) beats (3,1) on the shared row 1 and (0,2) on row.
        let mut battle = pressure_fixture(
            rules,
            GridPos::new(0, 0),
            [(PROTECTED, GridPos::new(2, 2))],
            &[GridPos::new(1, 2), GridPos::new(2, 1), GridPos::new(2, 0)],
        );
        advance_a_later_round(&mut battle);
        assert_eq!(
            battle.unit(FLANKER_COURIER).unwrap().position,
            GridPos::new(1, 1)
        );
    }

    #[test]
    fn flanker_courier_pressure_races_manhattan_to_the_escape_point() {
        let mut battle = pressure_fixture(
            courier_rules(),
            GridPos::new(0, 6),
            [(DECOY, GridPos::new(8, 8))],
            &[],
        );
        advance_a_later_round(&mut battle);

        let destination = battle.unit(FLANKER_COURIER).unwrap().position;
        assert_eq!(destination, GridPos::new(0, 2));
        assert_eq!(destination.manhattan(GridPos::new(8, 0)), 10);
    }

    #[test]
    fn flanker_protect_intent_locks_the_protected_gunner_over_shared_priority() {
        // One Cross1 center covers BOTH the Vanguard (shared priority 0) and the
        // Gunner; the protect preference must still lock the Gunner.
        let mut battle = intent_fixture(
            protect_rules(PROTECT_GUNNER),
            GridPos::new(5, 5),
            GridPos::new(4, 5),
        );
        advance_a_later_round(&mut battle);

        assert_eq!(
            battle
                .intent_for(FLANKER_COURIER)
                .unwrap()
                .intended_occupant,
            Some(PROTECT_GUNNER)
        );
    }

    #[test]
    fn flanker_fallback_intent_keeps_shared_priority_targeting() {
        // Same geometry without a protect primary: the shared Vanguard-first
        // priority wins even though the Gunner is equally coverable.
        let mut battle = intent_fixture(eliminate_rules(), GridPos::new(5, 5), GridPos::new(4, 5));
        advance_a_later_round(&mut battle);

        assert_eq!(
            battle
                .intent_for(FLANKER_COURIER)
                .unwrap()
                .intended_occupant,
            Some(SHARED_VANGUARD)
        );
    }

    #[test]
    fn flanker_protect_intent_falls_back_when_gunner_is_out_of_footprint() {
        // Gunner alive but every reachable footprint misses her (any cell within
        // move 4 of (4,4) is at least Manhattan 4 from (8,8)); shared targeting
        // must stand. From any plausible landing the Vanguard stays coverable.
        let mut battle = intent_fixture(
            protect_rules(PROTECT_GUNNER),
            GridPos::new(8, 8),
            GridPos::new(7, 5),
        );
        advance_a_later_round(&mut battle);

        assert_eq!(
            battle
                .intent_for(FLANKER_COURIER)
                .unwrap()
                .intended_occupant,
            Some(SHARED_VANGUARD)
        );
    }

    fn band_cross() -> WeaponSpec {
        squad::weapon(
            WeaponId(205),
            "Test Cross",
            1,
            2,
            WeaponShape::Cross1,
            4,
            0,
            0,
            0,
            false,
            false,
        )
    }

    fn intent_fixture(
        rules: MissionRules,
        gunner_at: GridPos,
        vanguard_at: GridPos,
    ) -> BattleState {
        let board = BoardState::new(9, 9, [], [], []);
        let mut units = vec![squad::unit(
            FLANKER_COURIER,
            "Flanker",
            UnitArchetype::Flanker,
            Faction::Enemy,
            squad::stats(8, 0, 4, 82, 30, 0),
            GridPos::new(4, 4),
            vec![WeaponId(205)],
        )];
        units.push(squad::unit(
            SHARED_VANGUARD,
            "Vanguard",
            UnitArchetype::Vanguard,
            Faction::Player,
            squad::stats(20, 3, 3, 78, 5, 7),
            vanguard_at,
            vec![],
        ));
        units.push(squad::unit(
            PROTECT_GUNNER,
            "Gunner",
            UnitArchetype::Gunner,
            Faction::Player,
            squad::stats(12, 0, 3, 70, 10, 7),
            gunner_at,
            vec![],
        ));
        BattleState::new(board, units, vec![band_cross()], rules, 7)
    }

    #[test]
    fn flanker_without_a_pressure_rule_reuses_the_attack_band_fallback() {
        // Same geometry as the protect test: without a protect primary the
        // Flanker hugs the nearest player (the decoy at (1,5), best cell
        // (1,4)) instead of the protected unit at (4,4), whose best cell is
        // (3,4).
        let mut battle = pressure_fixture(
            eliminate_rules(),
            GridPos::new(0, 4),
            [(PROTECTED, GridPos::new(4, 4)), (DECOY, GridPos::new(1, 5))],
            &[],
        );
        advance_a_later_round(&mut battle);

        assert_eq!(
            battle.unit(FLANKER_COURIER).unwrap().position,
            GridPos::new(1, 4)
        );
    }

    fn eliminate_rules() -> MissionRules {
        MissionRules {
            primary: PrimaryObjective::EliminateAllEnemies,
            optional: OptionalObjective::Turnabout,
            opening_plan: &[],
        }
    }

    fn protect_rules(target: UnitId) -> MissionRules {
        MissionRules {
            primary: PrimaryObjective::ProtectThroughRound { target, round: 3 },
            optional: OptionalObjective::Turnabout,
            opening_plan: &[],
        }
    }

    fn courier_rules() -> MissionRules {
        MissionRules {
            primary: PrimaryObjective::InterceptBeforeEscape {
                target: FLANKER_COURIER,
                escape: GridPos::new(8, 0),
                deadline_round: 5,
            },
            optional: OptionalObjective::Turnabout,
            opening_plan: &[],
        }
    }

    fn pressure_fixture(
        rules: MissionRules,
        flanker_at: GridPos,
        players: impl IntoIterator<Item = (UnitId, GridPos)>,
        blocking: &[GridPos],
    ) -> BattleState {
        let board = BoardState::new(9, 9, blocking.iter().copied(), [], []);
        let mut units = vec![enemies::flanker(FLANKER_COURIER, "Flanker", flanker_at)];
        units.extend(players.into_iter().map(|(id, position)| {
            squad::unit(
                id,
                "Player",
                UnitArchetype::Vanguard,
                Faction::Player,
                squad::stats(20, 3, 3, 78, 5, 7),
                position,
                vec![],
            )
        }));
        BattleState::new(board, units, vec![enemies::skirmish_carbine()], rules, 7)
    }

    fn squad_fixture(
        rules: MissionRules,
        enemy_roster: impl IntoIterator<Item = (UnitId, UnitArchetype, GridPos)>,
        players: impl IntoIterator<Item = (UnitId, GridPos)>,
    ) -> BattleState {
        let board = BoardState::new(9, 9, [], [], []);
        let mut units: Vec<_> = enemy_roster
            .into_iter()
            .map(|(id, archetype, position)| match archetype {
                UnitArchetype::Rifleman => enemies::rifleman(id, "Rifleman", position),
                UnitArchetype::Striker => enemies::striker(id, "Striker", position),
                UnitArchetype::Artillery => enemies::artillery(id, "Artillery", position),
                UnitArchetype::Flanker => enemies::flanker(id, "Flanker", position),
                _ => unreachable!("players are built separately"),
            })
            .collect();
        units.extend(players.into_iter().map(|(id, position)| {
            squad::unit(
                id,
                "Player",
                UnitArchetype::Vanguard,
                Faction::Player,
                squad::stats(20, 3, 3, 78, 5, 7),
                position,
                vec![],
            )
        }));
        BattleState::new(
            board,
            units,
            vec![
                enemies::service_rifle(),
                enemies::shock_claw(),
                enemies::siege_mortar(),
                enemies::skirmish_carbine(),
            ],
            rules,
            7,
        )
    }

    fn advance_a_later_round(battle: &mut BattleState) {
        battle.set_round_for_test(1);
        battle.begin_round().unwrap();
    }

    fn isolated_striker_fixture() -> BattleState {
        let mut battle = mission_one(7);
        battle.set_round_for_test(1);
        battle.move_unit_direct_for_test(ids::STRIKER, GridPos::new(0, 0));
        battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(8, 8));
        battle.move_unit_direct_for_test(ids::GUNNER, GridPos::new(7, 8));
        battle.move_unit_direct_for_test(ids::INTERCEPTOR, GridPos::new(8, 7));
        battle
    }

    fn locked_mortar_fixture(seed: u64) -> BattleState {
        let mut battle = mission_one(seed);
        battle.begin_round().unwrap();
        battle
    }
}
