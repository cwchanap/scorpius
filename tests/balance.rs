//! Headless Act 1 balance simulation.
//!
//! Three scripted player policies play the authored campaign through the
//! public domain API, the same commands a person issues through the HUD:
//!
//! - `Novice` attacks the best visible target, rotates through the three
//!   reactions, fires Focus on its first attack, activates the roster in
//!   order, and ignores enemy telegraphs.
//! - `Reader` also steps out of committed footprints, picks the reaction that
//!   minimizes locked incoming damage, and values the push previews (collision
//!   and hazard damage) the HUD shows.
//! - `Tactician` also values pushing enemies into other enemies' locked fire
//!   and detonations: the "turn their plan against them" hook.
//!
//! Bots score options from attack previews and committed intents only; they
//! never read the battle RNG. The preview math is mirrored so bots can score
//! cells they have not moved to; `bot_strike_math_matches_the_domain_previews`
//! and `bot_reaction_math_matches_resolved_enemy_fire` pin the mirror to the
//! domain. Every seed is fixed, so the report and the guard rails below are
//! deterministic.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use scorpius::campaign::model::{CampaignState, PlayerMech, UpgradeTrack};
use scorpius::campaign::progression::UPGRADE_COSTS;
use scorpius::domain::battle::BattleState;
use scorpius::domain::board::GridPos;
use scorpius::domain::combat::DamageSource;
use scorpius::domain::enemy::{AttackIntent, AttackProfile};
use scorpius::domain::model::{
    BattleEvent, BattlePhase, Faction, PrimaryObjective, Reaction, UnitArchetype, UnitId,
    UnitState, WeaponId, WeaponShape, WeaponSpec,
};
use scorpius::mission::{MissionDefinition, MissionId, mission_definition};

const MISSIONS: [MissionId; 7] = [
    MissionId::One,
    MissionId::Two,
    MissionId::Three,
    MissionId::Four,
    MissionId::Five,
    MissionId::Six,
    MissionId::Seven,
];
/// Rounds after which a battle that has not ended counts as a loss.
const ROUND_CAP: u16 = 25;
/// Retries before a campaign run gives up on a mission.
const ATTEMPT_CAP: u32 = 12;

const PLAYER_DAMAGE_WEIGHT: f64 = 1.2;
const KO_PENALTY: f64 = 10.0;
const WIN_VALUE: f64 = 100.0;
const KILL_VALUE: f64 = 4.0;
const APPROACH_WEIGHT: f64 = 0.4;
const FOCUS_RESERVE: f64 = 4.0;
const OVERDRIVE_RESERVE: f64 = 2.0;
const AEGIS_THRESHOLD: f64 = 1.5;
const COLLISION_DAMAGE: f64 = 3.0;
const HAZARD_DAMAGE: f64 = 3.0;
const EXPLOSION_DAMAGE: i16 = 4;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Policy {
    Novice,
    Reader,
    Tactician,
}

impl Policy {
    const ALL: [Self; 3] = [Self::Novice, Self::Reader, Self::Tactician];

    fn reads_telegraphs(self) -> bool {
        self != Self::Novice
    }

    fn turns_intents(self) -> bool {
        self == Self::Tactician
    }

    fn name(self) -> &'static str {
        match self {
            Self::Novice => "Novice",
            Self::Reader => "Reader",
            Self::Tactician => "Tactician",
        }
    }
}

// ---------------------------------------------------------------------------
// Expected-value math mirroring `combat.rs` (previews, not rolls).

fn chance(value: i16) -> f64 {
    f64::from(value.clamp(5, 95)) / 100.0
}

#[derive(Clone, Copy)]
struct Strike {
    hit: f64,
    normal: i16,
    critical: i16,
    crit: f64,
}

impl Strike {
    fn expected(self, hp: i16) -> f64 {
        self.hit
            * ((1.0 - self.crit) * f64::from(self.normal.min(hp))
                + self.crit * f64::from(self.critical.min(hp)))
    }

    fn kill(self, hp: i16) -> f64 {
        let lethal = |damage: i16| if damage >= hp { 1.0 } else { 0.0 };
        self.hit * ((1.0 - self.crit) * lethal(self.normal) + self.crit * lethal(self.critical))
    }
}

fn player_strike(
    attacker: &UnitState,
    weapon: &WeaponSpec,
    defender: &UnitState,
    focus: bool,
) -> Strike {
    let hit = if focus {
        1.0
    } else {
        chance(attacker.stats.accuracy + weapon.hit_modifier - defender.stats.evasion)
    };
    Strike {
        hit,
        normal: (weapon.base_damage - defender.stats.armor).max(1),
        critical: (weapon.base_damage + weapon.base_damage / 2 - defender.stats.armor).max(1),
        crit: f64::from(weapon.crit_chance) / 100.0,
    }
}

fn enemy_strike(
    profile: &AttackProfile,
    defender: &UnitState,
    reaction: Option<Reaction>,
    aegis: bool,
) -> Strike {
    let evade = if reaction == Some(Reaction::Evade) {
        25
    } else {
        0
    };
    let guard = if reaction == Some(Reaction::Guard) || aegis {
        3
    } else {
        0
    };
    Strike {
        hit: chance(profile.accuracy + profile.hit_modifier - defender.stats.evasion - evade),
        normal: ((profile.base_damage - defender.stats.armor).max(1) - guard).max(0),
        critical: ((profile.base_damage + profile.base_damage / 2 - defender.stats.armor).max(1)
            - guard)
            .max(0),
        crit: f64::from(profile.crit_chance) / 100.0,
    }
}

fn reaches(weapon: &WeaponSpec, from: GridPos, to: GridPos) -> bool {
    let distance = from.manhattan(to);
    distance >= weapon.min_range
        && distance <= weapon.max_range
        && (!weapon.push || from.x == to.x || from.y == to.y)
}

fn band_distance(distance: u8, weapon: &WeaponSpec) -> u8 {
    if distance < weapon.min_range {
        weapon.min_range - distance
    } else {
        distance.saturating_sub(weapon.max_range)
    }
}

fn cross(battle: &BattleState, center: GridPos) -> Vec<GridPos> {
    let mut cells = vec![center];
    cells.extend(center.orthogonal_neighbors(battle.board().width(), battle.board().height()));
    cells
}

fn weapon_footprint(battle: &BattleState, weapon: &WeaponSpec, center: GridPos) -> Vec<GridPos> {
    match weapon.shape {
        WeaponShape::Single => vec![center],
        WeaponShape::Cross1 => cross(battle, center),
    }
}

/// The one-cell displacement destination, or `None` when it leaves the board.
fn push_destination(battle: &BattleState, from: GridPos, target: GridPos) -> Option<GridPos> {
    let x = i16::from(target.x) + (i16::from(target.x) - i16::from(from.x)).signum();
    let y = i16::from(target.y) + (i16::from(target.y) - i16::from(from.y)).signum();
    let cell = GridPos::new(u8::try_from(x).ok()?, u8::try_from(y).ok()?);
    battle.board().contains(cell).then_some(cell)
}

