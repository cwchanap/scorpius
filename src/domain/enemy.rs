use std::{cmp::Reverse, collections::BTreeSet};

use crate::domain::{
    battle::BattleState,
    board::GridPos,
    combat::{AttackPreview, attack_footprint, preview_for_profile, weapon_reaches},
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
        // Terminal-check immediately so an escape/extraction move ends the
        // movement pass at once; otherwise later enemies would keep moving
        // (and could trip hazards) before `MissionFailed` is emitted. The
        // post-loop `check_terminal_state` in `begin_round` is a no-op once a
        // result is already locked in.
        events.extend(self.check_terminal_state());
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
        UnitArchetype::Rifleman
        | UnitArchetype::Striker
        | UnitArchetype::Bulwark
        | UnitArchetype::Dreadnought
        | UnitArchetype::Regent => {
            let weapon = unit_weapon(battle, unit)?;
            Ok(attack_band_destination(&candidates, &players, weapon))
        }
        UnitArchetype::Controller => {
            let weapon = unit_weapon(battle, unit)?;
            Ok(controller_destination(&candidates, &players, weapon))
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
        UnitArchetype::Vanguard | UnitArchetype::Gunner | UnitArchetype::Interceptor => {
            Ok(unit.position)
        }
    }
}

/// Controller pressure: hug a reachable push lane — some living player in the
/// weapon's range band AND on the projector's row/column — when one exists,
/// else fall back to the shared attack-band hug.
fn controller_destination(
    candidates: &[GridPos],
    players: &[UnitState],
    weapon: &WeaponSpec,
) -> GridPos {
    let lane: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|position| {
            players
                .iter()
                .any(|player| weapon_reaches(weapon, *position, player.position))
        })
        .collect();
    if lane.is_empty() {
        return attack_band_destination(candidates, players, weapon);
    }
    *lane
        .iter()
        .min_by_key(|position| attack_band_key(**position, players, weapon))
        .expect("the lane filter is non-empty")
}

