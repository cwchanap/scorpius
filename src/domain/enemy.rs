use std::{cmp::Reverse, collections::BTreeSet};

use crate::domain::{
    battle::BattleState,
    board::GridPos,
    combat::{AttackPreview, attack_footprint, preview_for_profile},
    model::{
        ActivationState, BattleError, BattleEvent, BattlePhase, Faction, UnitArchetype, UnitId,
        UnitState, WeaponId,
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
        let enemies: Vec<_> = self
            .units()
            .filter(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out())
            .map(|unit| (unit.id, unit.archetype, unit.position))
            .collect();
        let mut events = Vec::new();

        for (id, archetype, origin) in enemies {
            let destination = match archetype {
                UnitArchetype::Rifleman if origin.x < 4 => GridPos::new(2, 5),
                UnitArchetype::Rifleman => GridPos::new(6, 5),
                UnitArchetype::Striker => GridPos::new(4, 6),
                UnitArchetype::Artillery => origin,
                _ => origin,
            };
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
        UnitArchetype::Rifleman | UnitArchetype::Striker => {
            let weapon = unit
                .weapons
                .first()
                .and_then(|weapon| battle.weapon(*weapon))
                .ok_or(BattleError::InvalidTarget(unit.position))?;
            Ok(*candidates
                .iter()
                .min_by_key(|position| {
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
                })
                .expect("origin is always a movement candidate"))
        }
        UnitArchetype::Artillery => {
            let weapon = unit
                .weapons
                .first()
                .and_then(|weapon| battle.weapon(*weapon))
                .ok_or(BattleError::InvalidTarget(unit.position))?;
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
        Some(target) => choice_for_center(battle, weapon.shape, target),
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
    let mut choices: Vec<_> = (0..battle.board().height())
        .flat_map(|y| (0..battle.board().width()).map(move |x| GridPos::new(x, y)))
        .filter(|target| {
            let distance = attacker.position.manhattan(*target);
            distance >= weapon.min_range && distance <= weapon.max_range
        })
        .map(|target| choice_for_center(battle, weapon.shape, target))
        .collect();
    if choices.is_empty() {
        return Err(BattleError::InvalidTarget(attacker.position));
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
) -> TargetChoice {
    let footprint = attack_footprint(battle, shape, center);
    let intended_occupant = living_players(battle)
        .into_iter()
        .filter(|player| footprint.contains(&player.position))
        .min_by_key(player_priority)
        .map(|player| player.id);
    TargetChoice {
        center,
        footprint,
        intended_occupant,
    }
}

fn opening_target(battle: &BattleState, attacker: UnitId) -> Option<GridPos> {
    let enemy = battle.unit(attacker)?;
    let target_archetype = match enemy.archetype {
        UnitArchetype::Striker | UnitArchetype::Artillery => UnitArchetype::Vanguard,
        UnitArchetype::Rifleman if enemy.position.x < 4 => UnitArchetype::Gunner,
        UnitArchetype::Rifleman => UnitArchetype::Interceptor,
        _ => return None,
    };
    battle
        .units()
        .find(|unit| {
            unit.faction == Faction::Player
                && unit.archetype == target_archetype
                && !unit.is_knocked_out()
        })
        .map(|unit| unit.position)
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
        UnitArchetype::Rifleman if unit.position.x < 4 => 20,
        UnitArchetype::Rifleman => 19,
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
            board::GridPos,
            combat::DamageSource,
            model::{BattleEvent, BattlePhase, Reaction},
        },
        mission::mission_one::{ids, mission_one},
    };

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
        battle
            .move_unit(ids::VANGUARD, GridPos::new(3, 7))
            .unwrap();
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
            battle
                .choose_reaction(player, Reaction::Counter)
                .unwrap();
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

    fn isolated_striker_fixture() -> crate::domain::battle::BattleState {
        let mut battle = mission_one(7);
        battle.set_round_for_test(1);
        battle.move_unit_direct_for_test(ids::STRIKER, GridPos::new(0, 0));
        battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(8, 8));
        battle.move_unit_direct_for_test(ids::GUNNER, GridPos::new(7, 8));
        battle.move_unit_direct_for_test(ids::INTERCEPTOR, GridPos::new(8, 7));
        battle
    }

    fn locked_mortar_fixture(seed: u64) -> crate::domain::battle::BattleState {
        let mut battle = mission_one(seed);
        battle.begin_round().unwrap();
        battle
    }
}