fn living(battle: &BattleState, faction: Faction) -> Vec<&UnitState> {
    battle
        .units()
        .filter(|unit| unit.faction == faction && !unit.is_knocked_out())
        .collect()
}

fn affordable_weapons<'a>(battle: &'a BattleState, unit: &UnitState) -> Vec<&'a WeaponSpec> {
    unit.weapons
        .iter()
        .filter_map(|id| battle.weapon(*id))
        .filter(|weapon| weapon.en_cost <= unit.en)
        .collect()
}

// ---------------------------------------------------------------------------
// Plans and scoring.

type Kills = BTreeMap<UnitId, f64>;

#[derive(Clone, Debug)]
struct Plan {
    unit: UnitId,
    attack: Option<(WeaponId, GridPos)>,
    /// Cell the attack is fired from.
    attack_from: GridPos,
    /// Final cell after the activation.
    destination: GridPos,
    attack_first: bool,
    focus: bool,
    overdrive: bool,
    reaction: Reaction,
    score: f64,
}

struct Situation<'a> {
    battle: &'a BattleState,
    policy: Policy,
    intents: Vec<AttackIntent>,
    /// Cells inside the blast of an explosive that committed fire will detonate.
    blast_cells: Vec<GridPos>,
}

impl<'a> Situation<'a> {
    fn new(battle: &'a BattleState, policy: Policy) -> Self {
        let intents: Vec<_> = battle
            .intents()
            .iter()
            .filter(|intent| {
                battle
                    .unit(intent.attacker)
                    .is_some_and(|unit| !unit.is_knocked_out())
            })
            .cloned()
            .collect();
        let mut blast_cells = Vec::new();
        for explosive in battle.board().explosives().filter(|e| !e.exploded) {
            let incoming: i16 = intents
                .iter()
                .filter(|intent| intent.footprint.contains(&explosive.position))
                .map(|intent| intent.profile.base_damage)
                .sum();
            if incoming >= explosive.hp {
                blast_cells.extend(cross(battle, explosive.position));
            }
        }
        Self {
            battle,
            policy,
            intents,
            blast_cells,
        }
    }

    fn objective_target(&self) -> Option<UnitId> {
        match self.battle.rules().primary {
            PrimaryObjective::EliminateTarget { target }
            | PrimaryObjective::InterceptBeforeEscape { target, .. } => Some(target),
            PrimaryObjective::EliminateAllEnemies
            | PrimaryObjective::ProtectThroughRound { .. } => None,
        }
    }

    fn protected(&self) -> Option<UnitId> {
        match self.battle.rules().primary {
            PrimaryObjective::ProtectThroughRound { target, .. } => Some(target),
            _ => None,
        }
    }

    fn escape(&self) -> Option<(UnitId, GridPos)> {
        match self.battle.rules().primary {
            PrimaryObjective::InterceptBeforeEscape { target, escape, .. } => {
                Some((target, escape))
            }
            _ => None,
        }
    }

    fn enemy_weight(&self, enemy: UnitId) -> f64 {
        match (self.objective_target(), self.escape()) {
            (_, Some((courier, _))) if courier == enemy => 3.0,
            (Some(target), _) if target == enemy => 2.5,
            _ => 1.0,
        }
    }

    /// Damage to a mech matters more the closer it already is to a knockout,
    /// and three times as much for a mission's protected unit.
    fn player_weight(&self, player: UnitId) -> f64 {
        let wounds = self.battle.unit(player).map_or(0.0, |unit| {
            1.0 - f64::from(unit.hp) / f64::from(unit.stats.max_hp.max(1))
        });
        let protect = if self.protected() == Some(player) {
            3.0
        } else {
            1.0
        };
        protect * (1.0 + wounds)
    }

    fn wins_on_kill(&self, enemy: UnitId) -> bool {
        match self.battle.rules().primary {
            PrimaryObjective::EliminateAllEnemies
            | PrimaryObjective::ProtectThroughRound { .. } => living(self.battle, Faction::Enemy)
                .iter()
                .all(|unit| unit.id == enemy),
            PrimaryObjective::EliminateTarget { target }
            | PrimaryObjective::InterceptBeforeEscape { target, .. } => target == enemy,
        }
    }

    /// Expected damage an enemy's committed intent deals to players where
    /// they stand now.
    fn intent_threat(&self, enemy: UnitId) -> f64 {
        let Some(intent) = self.intents.iter().find(|intent| intent.attacker == enemy) else {
            return 0.0;
        };
        living(self.battle, Faction::Player)
            .iter()
            .filter(|player| intent.footprint.contains(&player.position))
            .map(|player| {
                enemy_strike(&intent.profile, player, None, false).expected(player.hp)
                    * PLAYER_DAMAGE_WEIGHT
                    * self.player_weight(player.id)
            })
            .sum()
    }

    fn kill_value(&self, enemy: UnitId) -> f64 {
        if self.wins_on_kill(enemy) {
            WIN_VALUE
        } else if self.policy.reads_telegraphs() {
            KILL_VALUE + 0.6 * self.intent_threat(enemy)
        } else {
            KILL_VALUE
        }
    }

    /// Locked fire (and certain detonations) an enemy standing on `cell` takes.
    fn friendly_fire(&self, enemy: &UnitState, cell: GridPos) -> f64 {
        let fire: f64 = self
            .intents
            .iter()
            .filter(|intent| intent.footprint.contains(&cell))
            .map(|intent| enemy_strike(&intent.profile, enemy, None, false).expected(enemy.hp))
            .sum();
        let blast = if self.blast_cells.contains(&cell) {
            f64::from(EXPLOSION_DAMAGE.min(enemy.hp))
        } else {
            0.0
        };
        fire + blast
    }

    fn cell_is_open(&self, mover: UnitId, cell: GridPos) -> bool {
        !self.battle.board().is_blocking(cell)
            && !self.battle.board().has_live_explosive(cell)
            && self
                .battle
                .occupant_at(cell)
                .is_none_or(|occupant| occupant == mover)
    }

    fn push_value(&self, attacker_cell: GridPos, enemy: &UnitState, pushed: f64) -> f64 {
        if !self.policy.reads_telegraphs() || pushed <= 0.0 {
            return 0.0;
        }
        let weight = self.enemy_weight(enemy.id);
        let destination = push_destination(self.battle, attacker_cell, enemy.position)
            .filter(|cell| self.cell_is_open(enemy.id, *cell));
        let mut value = 0.0;
        if let Some((courier, escape)) = self.escape()
            && courier == enemy.id
        {
            let landed = destination.unwrap_or(enemy.position);
            value += 1.5
                * (f64::from(landed.manhattan(escape))
                    - f64::from(enemy.position.manhattan(escape)));
        }
        match destination {
            None => value += COLLISION_DAMAGE * weight,
            Some(cell) => {
                if self.battle.board().is_hazard(cell) {
                    value += HAZARD_DAMAGE * weight;
                }
                if self.policy.turns_intents() {
                    value += weight
                        * (self.friendly_fire(enemy, cell)
                            - self.friendly_fire(enemy, enemy.position));
                }
            }
        }
        pushed * value
    }