pub(crate) fn unit_weapon<'a>(
    battle: &'a BattleState,
    unit: &UnitState,
) -> Result<&'a WeaponSpec, BattleError> {
    let index = match unit.archetype {
        UnitArchetype::Dreadnought | UnitArchetype::Regent if unit.hp * 2 <= unit.stats.max_hp => 1,
        _ => 0,
    };
    let id = unit
        .weapons
        .get(index)
        .copied()
        .ok_or(BattleError::InvalidTarget(unit.position))?;
    battle.weapon(id).ok_or(BattleError::UnknownWeapon(id))
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
    let weapon = unit_weapon(battle, attacker)?;
    let weapon_id = weapon.id;
    let choice = match forced_target {
        // Authored openings keep shared occupant selection inside the forced
        // footprint; only the protect-preference path overrides priority.
        Some(target) => choice_for_center(battle, weapon.shape, target, None),
        None => choose_target(battle, attacker, weapon)?,
    };
    // Authored/forced centers never passed the dynamic legality filter, so
    // range and push alignment are enforced here before anything commits.
    if !weapon_reaches(weapon, attacker.position, choice.center) {
        let distance = attacker.position.manhattan(choice.center);
        return Err(
            if distance < weapon.min_range || distance > weapon.max_range {
                BattleError::TargetOutOfRange {
                    attacker: attacker_id,
                    weapon: weapon_id,
                    target: choice.center,
                }
            } else {
                BattleError::PushTargetNotAligned {
                    attacker: attacker.position,
                    target: choice.center,
                }
            },
        );
    }
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
        .filter(|target| weapon_reaches(weapon, attacker.position, *target))
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
        UnitArchetype::Regent => 45,
        UnitArchetype::Dreadnought => 40,
        UnitArchetype::Controller => 35,
        UnitArchetype::Striker => 30,
        UnitArchetype::Flanker => 25,
        UnitArchetype::Rifleman => 20,
        UnitArchetype::Bulwark => 15,
        UnitArchetype::Artillery => 10,
        UnitArchetype::Vanguard | UnitArchetype::Gunner | UnitArchetype::Interceptor => 0,
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
            combat::{DamageSource, weapon_reaches},
            model::{
                BattleError, BattleEvent, BattlePhase, EnemyOpening, Faction, MissionRules,
                OptionalObjective, PrimaryObjective, Reaction, UnitArchetype, UnitId, WeaponId,
                WeaponShape, WeaponSpec,
            },
        },
        mission::{
            enemies,
            mission_one::{ids, mission_one},
            squad,
        },
    };

    use super::{build_intent, choose_enemy_destination, initiative, unit_weapon};

    const FLANKER_COURIER: UnitId = UnitId(21);
    const PROTECTED: UnitId = UnitId(2);
    const DECOY: UnitId = UnitId(3);
    const SHARED_VANGUARD: UnitId = UnitId(2);
    const PROTECT_GUNNER: UnitId = UnitId(3);
    const CONTROLLER: UnitId = UnitId(42);
    const BULWARK: UnitId = UnitId(41);
    const PUSHER: UnitId = UnitId(4);
    const DREADNOUGHT: UnitId = UnitId(90);
    const TEST_PLAYER: UnitId = UnitId(91);
    const GRAVITON: WeaponId = WeaponId(290);
    const OVERLOAD: WeaponId = WeaponId(291);
    const REGENT: UnitId = UnitId(92);
    const REGENT_PLAYER: UnitId = UnitId(93);
    const COMMAND_BARRAGE: WeaponId = WeaponId(292);
    const RUPTURE_BEAM: WeaponId = WeaponId(293);

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
        assert_eq!(
            initiative(&enemies::controller(
                CONTROLLER,
                "Controller",
                GridPos::new(0, 7)
            )),
            35
        );
        assert_eq!(
            initiative(&enemies::bulwark(
                BULWARK,
                "Gate Bulwark",
                GridPos::new(4, 5)
            )),
            15
        );
        let controller = enemies::controller(CONTROLLER, "Controller", GridPos::new(0, 7));
        let dreadnought = squad::unit(
            DREADNOUGHT,
            "Dreadnought",
            UnitArchetype::Dreadnought,
            Faction::Enemy,
            squad::stats(40, 3, 1, 90, 5, 0),
            GridPos::new(3, 1),
            vec![GRAVITON, OVERLOAD],
        );
        assert_eq!(initiative(&dreadnought), 40);
        assert!(initiative(&dreadnought) > initiative(&controller));
        let regent = squad::unit(
            REGENT,
            "Regent",
            UnitArchetype::Regent,
            Faction::Enemy,
            squad::stats(52, 4, 2, 92, 8, 0),
            GridPos::new(3, 1),
            vec![COMMAND_BARRAGE, RUPTURE_BEAM],
        );
        assert_eq!(initiative(&regent), 45);
        assert!(initiative(&regent) > initiative(&dreadnought));
        assert!(initiative(&regent) > initiative(&controller));
    }

    #[test]
    fn controller_steps_onto_a_reachable_aligned_push_lane() {
        // Vanguard at (4,2) puts the shared x=4 column at aligned range 2..4;
        // the Move-2 Controller hugs the closest lane cell (4,4).
        let mut battle = planning_fixture(
            enemies::controller(CONTROLLER, "Controller", GridPos::new(4, 6)),
            enemies::impulse_projector(),
            [(SHARED_VANGUARD, GridPos::new(4, 2))],
        );
        advance_a_later_round(&mut battle);

        assert_eq!(
            battle.unit(CONTROLLER).unwrap().position,
            GridPos::new(4, 4)
        );
    }

    #[test]
    fn controller_without_an_aligned_lane_falls_back_to_the_attack_band() {
        // No reachable cell shares a row or column with the distant player, so
        // the plain band fallback picks the closest Move-2 candidate (2,0).
        let mut battle = planning_fixture(
            enemies::controller(CONTROLLER, "Controller", GridPos::new(0, 0)),
            enemies::impulse_projector(),
            [(SHARED_VANGUARD, GridPos::new(8, 8))],
        );
        advance_a_later_round(&mut battle);

        assert_eq!(
            battle.unit(CONTROLLER).unwrap().position,
            GridPos::new(2, 0)
        );
    }

    #[test]
    fn controller_dynamic_intent_center_satisfies_weapon_reaches() {
        let mut battle = planning_fixture(
            enemies::controller(CONTROLLER, "Controller", GridPos::new(4, 6)),
            enemies::impulse_projector(),
            [(SHARED_VANGUARD, GridPos::new(4, 2))],
        );
        advance_a_later_round(&mut battle);

        let intent = battle.intent_for(CONTROLLER).unwrap();
        let attacker = battle.unit(CONTROLLER).unwrap().position;
        assert!(weapon_reaches(
            &enemies::impulse_projector(),
            attacker,
            intent.footprint[0]
        ));
    }

    #[test]
    fn forced_diagonal_push_target_is_rejected_before_commitment() {
        let battle = planning_fixture(
            enemies::controller(CONTROLLER, "Controller", GridPos::new(4, 4)),
            enemies::impulse_projector(),
            [(SHARED_VANGUARD, GridPos::new(8, 8))],
        );

        // (2,2) is inside the projector's range 2..4 but off-lane from (4,4).
        let error = build_intent(&battle, CONTROLLER, Some(GridPos::new(2, 2))).unwrap_err();
        assert!(matches!(error, BattleError::PushTargetNotAligned { .. }));
    }

    #[test]
    fn bulwark_steps_to_a_better_move_one_attack_band_cell() {
        // Bastion Cannon band 1..3: standing at (4,4) is band 1 from the
        // Vanguard at (4,8); the Move-1 step to (4,5) reaches band 0.
        let mut battle = planning_fixture(
            enemies::bulwark(BULWARK, "Gate Bulwark", GridPos::new(4, 4)),
            enemies::bastion_cannon(),
            [(SHARED_VANGUARD, GridPos::new(4, 8))],
        );
        advance_a_later_round(&mut battle);

        assert_eq!(battle.unit(BULWARK).unwrap().position, GridPos::new(4, 5));
    }

    #[test]
    fn dreadnought_switches_weapon_once_at_half_hp() {
        let mut battle = dreadnought_threshold_fixture();
        assert_eq!(
            unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap())
                .unwrap()
                .id,
            GRAVITON
        );

        battle.apply_direct_damage(DREADNOUGHT, 19, DamageSource::Collision);
        assert_eq!(battle.unit(DREADNOUGHT).unwrap().hp, 21);
        assert_eq!(
            unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap())
                .unwrap()
                .id,
            GRAVITON
        );

        battle.apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
        assert_eq!(battle.unit(DREADNOUGHT).unwrap().hp, 20);
        assert_eq!(
            unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap())
                .unwrap()
                .id,
            OVERLOAD
        );

        battle.apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
        assert_eq!(battle.unit(DREADNOUGHT).unwrap().hp, 19);
        assert_eq!(
            unit_weapon(&battle, battle.unit(DREADNOUGHT).unwrap())
                .unwrap()
                .id,
            OVERLOAD
        );
    }

    #[test]
    fn crossing_threshold_does_not_rewrite_committed_dreadnought_intent() {
        let mut battle = dreadnought_threshold_fixture();
        battle.begin_round().unwrap();
        let committed = battle.intent_for(DREADNOUGHT).unwrap().clone();
        assert_eq!(committed.profile.weapon, GRAVITON);

        battle.apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);

        assert_eq!(battle.intent_for(DREADNOUGHT).unwrap(), &committed);
        let future = build_intent(&battle, DREADNOUGHT, Some(GridPos::new(3, 5))).unwrap();
        assert_eq!(future.profile.weapon, OVERLOAD);
    }

    #[test]
    fn overload_cross_never_contains_its_attacker() {
        let mut battle = dreadnought_range_two_fixture();
        battle.apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);

        let intent = build_intent(&battle, DREADNOUGHT, Some(GridPos::new(3, 3))).unwrap();
        let weapon = battle.weapon(OVERLOAD).unwrap();

        assert_eq!((weapon.min_range, weapon.max_range), (2, 4));
        assert_eq!(intent.profile.weapon, OVERLOAD);
        assert!(!intent.footprint.contains(&GridPos::new(3, 1)));
    }

    #[test]
    fn both_bosses_switch_at_their_exact_half_hp_boundary() {
        let mut regent = regent_threshold_fixture();
        regent.apply_direct_damage(REGENT, 25, DamageSource::Collision);
        assert_eq!(regent.unit(REGENT).unwrap().hp, 27);
        assert_eq!(
            unit_weapon(&regent, regent.unit(REGENT).unwrap())
                .unwrap()
                .id,
            COMMAND_BARRAGE
        );
        regent.apply_direct_damage(REGENT, 1, DamageSource::Collision);
        assert_eq!(regent.unit(REGENT).unwrap().hp, 26);
        assert_eq!(
            unit_weapon(&regent, regent.unit(REGENT).unwrap())
                .unwrap()
                .id,
            RUPTURE_BEAM
        );

        let mut dreadnought = dreadnought_threshold_fixture();
        dreadnought.apply_direct_damage(DREADNOUGHT, 19, DamageSource::Collision);
        assert_eq!(dreadnought.unit(DREADNOUGHT).unwrap().hp, 21);
        assert_eq!(
            unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap())
                .unwrap()
                .id,
            GRAVITON
        );
        dreadnought.apply_direct_damage(DREADNOUGHT, 1, DamageSource::Collision);
        assert_eq!(dreadnought.unit(DREADNOUGHT).unwrap().hp, 20);
        assert_eq!(
            unit_weapon(&dreadnought, dreadnought.unit(DREADNOUGHT).unwrap())
                .unwrap()
                .id,
            OVERLOAD
        );
    }

    #[test]
    fn regent_threshold_crossing_changes_only_future_intents() {
        let mut battle = regent_threshold_fixture();
        battle.begin_round().unwrap();
        let committed = battle.intent_for(REGENT).unwrap().clone();
        assert_eq!(committed.profile.weapon, COMMAND_BARRAGE);

        battle.apply_direct_damage(REGENT, 26, DamageSource::Collision);
        assert_eq!(battle.intent_for(REGENT).unwrap(), &committed);

        let future = build_intent(&battle, REGENT, Some(GridPos::new(3, 5))).unwrap();
        assert_eq!(future.profile.weapon, RUPTURE_BEAM);
    }

    #[test]
    fn dreadnought_overload_closes_from_range_five() {
        let mut battle = dreadnought_close_pressure_fixture();
        battle.apply_direct_damage(DREADNOUGHT, 20, DamageSource::Collision);

        let destination = choose_enemy_destination(&battle, DREADNOUGHT).unwrap();

        assert_eq!(destination, GridPos::new(3, 1));
        assert_eq!(destination.manhattan(GridPos::new(3, 5)), 4);
    }

    fn dreadnought_fixture(boss_at: GridPos, player_at: GridPos) -> BattleState {
        let board = BoardState::new(7, 7, [], [], []);
        let units = vec![
            squad::unit(
                DREADNOUGHT,
                "Dreadnought",
                UnitArchetype::Dreadnought,
                Faction::Enemy,
                squad::stats(40, 3, 1, 90, 5, 0),
                boss_at,
                vec![GRAVITON, OVERLOAD],
            ),
            squad::unit(
                TEST_PLAYER,
                "Player",
                UnitArchetype::Vanguard,
                Faction::Player,
                squad::stats(20, 3, 3, 78, 5, 7),
                player_at,
                vec![],
            ),
        ];
        BattleState::new(
            board,
            units,
            vec![
                squad::weapon(
                    GRAVITON,
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
                ),
                squad::weapon(
                    OVERLOAD,
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
                ),
            ],
            eliminate_rules(),
            7,
        )
    }

    fn dreadnought_threshold_fixture() -> BattleState {
        dreadnought_fixture(GridPos::new(3, 1), GridPos::new(3, 5))
    }

    fn dreadnought_range_two_fixture() -> BattleState {
        dreadnought_fixture(GridPos::new(3, 1), GridPos::new(3, 3))
    }

    fn dreadnought_close_pressure_fixture() -> BattleState {
        dreadnought_fixture(GridPos::new(3, 0), GridPos::new(3, 5))
    }

    fn regent_fixture(boss_at: GridPos, player_at: GridPos) -> BattleState {
        let board = BoardState::new(7, 7, [], [], []);
        let units = vec![
            squad::unit(
                REGENT,
                "Regent",
                UnitArchetype::Regent,
                Faction::Enemy,
                squad::stats(52, 4, 2, 92, 8, 0),
                boss_at,
                vec![COMMAND_BARRAGE, RUPTURE_BEAM],
            ),
            squad::unit(
                REGENT_PLAYER,
                "Player",
                UnitArchetype::Vanguard,
                Faction::Player,
                squad::stats(20, 3, 3, 78, 5, 7),
                player_at,
                vec![],
            ),
        ];
        BattleState::new(
            board,
            units,
            vec![
                squad::weapon(
                    COMMAND_BARRAGE,
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
                ),
                squad::weapon(
                    RUPTURE_BEAM,
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
                ),
            ],
            eliminate_rules(),
            7,
        )
    }

    fn regent_threshold_fixture() -> BattleState {
        regent_fixture(GridPos::new(3, 1), GridPos::new(3, 5))
    }

    fn planning_fixture(
        enemy: crate::domain::model::UnitState,
        weapon: WeaponSpec,
        players: impl IntoIterator<Item = (UnitId, GridPos)>,
    ) -> BattleState {
        let board = BoardState::new(9, 9, [], [], []);
        let mut units = vec![enemy];
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
        BattleState::new(board, units, vec![weapon], eliminate_rules(), 7)
    }

    static DISPLACED_OPENING: [EnemyOpening; 1] = [EnemyOpening {
        unit: CONTROLLER,
        destination: GridPos::new(4, 2),
        target: Some(SHARED_VANGUARD),
    }];

    /// Authored opening pins the Controller on a vertical push lane over the
    /// Vanguard at (4,5); the Pusher at (2,2) can shove the Controller onto
    /// (5,2), breaking that lane perpendicular to the committed footprint.
    fn displaced_controller_fixture(seed: u64) -> BattleState {
        let board = BoardState::new(9, 9, [], [], []);
        let units = vec![
            enemies::controller(CONTROLLER, "Controller", GridPos::new(4, 2)),
            squad::unit(
                SHARED_VANGUARD,
                "Vanguard",
                UnitArchetype::Vanguard,
                Faction::Player,
                // Zero armor/evasion keep the projector's 3 base damage
                // observable; Guard's flat 3 reduction would zero it out.
                squad::stats(20, 0, 3, 78, 0, 7),
                GridPos::new(4, 5),
                vec![],
            ),
            squad::unit(
                PUSHER,
                "Pusher",
                UnitArchetype::Interceptor,
                Faction::Player,
                squad::stats(15, 1, 4, 82, 20, 8),
                GridPos::new(2, 2),
                vec![],
            ),
        ];
        BattleState::new(
            board,
            units,
            vec![enemies::impulse_projector()],
            MissionRules {
                primary: PrimaryObjective::EliminateAllEnemies,
                optional: OptionalObjective::Turnabout,
                opening_plan: &DISPLACED_OPENING,
            },
            seed,
        )
    }

    fn resolve_displaced_controller_round(seed: u64) -> (BattleState, Vec<BattleEvent>) {
        let mut battle = displaced_controller_fixture(seed);
        battle.begin_round().unwrap();
        let intent = battle.intent_for(CONTROLLER).unwrap();
        assert_eq!(intent.footprint[0], GridPos::new(4, 5));
        assert_eq!(intent.intended_occupant, Some(SHARED_VANGUARD));

        battle.resolve_push(PUSHER, CONTROLLER).unwrap();
        assert_eq!(
            battle.unit(CONTROLLER).unwrap().position,
            GridPos::new(5, 2)
        );

        for player in [SHARED_VANGUARD, PUSHER] {
            battle.begin_activation(player).unwrap();
            battle.choose_reaction(player, Reaction::Evade).unwrap();
            battle.finish_activation(player).unwrap();
        }
        let events = battle.resolve_enemy_phase().unwrap();
        (battle, events)
    }

    fn displaced_controller_hit_seed() -> Option<u64> {
        (0..64).find(|&seed| {
            let (_, events) = resolve_displaced_controller_round(seed);
            events.iter().any(|event| {
                matches!(
                    event,
                    BattleEvent::DamageApplied {
                        target,
                        source: DamageSource::EnemyWeapon(attacker, _),
                        ..
                    } if *target == SHARED_VANGUARD && *attacker == CONTROLLER
                )
            })
        })
    }

    #[test]
    fn displaced_controller_resolves_damage_without_its_lost_push() {
        // Bounded sweep for one deterministic seed where the displaced
        // Controller's hit still lands (same pattern as the Aegis sweep).
        let seed = displaced_controller_hit_seed()
            .unwrap_or_else(|| panic!("no seed in 0..64 lands the displaced Controller hit"));
        let (battle, events) = resolve_displaced_controller_round(seed);

        // Normal round completion, not a resolution-phase error.
        assert_eq!(battle.phase(), BattlePhase::Player);
        assert!(battle.result().is_none());

        // The locked attack still rolled against and damaged the Vanguard.
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackRolled {
                attacker,
                target,
                hit: true,
                ..
            } if *attacker == CONTROLLER && *target == SHARED_VANGUARD
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::DamageApplied {
                target,
                source: DamageSource::EnemyWeapon(attacker, weapon),
                ..
            } if *target == SHARED_VANGUARD
                && *attacker == CONTROLLER
                && *weapon == enemies::ids::IMPULSE_PROJECTOR
        )));
        assert_eq!(battle.unit(SHARED_VANGUARD).unwrap().hp, 17);

        // The lost lane means no enemy push of the Vanguard ever occurs.
        assert!(!events.iter().any(|event| matches!(
            event,
            BattleEvent::UnitPushed { unit, .. } if *unit == SHARED_VANGUARD
        )));
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