    /// Value of firing `weapon` at `center` from `from`, plus the kill odds it
    /// creates for the positional score.
    fn attack_value(
        &self,
        unit: &UnitState,
        from: GridPos,
        weapon: &WeaponSpec,
        center: GridPos,
        focus: bool,
    ) -> (f64, Kills) {
        let mut attacker = unit.clone();
        attacker.position = from;
        let mut value = 0.0;
        let mut kills = Kills::new();
        for cell in weapon_footprint(self.battle, weapon, center) {
            if let Some(enemy) = self
                .battle
                .occupant_at(cell)
                .and_then(|id| self.battle.unit(id))
                .filter(|unit| unit.faction == Faction::Enemy)
            {
                let strike = player_strike(&attacker, weapon, enemy, focus);
                let kill = strike.kill(enemy.hp);
                value += strike.expected(enemy.hp) * self.enemy_weight(enemy.id)
                    + kill * self.kill_value(enemy.id);
                *kills.entry(enemy.id).or_default() += kill;
                if weapon.push && cell == center {
                    value += self.push_value(from, enemy, strike.hit - kill);
                }
            }
            if let Some(explosive) = self
                .battle
                .board()
                .explosive_at(cell)
                .filter(|explosive| !explosive.exploded && explosive.hp <= weapon.base_damage)
            {
                value += self.blast_value(unit.id, from, explosive.position, &mut kills);
            }
        }
        (value, kills)
    }

    fn blast_value(
        &self,
        acting: UnitId,
        acting_cell: GridPos,
        explosive: GridPos,
        kills: &mut Kills,
    ) -> f64 {
        let mut value = 0.0;
        for cell in cross(self.battle, explosive) {
            let occupant = if cell == acting_cell {
                Some(acting)
            } else {
                self.battle.occupant_at(cell).filter(|id| *id != acting)
            };
            let Some(unit) = occupant.and_then(|id| self.battle.unit(id)) else {
                continue;
            };
            let damage = f64::from(EXPLOSION_DAMAGE.min(unit.hp));
            let lethal = if unit.hp <= EXPLOSION_DAMAGE {
                1.0
            } else {
                0.0
            };
            match unit.faction {
                Faction::Enemy => {
                    let alive = 1.0 - kills.get(&unit.id).copied().unwrap_or(0.0);
                    value += alive
                        * (damage * self.enemy_weight(unit.id) + lethal * self.kill_value(unit.id));
                    *kills.entry(unit.id).or_default() += alive * lethal;
                }
                Faction::Player => {
                    value -= damage * PLAYER_DAMAGE_WEIGHT * self.player_weight(unit.id)
                        + lethal * KO_PENALTY;
                }
            }
        }
        value
    }

    /// Value of ending the activation on `cell` with the best reaction.
    fn position_value(
        &self,
        unit: &UnitState,
        cell: GridPos,
        moved: bool,
        kills: &Kills,
    ) -> (f64, Reaction) {
        let weight = self.player_weight(unit.id);
        let hazard = if moved && cell != unit.position && self.battle.board().is_hazard(cell) {
            let lethal = if f64::from(unit.hp) <= HAZARD_DAMAGE {
                KO_PENALTY
            } else {
                0.0
            };
            -HAZARD_DAMAGE * PLAYER_DAMAGE_WEIGHT * weight - lethal
        } else {
            0.0
        };
        let base = hazard + self.approach_value(unit, cell);
        if !self.policy.reads_telegraphs() {
            return (base, novice_reaction(unit, self.battle.round()));
        }

        [Reaction::Guard, Reaction::Evade, Reaction::Counter]
            .into_iter()
            .map(|reaction| {
                let (damage, knockout, counter) = self.threat(unit, cell, reaction, false, kills);
                let protect_ko = if self.protected() == Some(unit.id) {
                    WIN_VALUE
                } else {
                    KO_PENALTY
                };
                (
                    base - damage * PLAYER_DAMAGE_WEIGHT * weight - knockout * protect_ko + counter,
                    reaction,
                )
            })
            .max_by(|left, right| left.0.total_cmp(&right.0))
            .expect("three reactions")
    }

    /// Expected damage, knockout odds, and counter value for `unit` on `cell`.
    fn threat(
        &self,
        unit: &UnitState,
        cell: GridPos,
        reaction: Reaction,
        aegis: bool,
        kills: &Kills,
    ) -> (f64, f64, f64) {
        let mut damage = 0.0;
        let mut knockout: f64 = 0.0;
        let mut counter = 0.0;
        for intent in self
            .intents
            .iter()
            .filter(|intent| intent.footprint.contains(&cell))
        {
            let alive = 1.0 - kills.get(&intent.attacker).copied().unwrap_or(0.0);
            let strike = enemy_strike(&intent.profile, unit, Some(reaction), aegis);
            damage += alive * strike.expected(unit.hp);
            knockout += alive * strike.kill(unit.hp);
            if reaction == Reaction::Counter {
                counter += alive * strike.hit * self.counter_value(unit, cell, intent.attacker);
            }
        }
        if self.blast_cells.contains(&cell) {
            damage += f64::from(EXPLOSION_DAMAGE.min(unit.hp));
            if unit.hp <= EXPLOSION_DAMAGE {
                knockout += 1.0;
            }
        }
        (damage, knockout.min(1.0), counter)
    }

    fn counter_value(&self, unit: &UnitState, cell: GridPos, attacker: UnitId) -> f64 {
        let Some(enemy) = self.battle.unit(attacker) else {
            return 0.0;
        };
        let mut defender = unit.clone();
        defender.position = cell;
        affordable_weapons(self.battle, unit)
            .into_iter()
            .find(|weapon| weapon.counter_weapon && reaches(weapon, cell, enemy.position))
            // A counter only fires if the locked hit lands first, so it is a
            // consolation, not a reason to stand in the footprint.
            .map(|weapon| 0.5 * player_strike(&defender, weapon, enemy, false).expected(enemy.hp))
            .unwrap_or(0.0)
    }

    /// Small pull toward the closest firing band and the objective target.
    fn approach_value(&self, unit: &UnitState, cell: GridPos) -> f64 {
        let weapons = affordable_weapons(self.battle, unit);
        let band = |enemy: &UnitState| {
            weapons
                .iter()
                .map(|weapon| band_distance(cell.manhattan(enemy.position), weapon))
                .min()
                .unwrap_or_else(|| cell.manhattan(enemy.position))
        };
        let enemies = living(self.battle, Faction::Enemy);
        let nearest = enemies.iter().map(|enemy| band(enemy)).min().unwrap_or(0);
        let target = self
            .objective_target()
            .and_then(|id| enemies.iter().find(|enemy| enemy.id == id))
            .map_or(0, |enemy| band(enemy));
        -APPROACH_WEIGHT * f64::from(nearest + target)
    }

    fn attack_options(&self, unit: &UnitState, from: GridPos) -> Vec<(WeaponId, GridPos)> {
        let mut centers: Vec<GridPos> = living(self.battle, Faction::Enemy)
            .iter()
            .map(|enemy| enemy.position)
            .collect();
        centers.extend(
            self.battle
                .board()
                .explosives()
                .filter(|explosive| !explosive.exploded)
                .map(|explosive| explosive.position),
        );
        let mut options = Vec::new();
        for weapon in affordable_weapons(self.battle, unit) {
            for center in &centers {
                if reaches(weapon, from, *center) {
                    options.push((weapon.id, *center));
                }
            }
        }
        options
    }
}

fn reachable(battle: &BattleState, unit: UnitId, overdrive: bool) -> Vec<GridPos> {
    let mut probe = battle.clone();
    probe
        .begin_activation(unit)
        .expect("planner only probes activatable units");
    if overdrive {
        probe
            .use_overdrive()
            .expect("overdrive availability checked");
    }
    let origin = battle.unit(unit).expect("planned unit exists").position;
    let mut cells: Vec<_> = probe
        .reachable_cells(unit)
        .expect("planned unit is alive")
        .into_iter()
        .collect();
    cells.push(origin);
    cells
}

fn best_plan(battle: &BattleState, policy: Policy, id: UnitId) -> Plan {
    let situation = Situation::new(battle, policy);
    let unit = battle.unit(id).expect("planned unit exists").clone();
    let origin = unit.position;
    let skills = battle.pilot_skills();
    let can_focus = unit.archetype == UnitArchetype::Gunner && !skills.focus_used;
    let can_overdrive = unit.archetype == UnitArchetype::Interceptor && !skills.overdrive_used;
    let none = Kills::new();

    let mut plans = Vec::new();
    let overdrive_options: &[bool] = if can_overdrive {
        &[false, true]
    } else {
        &[false]
    };
    // Novices fire Focus with their first attack; planners hold it in reserve.
    let focus_options: &[bool] = match (can_focus, policy) {
        (false, _) => &[false],
        (true, Policy::Novice) => &[true],
        (true, _) => &[false, true],
    };
    for &overdrive in overdrive_options {
        let reserve = if overdrive && policy != Policy::Novice {
            OVERDRIVE_RESERVE
        } else {
            0.0
        };
        let cells = reachable(battle, id, overdrive);
        for &cell in &cells {
            let (value, reaction) = situation.position_value(&unit, cell, true, &none);
            plans.push(Plan {
                unit: id,
                attack: None,
                attack_from: origin,
                destination: cell,
                attack_first: false,
                focus: false,
                overdrive,
                reaction,
                score: value - reserve,
            });
        }
        for &focus in focus_options {
            let reserve = reserve
                + if focus && policy != Policy::Novice {
                    FOCUS_RESERVE
                } else {
                    0.0
                };
            for &from in &cells {
                for (weapon, center) in situation.attack_options(&unit, from) {
                    let spec = battle.weapon(weapon).expect("owned weapon");
                    let (value, kills) = situation.attack_value(&unit, from, spec, center, focus);
                    let (position, reaction) =
                        situation.position_value(&unit, from, from != origin, &kills);
                    plans.push(Plan {
                        unit: id,
                        attack: Some((weapon, center)),
                        attack_from: from,
                        destination: from,
                        attack_first: false,
                        focus,
                        overdrive,
                        reaction,
                        score: value + position - reserve,
                    });
                    if from == origin {
                        let (position, destination, reaction) = cells
                            .iter()
                            .map(|cell| {
                                let (value, reaction) =
                                    situation.position_value(&unit, *cell, true, &kills);
                                (value, *cell, reaction)
                            })
                            .max_by(|left, right| left.0.total_cmp(&right.0))
                            .expect("origin is always a cell");
                        plans.push(Plan {
                            unit: id,
                            attack: Some((weapon, center)),
                            attack_from: origin,
                            destination,
                            attack_first: true,
                            focus,
                            overdrive,
                            reaction,
                            score: value + position - reserve,
                        });
                    }
                }
            }
        }
    }

    if policy == Policy::Novice {
        // A novice reaches for Overdrive only when nothing is in reach without it.
        let has_plain_attack = plans
            .iter()
            .any(|plan| plan.attack.is_some() && !plan.overdrive);
        if has_plain_attack {
            plans.retain(|plan| !plan.overdrive);
        }
    }

    plans
        .into_iter()
        .max_by(|left, right| left.score.total_cmp(&right.score))
        .expect("staying put is always a plan")
}

fn stay_score(battle: &BattleState, policy: Policy, id: UnitId) -> f64 {
    let situation = Situation::new(battle, policy);
    let unit = battle.unit(id).expect("planned unit exists");
    situation
        .position_value(unit, unit.position, false, &Kills::new())
        .0
}

/// Use Aegis on an adjacent finished ally that will eat locked fire unguarded.
fn aegis_target(battle: &BattleState, policy: Policy, vanguard: UnitId) -> Option<UnitId> {
    if !policy.reads_telegraphs() || battle.pilot_skills().aegis_used {
        return None;
    }
    let situation = Situation::new(battle, policy);
    let position = battle.unit(vanguard)?.position;
    living(battle, Faction::Player)
        .into_iter()
        .filter(|ally| {
            ally.id != vanguard
                && ally.activation.finished
                && ally.position.manhattan(position) == 1
                && ally.reaction != Some(Reaction::Guard)
        })
        .map(|ally| {
            let reaction = ally.reaction.unwrap_or(Reaction::Guard);
            let none = Kills::new();
            let open = situation
                .threat(ally, ally.position, reaction, false, &none)
                .0;
            let shielded = situation
                .threat(ally, ally.position, reaction, true, &none)
                .0;
            (
                (open - shielded) * situation.player_weight(ally.id),
                ally.id,
            )
        })
        .filter(|(value, _)| *value > AEGIS_THRESHOLD)
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, id)| id)
}

// ---------------------------------------------------------------------------
// Battle driver and per-battle statistics.

#[derive(Clone, Debug, Default)]
struct BattleStats {
    victory: bool,
    bonus: bool,
    rounds: u16,
    hp_left: i32,
    hp_max: i32,
    knockouts: u32,
    intents: u32,
    intents_on_players: u32,
    intents_turned: u32,
    intents_empty: u32,
    intents_canceled: u32,
    enemy_damage: i32,
    enemy_damage_by_enemy_fire: i32,
    enemy_damage_by_environment: i32,
    focus_damage: i32,
    gunner_damage: i32,
    bot_errors: u32,
}

impl BattleStats {
    fn record_damage(&mut self, battle: &BattleState, events: &[BattleEvent]) {
        for event in events {
            let BattleEvent::DamageApplied {
                target,
                amount,
                source,
                ..
            } = event
            else {
                continue;
            };
            if battle
                .unit(*target)
                .is_none_or(|unit| unit.faction != Faction::Enemy)
            {
                continue;
            }
            let amount = i32::from(*amount);
            self.enemy_damage += amount;
            match source {
                DamageSource::EnemyWeapon(..) => self.enemy_damage_by_enemy_fire += amount,
                DamageSource::Collision | DamageSource::Hazard | DamageSource::Explosion => {
                    self.enemy_damage_by_environment += amount;
                }
                DamageSource::PlayerWeapon(weapon) => {
                    if is_gunner_weapon(battle, *weapon) {
                        self.gunner_damage += amount;
                    }
                }
            }
        }
    }

    fn record_resolution(
        &mut self,
        battle: &BattleState,
        attackers: &[UnitId],
        events: &[BattleEvent],
    ) {
        for attacker in attackers {
            let mut touched = false;
            let mut on_player = false;
            let mut on_enemy = false;
            let mut canceled = false;
            for event in events {
                match event {
                    BattleEvent::IntentCanceled { attacker: id } if id == attacker => {
                        touched = true;
                        canceled = true;
                    }
                    BattleEvent::AttackHitEmpty { attacker: id, .. } if id == attacker => {
                        touched = true;
                    }
                    BattleEvent::AttackRolled {
                        attacker: id,
                        target,
                        ..
                    } if id == attacker => {
                        touched = true;
                        match battle.unit(*target).map(|unit| unit.faction) {
                            Some(Faction::Player) => on_player = true,
                            Some(Faction::Enemy) => on_enemy = true,
                            None => {}
                        }
                    }
                    _ => {}
                }
            }
            if !touched {
                continue;
            }
            self.intents += 1;
            if canceled {
                self.intents_canceled += 1;
            } else if on_player {
                self.intents_on_players += 1;
            } else if on_enemy {
                self.intents_turned += 1;
            } else {
                self.intents_empty += 1;
            }
        }
    }
}

fn is_gunner_weapon(battle: &BattleState, weapon: WeaponId) -> bool {
    battle
        .units()
        .find(|unit| unit.archetype == UnitArchetype::Gunner)
        .is_some_and(|gunner| gunner.weapons.contains(&weapon))
}

/// Novices never weigh a reaction against the telegraphs; they rotate
/// through the three the menu offers, the way a first-time player tries each.
fn novice_reaction(unit: &UnitState, round: u16) -> Reaction {
    [Reaction::Guard, Reaction::Counter, Reaction::Evade]
        [(usize::from(unit.id.0) + usize::from(round)) % 3]
}

fn execute(battle: &mut BattleState, policy: Policy, plan: &Plan, stats: &mut BattleStats) {
    let id = plan.unit;
    if battle.begin_activation(id).is_err() {
        stats.bot_errors += 1;
        return;
    }
    if plan.overdrive && battle.use_overdrive().is_err() {
        stats.bot_errors += 1;
    }
    let fire = |battle: &mut BattleState, stats: &mut BattleStats| {
        let Some((weapon, center)) = plan.attack else {
            return;
        };
        if battle
            .unit(id)
            .is_none_or(|unit| unit.is_knocked_out() || unit.position != plan.attack_from)
        {
            return;
        }
        if plan.focus && battle.use_focus().is_err() {
            stats.bot_errors += 1;
        }
        match battle.attack(id, weapon, center) {
            Ok(events) => {
                let before = stats.enemy_damage;
                stats.record_damage(battle, &events);
                if plan.focus {
                    stats.focus_damage += stats.enemy_damage - before;
                }
            }
            Err(_) => stats.bot_errors += 1,
        }
    };
    // Returns whether the unit ended up where the plan wanted it.
    let walk = |battle: &mut BattleState, to: GridPos, stats: &mut BattleStats| {
        let Some(unit) = battle.unit(id) else {
            return true;
        };
        if unit.is_knocked_out() || unit.position == to {
            return true;
        }
        match battle.move_unit(id, to) {
            Ok(events) => {
                stats.record_damage(battle, &events);
                true
            }
            Err(_) => false,
        }
    };
    if plan.attack_first && plan.attack.is_some() {
        fire(battle, stats);
        // The attack's push, blast or knockout can block the planned cell;
        // staying put is the legal fallback there, not a bot error.
        walk(battle, plan.destination, stats);
    } else {
        // Nothing has changed since planning, so a refused move is a bot
        // error.
        let target = if plan.attack.is_some() {
            plan.attack_from
        } else {
            plan.destination
        };
        if !walk(battle, target, stats) {
            stats.bot_errors += 1;
        }
        fire(battle, stats);
    }
    if battle.phase() != BattlePhase::Player || battle.active_unit() != Some(id) {
        return;
    }
    if battle
        .unit(id)
        .is_some_and(|unit| unit.archetype == UnitArchetype::Vanguard)
        && let Some(ally) = aegis_target(battle, policy, id)
        && battle.use_aegis(ally).is_err()
    {
        stats.bot_errors += 1;
    }
    let reaction = if policy.reads_telegraphs() {
        let refreshed = Situation::new(battle, policy);
        let unit = battle.unit(id).expect("active unit exists");
        refreshed
            .position_value(unit, unit.position, false, &Kills::new())
            .1
    } else {
        plan.reaction
    };
    if battle.choose_reaction(id, reaction).is_err() || battle.finish_activation(id).is_err() {
        stats.bot_errors += 1;
    }
}

fn play_player_phase(battle: &mut BattleState, policy: Policy, stats: &mut BattleStats) {
    loop {
        if battle.phase() != BattlePhase::Player {
            return;
        }
        let pending: Vec<UnitId> = living(battle, Faction::Player)
            .iter()
            .filter(|unit| !unit.activation.finished)
            .map(|unit| unit.id)
            .collect();
        if pending.is_empty() {
            return;
        }
        let plan = if policy == Policy::Novice {
            // Novices click through the roster in order.
            best_plan(battle, policy, pending[0])
        } else {
            pending
                .iter()
                .map(|id| {
                    let plan = best_plan(battle, policy, *id);
                    let gain = plan.score - stay_score(battle, policy, *id);
                    (gain, plan)
                })
                .max_by(|left, right| left.0.total_cmp(&right.0))
                .map(|(_, plan)| plan)
                .expect("pending is non-empty")
        };
        execute(battle, policy, &plan, stats);
    }
}

fn play_battle(mut battle: BattleState, policy: Policy) -> BattleStats {
    let mut stats = BattleStats {
        hp_max: living(&battle, Faction::Player)
            .iter()
            .map(|unit| i32::from(unit.stats.max_hp))
            .sum(),
        ..BattleStats::default()
    };
    let opening = battle.begin_round().expect("missions open in planning");
    stats.record_damage(&battle, &opening);
    while battle.result().is_none() && battle.round() <= ROUND_CAP {
        play_player_phase(&mut battle, policy, &mut stats);
        if battle.result().is_some() {
            break;
        }
        let attackers: Vec<_> = battle
            .intents()
            .iter()
            .map(|intent| intent.attacker)
            .collect();
        match battle.resolve_enemy_phase() {
            Ok(events) => {
                stats.record_damage(&battle, &events);
                stats.record_resolution(&battle, &attackers, &events);
            }
            Err(_) => {
                stats.bot_errors += 1;
                break;
            }
        }
    }
    let result = battle.result();
    stats.victory = result.is_some_and(|result| result.victory);
    stats.bonus = result.is_some_and(|result| result.victory && result.optional_complete);
    stats.rounds = battle.round();
    let players: Vec<_> = battle
        .units()
        .filter(|unit| unit.faction == Faction::Player)
        .collect();
    stats.hp_left = players.iter().map(|unit| i32::from(unit.hp)).sum();
    stats.knockouts = players.iter().filter(|unit| unit.is_knocked_out()).count() as u32;
    stats
}

// ---------------------------------------------------------------------------
// Campaign runs.

/// Cheapest-first purchases in a fixed priority order, the way a player
/// spreads early credits across the squad.
const PURCHASE_ORDER: [(PlayerMech, UpgradeTrack); 12] = [
    (PlayerMech::Gunner, UpgradeTrack::Hp),
    (PlayerMech::Vanguard, UpgradeTrack::Weapon),
    (PlayerMech::Interceptor, UpgradeTrack::Weapon),
    (PlayerMech::Gunner, UpgradeTrack::Weapon),
    (PlayerMech::Vanguard, UpgradeTrack::Armor),
    (PlayerMech::Interceptor, UpgradeTrack::Hp),
    (PlayerMech::Gunner, UpgradeTrack::Armor),
    (PlayerMech::Vanguard, UpgradeTrack::Hp),
    (PlayerMech::Interceptor, UpgradeTrack::Mobility),
    (PlayerMech::Interceptor, UpgradeTrack::Armor),
    (PlayerMech::Gunner, UpgradeTrack::Mobility),
    (PlayerMech::Vanguard, UpgradeTrack::Mobility),
];

fn shop(state: &mut CampaignState) {
    loop {
        let cheapest = PURCHASE_ORDER
            .iter()
            .filter_map(|(mech, track)| {
                let level = state.upgrades.levels(*mech).level(*track);
                (level < 3).then(|| (UPGRADE_COSTS[usize::from(level)], *mech, *track))
            })
            .min_by_key(|(cost, ..)| *cost);
        let Some((cost, mech, track)) = cheapest else {
            return;
        };
        if cost > state.credits {
            return;
        }
        state
            .purchase_upgrade(mech, track)
            .expect("affordable, unmaxed purchase");
    }
}

fn splitmix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[derive(Clone, Debug, Default)]
struct MissionTally {
    runs: u32,
    attempts: u32,
    first_try_wins: u32,
    clears: u32,
    stuck: u32,
    round_one_wins: u32,
    rounds: u32,
    bonus: u32,
    hp_left: i64,
    hp_max: i64,
    knockouts: u32,
    intents: u32,
    intents_on_players: u32,
    intents_turned: u32,
    intents_empty: u32,
    intents_canceled: u32,
    enemy_damage: i64,
    enemy_damage_by_enemy_fire: i64,
    enemy_damage_by_environment: i64,
    focus_damage: i64,
    gunner_damage: i64,
    credits_before: i64,
    bot_errors: u32,
}

impl MissionTally {
    fn add_attempt(&mut self, stats: &BattleStats) {
        self.attempts += 1;
        self.knockouts += stats.knockouts;
        self.intents += stats.intents;
        self.intents_on_players += stats.intents_on_players;
        self.intents_turned += stats.intents_turned;
        self.intents_empty += stats.intents_empty;
        self.intents_canceled += stats.intents_canceled;
        self.enemy_damage += i64::from(stats.enemy_damage);
        self.enemy_damage_by_enemy_fire += i64::from(stats.enemy_damage_by_enemy_fire);
        self.enemy_damage_by_environment += i64::from(stats.enemy_damage_by_environment);
        self.focus_damage += i64::from(stats.focus_damage);
        self.gunner_damage += i64::from(stats.gunner_damage);
        self.bot_errors += stats.bot_errors;
    }

    fn add_clear(&mut self, stats: &BattleStats) {
        self.clears += 1;
        self.rounds += u32::from(stats.rounds);
        if stats.rounds == 1 {
            self.round_one_wins += 1;
        }
        if stats.bonus {
            self.bonus += 1;
        }
        self.hp_left += i64::from(stats.hp_left);
        self.hp_max += i64::from(stats.hp_max);
    }
}

fn definition(id: MissionId) -> &'static MissionDefinition {
    mission_definition(id).expect("authored mission")
}

/// One full campaign: retry each mission until it is won, then shop.
fn play_campaign(policy: Policy, run: u64, tallies: &mut [MissionTally]) {
    let mut state = CampaignState::new_game();
    for (index, mission) in MISSIONS.iter().enumerate() {
        let definition = definition(*mission);
        let tally = &mut tallies[index];
        tally.runs += 1;
        tally.credits_before += i64::from(state.credits);
        let mut won = None;
        for attempt in 0..ATTEMPT_CAP {
            let seed = splitmix(run * 10_007 + index as u64 * 101 + u64::from(attempt));
            let stats = play_battle((definition.build)(seed, &state.upgrades), policy);
            tally.add_attempt(&stats);
            if stats.victory {
                if attempt == 0 {
                    tally.first_try_wins += 1;
                }
                won = Some(stats);
                break;
            }
        }
        let Some(stats) = won else {
            tally.stuck += 1;
            return;
        };
        tally.add_clear(&stats);
        state
            .complete_mission(
                definition,
                scorpius::domain::model::MissionResult {
                    victory: true,
                    optional_complete: stats.bonus,
                    rounds: stats.rounds,
                },
            )
            .expect("victory advances the campaign");
        shop(&mut state);
    }
}

fn simulate(runs: u64) -> BTreeMap<Policy, Vec<MissionTally>> {
    Policy::ALL
        .into_iter()
        .map(|policy| {
            let mut tallies = vec![MissionTally::default(); MISSIONS.len()];
            for run in 0..runs {
                play_campaign(policy, run, &mut tallies);
            }
            (policy, tallies)
        })
        .collect()
}

fn percent(part: impl Into<f64>, whole: impl Into<f64>) -> String {
    let whole = whole.into();
    if whole == 0.0 {
        "—".to_owned()
    } else {
        format!("{:.0}%", 100.0 * part.into() / whole)
    }
}

fn mean(total: impl Into<f64>, count: impl Into<f64>) -> String {
    let count = count.into();
    if count == 0.0 {
        "—".to_owned()
    } else {
        format!("{:.1}", total.into() / count)
    }
}

fn report(results: &BTreeMap<Policy, Vec<MissionTally>>) -> String {
    let mut out = String::new();
    for (policy, tallies) in results {
        writeln!(out, "\n### {}\n", policy.name()).unwrap();
        writeln!(
            out,
            "| Mission | First-try win | Tries/clear | Stuck | Rounds | R1 wins | Bonus | HP left | KOs/try | Intents on squad | Turned on enemies | Empty | Canceled | Enemy dmg from enemy fire | from environment | Focus share | Gunner share | Credits in |"
        )
        .unwrap();
        writeln!(
            out,
            "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
        )
        .unwrap();
        for (mission, tally) in MISSIONS.iter().zip(tallies) {
            writeln!(
                out,
                "| {mission} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                percent(tally.first_try_wins, tally.runs),
                mean(tally.attempts, tally.clears),
                tally.stuck,
                mean(tally.rounds, tally.clears),
                percent(tally.round_one_wins, tally.clears),
                percent(tally.bonus, tally.clears),
                percent(tally.hp_left as f64, tally.hp_max as f64),
                mean(tally.knockouts, tally.attempts),
                percent(tally.intents_on_players, tally.intents),
                percent(tally.intents_turned, tally.intents),
                percent(tally.intents_empty, tally.intents),
                percent(tally.intents_canceled, tally.intents),
                percent(tally.enemy_damage_by_enemy_fire as f64, tally.enemy_damage as f64),
                percent(tally.enemy_damage_by_environment as f64, tally.enemy_damage as f64),
                percent(tally.focus_damage as f64, tally.enemy_damage as f64),
                percent(tally.gunner_damage as f64, tally.enemy_damage as f64),
                mean(tally.credits_before as f64, tally.runs),
            )
            .unwrap();
        }
    }
    out
}

/// Campaigns per policy when `SCORPIUS_BALANCE_RUNS` is unset: the sample the
/// tables in `docs/validation/act-1-balance.md` come from.
const DEFAULT_RUNS: u64 = 200;
/// Missions whose enemies commit real damage. Missions 3 and 4 are objective
/// puzzles with little incoming fire, so their lesson is not survival.
const TELEGRAPH_LESSONS: [MissionId; 5] = [
    MissionId::One,
    MissionId::Two,
    MissionId::Five,
    MissionId::Six,
    MissionId::Seven,
];

fn ratio(part: impl Into<f64>, whole: impl Into<f64>) -> f64 {
    let whole = whole.into();
    if whole == 0.0 {
        0.0
    } else {
        part.into() / whole
    }
}

impl MissionTally {
    fn first_try_rate(&self) -> f64 {
        ratio(self.first_try_wins, self.runs)
    }

    fn tries_per_clear(&self) -> f64 {
        ratio(self.attempts, self.clears)
    }

    fn round_one_rate(&self) -> f64 {
        ratio(self.round_one_wins, self.clears)
    }

    fn bonus_rate(&self) -> f64 {
        ratio(self.bonus, self.clears)
    }

    fn hp_left_rate(&self) -> f64 {
        ratio(self.hp_left as f64, self.hp_max as f64)
    }
}

/// The Act 1 rubric from `docs/validation/act-1-balance.md`: an introduction
/// anyone clears, where reading the telegraphs pays and each mission's
/// mechanic plays out before it ends.
fn assert_act_one_curve(results: &BTreeMap<Policy, Vec<MissionTally>>) {
    for (policy, tallies) in results {
        for (mission, tally) in MISSIONS.iter().zip(tallies) {
            let who = format!("{} on Mission {mission}", policy.name());
            assert!(
                tally.round_one_rate() <= 0.25,
                "{who}: too many wins before the first enemy round"
            );
            if *policy != Policy::Novice {
                assert!(
                    tally.first_try_rate() >= 0.95,
                    "{who}: a telegraph reader should almost never lose"
                );
            }
        }
    }

    let novice = &results[&Policy::Novice];
    let reader = &results[&Policy::Reader];
    let tactician = &results[&Policy::Tactician];
    for (index, mission) in MISSIONS.iter().enumerate() {
        let (novice, reader) = (&novice[index], &reader[index]);
        assert!(
            novice.first_try_rate() >= 0.85 && novice.tries_per_clear() <= 1.5,
            "Novice on Mission {mission}: Act 1 must stay clearable by anyone"
        );
        if TELEGRAPH_LESSONS.contains(mission) {
            assert!(
                reader.hp_left_rate() - novice.hp_left_rate() >= 0.10,
                "Mission {mission}: ignoring the telegraphs must cost HP"
            );
        }
    }

    let finale = novice.last().expect("seven missions").hp_left_rate();
    assert!(
        novice.iter().all(|tally| tally.hp_left_rate() >= finale),
        "the final boss must be the Novice's hardest fight"
    );

    let six = MISSIONS
        .iter()
        .position(|mission| *mission == MissionId::Six)
        .expect("Mission 6 is authored");
    assert!(
        tactician[six].bonus_rate() - reader[six].bonus_rate() >= 0.15,
        "turning the Dreadnought's fire on its escorts must pay off"
    );
}

#[test]
fn act_one_balance_report() {
    let runs = std::env::var("SCORPIUS_BALANCE_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_RUNS);
    let results = simulate(runs);
    println!("{}", report(&results));

    for tallies in results.values() {
        for tally in tallies {
            assert_eq!(tally.bot_errors, 0, "bots only issue legal commands");
            assert_eq!(tally.stuck, 0, "no campaign run gets stuck");
        }
    }
    // Small samples are for quick looks; the rubric needs the full one.
    if runs >= DEFAULT_RUNS {
        assert_act_one_curve(&results);
    }
}

fn as_percent(hit: f64) -> u8 {
    (hit * 100.0).round() as u8
}

/// The bots score hypothetical cells, so they mirror the combat math instead
/// of calling `preview_attack`. Pin the mirror to the domain previews on every
/// mission's opening, Focus included, so a rules change cannot leave the bots
/// scoring stale odds.
#[test]
fn bot_strike_math_matches_the_domain_previews() {
    let mut focused_shots = 0;
    for mission in MISSIONS {
        let mut opening = (definition(mission).build)(7, &CampaignState::new_game().upgrades);
        opening.begin_round().expect("missions open in planning");

        for intent in opening.intents() {
            let (Some(preview), Some(defender)) = (
                intent.intended_preview.as_ref(),
                intent.intended_occupant.and_then(|id| opening.unit(id)),
            ) else {
                continue;
            };
            let strike = enemy_strike(&intent.profile, defender, defender.reaction, false);
            assert_eq!(
                (as_percent(strike.hit), strike.normal, strike.critical),
                (
                    preview.hit_chance,
                    preview.normal_damage,
                    preview.critical_damage
                ),
                "Mission {mission}: enemy intent mirror drifted from its preview"
            );
        }

        let mut compared = 0;
        for attacker in living(&opening, Faction::Player) {
            let mut battle = opening.clone();
            battle
                .begin_activation(attacker.id)
                .expect("opening units can activate");
            let focused = (attacker.archetype == UnitArchetype::Gunner).then(|| {
                let mut focused = battle.clone();
                focused.use_focus().expect("an active Gunner can Focus");
                focused
            });
            for weapon in &attacker.weapons {
                let spec = battle.weapon(*weapon).expect("owned weapon");
                for enemy in living(&battle, Faction::Enemy) {
                    if !reaches(spec, attacker.position, enemy.position)
                        || attacker.en < spec.en_cost
                    {
                        continue;
                    }
                    let preview = battle
                        .preview_attack(attacker.id, *weapon, enemy.position)
                        .expect("a reachable, affordable target previews");
                    let strike = player_strike(attacker, spec, enemy, false);
                    assert_eq!(
                        (as_percent(strike.hit), strike.normal, strike.critical),
                        (
                            preview.hit_chance,
                            preview.normal_damage,
                            preview.critical_damage
                        ),
                        "Mission {mission}: {} mirror drifted from its preview",
                        spec.name
                    );
                    compared += 1;

                    let Some(focused) = &focused else {
                        continue;
                    };
                    let preview = focused
                        .preview_attack(attacker.id, *weapon, enemy.position)
                        .expect("a focused shot previews");
                    let strike = player_strike(attacker, spec, enemy, true);
                    assert_eq!(
                        (as_percent(strike.hit), strike.normal, strike.critical),
                        (
                            preview.hit_chance,
                            preview.normal_damage,
                            preview.critical_damage
                        ),
                        "Mission {mission}: focused {} mirror drifted from its preview",
                        spec.name
                    );
                    focused_shots += 1;
                }
            }
        }
        assert!(
            compared > 0,
            "Mission {mission}: no opening shot to compare"
        );
    }
    assert!(focused_shots > 0, "no focused opening shot to compare");
}

/// Reactions and Aegis only change enemy fire as it resolves; the committed
/// intent previews never include them. Pin those branches of `enemy_strike` to
/// resolved fire instead: every roll must land on the side of the mirrored hit
/// chance the domain chose, and every hit must deal the mirrored damage.
#[test]
fn bot_reaction_math_matches_resolved_enemy_fire() {
    const SEEDS: u64 = 48;
    // Finishing an activation requires a reaction, and Counter leaves
    // incoming fire alone, so Counter with Aegis isolates the shield.
    const CASES: [(Reaction, bool); 5] = [
        (Reaction::Guard, false),
        (Reaction::Evade, false),
        (Reaction::Counter, false),
        (Reaction::Counter, true),
        (Reaction::Guard, true),
    ];
    let upgrades = CampaignState::new_game().upgrades;
    for (reaction, aegis) in CASES {
        let case = format!("{reaction:?} with Aegis {aegis}");
        let (mut damage_compared, mut evaded, mut shielded) = (0, 0, 0);
        for seed in 0..SEEDS {
            for mission in MISSIONS {
                let mut battle = (definition(mission).build)(seed, &upgrades);
                battle.begin_round().expect("missions open in planning");
                ready_reactions(&mut battle, reaction, aegis);
                let before = battle.clone();
                let shield = before.pilot_skills().aegis_target;
                let profiles: BTreeMap<_, _> = before
                    .intents()
                    .iter()
                    .map(|intent| (intent.attacker, &intent.profile))
                    .collect();
                let mut hp: BTreeMap<_, _> =
                    before.units().map(|unit| (unit.id, unit.hp)).collect();
                let events = battle
                    .resolve_enemy_phase()
                    .expect("a ready round resolves");

                for (index, event) in events.iter().enumerate() {
                    if let BattleEvent::DamageApplied {
                        target,
                        remaining_hp,
                        ..
                    } = event
                    {
                        hp.insert(*target, *remaining_hp);
                    }
                    let BattleEvent::AttackRolled {
                        attacker,
                        target,
                        roll,
                        hit,
                        critical,
                        ..
                    } = event
                    else {
                        continue;
                    };
                    // Player counters roll too; only committed enemy fire is mirrored.
                    let Some(profile) = profiles.get(attacker) else {
                        continue;
                    };
                    let defender = before.unit(*target).expect("a rolled target exists");
                    let shielded_target = shield == Some(*target);
                    let strike =
                        enemy_strike(profile, defender, defender.reaction, shielded_target);
                    assert_eq!(
                        *hit,
                        *roll <= as_percent(strike.hit),
                        "Mission {mission}, seed {seed}, {case}: hit-chance mirror drifted"
                    );
                    if !*hit {
                        let plain = enemy_strike(profile, defender, None, false);
                        if defender.reaction == Some(Reaction::Evade)
                            && *roll <= as_percent(plain.hit)
                        {
                            evaded += 1;
                        }
                        continue;
                    }

                    let damage = if *critical {
                        strike.critical
                    } else {
                        strike.normal
                    };
                    let applied = match events.get(index + 1) {
                        Some(BattleEvent::DamageApplied {
                            target: hurt,
                            amount,
                            source: DamageSource::EnemyWeapon(from, _),
                            ..
                        }) if hurt == target && from == attacker => *amount,
                        _ => 0,
                    };
                    assert_eq!(
                        applied,
                        damage.min(hp[target]),
                        "Mission {mission}, seed {seed}, {case}: damage mirror drifted"
                    );
                    if damage < hp[target] {
                        damage_compared += 1;
                        if shielded_target {
                            shielded += 1;
                        }
                    }
                }
            }
        }
        assert!(damage_compared > 0, "{case}: no enemy hit to compare");
        if reaction == Reaction::Evade {
            assert!(
                evaded > 0,
                "{case}: no roll that only Evade turned into a miss"
            );
        }
        if aegis {
            assert!(
                shielded > 0,
                "{case}: no hit on the Aegis target to compare"
            );
        }
    }
}

/// Activate every living player, give each `reaction`, and with `aegis` walk
/// the Vanguard next to an ally in committed fire and shield it. Nobody else
/// moves, so the committed footprints still land on their occupants.
fn ready_reactions(battle: &mut BattleState, reaction: Reaction, aegis: bool) {
    let players: Vec<_> = living(battle, Faction::Player)
        .iter()
        .map(|unit| (unit.id, unit.archetype, unit.position))
        .collect();
    for &(id, archetype, _) in &players {
        battle
            .begin_activation(id)
            .expect("opening units can activate");
        battle
            .choose_reaction(id, reaction)
            .expect("an active unit picks a reaction");
        if aegis && archetype == UnitArchetype::Vanguard {
            let cells = battle.reachable_cells(id).expect("the Vanguard is active");
            let in_fire = |at: GridPos| {
                battle
                    .intents()
                    .iter()
                    .any(|intent| intent.footprint.contains(&at))
            };
            let post = players
                .iter()
                .filter(|&&(ally, _, at)| ally != id && in_fire(at))
                .find_map(|&(ally, _, at)| {
                    cells
                        .iter()
                        .find(|cell| cell.manhattan(at) == 1)
                        .map(|&cell| (cell, ally))
                });
            if let Some((cell, ally)) = post {
                battle
                    .move_unit(id, cell)
                    .expect("a reachable cell accepts the Vanguard");
                battle
                    .use_aegis(ally)
                    .expect("an adjacent ally can be shielded");
            }
        }
        battle
            .finish_activation(id)
            .expect("an active unit can finish");
    }
}
