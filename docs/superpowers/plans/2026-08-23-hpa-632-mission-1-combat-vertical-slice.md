# HPA-632 Mission 1 Combat Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and validate one retained, playable Mission 1 that proves Scorpius's Bevy 0.19 presentation seam and complete committed-intent combat loop.

**Architecture:** One Rust 2024 application crate owns a plain-Rust, phase-oriented `BattleState`; narrow Bevy systems project that state into a 3D orthographic board, mesh picking, checked-in glTF scenes, and native UI. Domain transitions validate and mutate rules synchronously, then return ordered `BattleEvent` values for presentation.

**Tech Stack:** Rust 2024, Bevy `0.19`, built-in Bevy mesh picking, native Bevy UI, glTF 2.0, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-23-hpa-632-mission-1-combat-vertical-slice-design.md`

## Global Constraints

- Declare `bevy = "0.19"`; commit `Cargo.lock`; do not track Bevy `main` or add a physics engine.
- Use one application crate, not a Cargo workspace, engine crate, editor project, or plugin suite.
- Keep canonical combat state and rules in plain Rust; Bevy entities are visual/input adapters through stable IDs.
- Use an orthographic `Camera3d`, checked-in glTF battlefield art, and native screen-space Bevy UI only.
- Keep Mission 1 data concrete and typed in Rust; add no RON/JSON authoring, scripting, behavior tree, generic status/ability framework, backend, or persistence.
- Implement the Bevy viability checkpoint in the same branch before the remaining combat slice.
- Use pure `cargo test` for domain behavior and only a few renderer-free Bevy `App` tests for scheduling/presentation handoff.
- Preserve the exact HPA-632 scope and defer campaign/save/story/upgrades/pilot skills to HPA-635.
- One ticket equals one PR; use small commits inside that PR.

## File Map

| Path | Responsibility |
| --- | --- |
| `Cargo.toml` | One Rust 2024 application crate and Bevy 0.19 dependency |
| `src/lib.rs` | Public module surface and app runner used by `main` |
| `src/main.rs` | Desktop entry point only |
| `src/app.rs` | Default plugins, top-level loading/battle/result state, plugin composition |
| `src/domain/model.rs` | Stable IDs and typed unit/weapon/intent/reaction/event/error values |
| `src/domain/board.rs` | Coordinates, terrain/props, occupancy queries, path/range helpers |
| `src/domain/rng.rs` | Small seedable hit/critical PRNG |
| `src/domain/battle.rs` | Canonical state, phases, activation, objectives, terminal/restart logic |
| `src/domain/combat.rs` | Attack preview, EN commitment, damage, knockout, counter resolution |
| `src/domain/environment.rs` | Push, collision, hazard, explosive exactly-once resolution |
| `src/domain/enemy.rs` | Deterministic Mission 1 positioning, commitment, locked resolution |
| `src/mission/mission_one.rs` | 9×9 layout, six unit instances, twelve weapons, objectives, opening plan |
| `src/presentation/assets.rs` | glTF handles and load/error gate |
| `src/presentation/battlefield.rs` | Camera, board cells, glTF visuals, highlights and stable-ID markers |
| `src/presentation/interaction.rs` | Pointer/keyboard interaction state converted to domain commands |
| `src/presentation/sync.rs` | Canonical state/events projected to transforms, visibility and effects |
| `src/presentation/ui.rs` | Native objective, threat, unit, command, preview, status and result UI |
| `assets/models/mission_one.gltf` | Original low-poly scene roots for units, terrain, prop, hazard and impact |
| `tests/presentation_app.rs` | Minimal-plugin scheduling/handoff tests without renderer/window |
| `.github/workflows/ci.yml` | Format, Clippy, test and release-build gates |
| `docs/validation/hpa-632.md` | Viability and final manual-play evidence |
| `README.md` | Local run, controls, objective and verification instructions |

---

### Task 1: Bootstrap the crate and the first renderer-free movement rule

**Files:**
- Create: `.gitignore`
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/domain/mod.rs`
- Create: `src/domain/model.rs`
- Create: `src/domain/board.rs`
- Create: `src/domain/battle.rs`
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: `GridPos`, `UnitId`, `UnitState`, `BattleState::viability_fixture()`, `BattleState::move_unit(UnitId, GridPos) -> Result<Vec<BattleEvent>, BattleError>`.
- Consumes: nothing; this is the repository foundation.

- [x] **Step 1: Write the failing coordinate and movement tests**

Add these tests at the bottoms of `src/domain/board.rs` and `src/domain/battle.rs`:

```rust
// board.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manhattan_distance_and_neighbors_are_orthogonal() {
        let origin = GridPos::new(2, 3);
        assert_eq!(origin.manhattan(GridPos::new(5, 1)), 5);
        assert_eq!(
            origin.orthogonal_neighbors(5, 5),
            vec![
                GridPos::new(2, 2),
                GridPos::new(1, 3),
                GridPos::new(3, 3),
                GridPos::new(2, 4),
            ]
        );
    }
}

// battle.rs
#[cfg(test)]
mod tests {
    use super::*;

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
}
```

- [x] **Step 2: Run the focused tests and confirm the red state**

Run: `cargo test domain::`

Expected: compilation fails because the crate and referenced types do not exist yet.

- [x] **Step 3: Add the minimal Rust 2024 crate**

Create `Cargo.toml`:

```toml
[package]
name = "scorpius"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
bevy = "0.19"

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

Create `.gitignore` containing `/target` and `.DS_Store`. Create `src/main.rs` with an empty `fn main() {}` until Task 2 adds the real runner. Export `pub mod domain;` from `src/lib.rs`, and export `battle`, `board`, and `model` from `src/domain/mod.rs`.

- [x] **Step 4: Implement the viability domain types and transition**

Use these minimal definitions, keeping them free of Bevy types:

```rust
// model.rs
use crate::domain::board::GridPos;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitId(pub u16);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitState {
    pub id: UnitId,
    pub position: GridPos,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleEvent {
    UnitMoved { unit: UnitId, from: GridPos, to: GridPos },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleError {
    UnknownUnit(UnitId),
    OutOfBounds(GridPos),
    DestinationOccupied(GridPos),
    NotOrthogonallyAdjacent { from: GridPos, to: GridPos },
}

// board.rs
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GridPos { pub x: u8, pub y: u8 }

impl GridPos {
    pub const fn new(x: u8, y: u8) -> Self { Self { x, y } }
    pub fn manhattan(self, other: Self) -> u8 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }
    pub fn orthogonal_neighbors(self, width: u8, height: u8) -> Vec<Self> {
        let mut out = Vec::with_capacity(4);
        if self.y > 0 { out.push(Self::new(self.x, self.y - 1)); }
        if self.x > 0 { out.push(Self::new(self.x - 1, self.y)); }
        if self.x + 1 < width { out.push(Self::new(self.x + 1, self.y)); }
        if self.y + 1 < height { out.push(Self::new(self.x, self.y + 1)); }
        out
    }
}

// battle.rs
use std::collections::BTreeMap;
use super::{board::GridPos, model::{BattleError, BattleEvent, UnitId, UnitState}};

pub struct BattleState {
    width: u8,
    height: u8,
    units: BTreeMap<UnitId, UnitState>,
}

impl BattleState {
    pub fn viability_fixture() -> Self {
        Self {
            width: 3,
            height: 3,
            units: [(UnitId(1), UnitState { id: UnitId(1), position: GridPos::new(1, 1) })]
                .into_iter().collect(),
        }
    }

    pub fn unit(&self, id: UnitId) -> Option<&UnitState> { self.units.get(&id) }

    pub fn move_unit(&mut self, id: UnitId, to: GridPos) -> Result<Vec<BattleEvent>, BattleError> {
        if to.x >= self.width || to.y >= self.height { return Err(BattleError::OutOfBounds(to)); }
        let from = self.units.get(&id).ok_or(BattleError::UnknownUnit(id))?.position;
        if from.manhattan(to) != 1 {
            return Err(BattleError::NotOrthogonallyAdjacent { from, to });
        }
        if self.units.values().any(|unit| unit.position == to) {
            return Err(BattleError::DestinationOccupied(to));
        }
        self.units.get_mut(&id).expect("unit checked above").position = to;
        Ok(vec![BattleEvent::UnitMoved { unit: id, from, to }])
    }
}
```

- [x] **Step 5: Add the initial CI workflow**

Create `.github/workflows/ci.yml` with one Ubuntu job using `actions/checkout@v4`, `rustup update stable`, `rustup component add rustfmt clippy`, then the four required commands in order:

```yaml
name: CI
on:
  push:
  pull_request:

jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: rustup update stable
      - run: rustup default stable
      - run: rustup component add rustfmt clippy
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test --all-targets
      - run: cargo build --release
```

- [x] **Step 6: Verify the green state and lock dependencies**

Run: `cargo fmt --check`

Run: `cargo test domain::`

Expected: both tests pass and `Cargo.lock` is created with Bevy 0.19.x.

- [x] **Step 7: Commit the foundation**

```bash
git add .gitignore Cargo.toml Cargo.lock src .github/workflows/ci.yml
git commit -m "chore: bootstrap Scorpius Bevy application"
```

---

### Task 2: Pass the in-ticket Bevy viability checkpoint

**Files:**
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Create: `src/app.rs`
- Create: `src/presentation/mod.rs`
- Create: `src/presentation/assets.rs`
- Create: `src/presentation/battlefield.rs`
- Create: `src/presentation/interaction.rs`
- Create: `src/presentation/sync.rs`
- Create: `src/presentation/ui.rs`
- Create: `assets/models/mission_one.gltf`
- Create: `tests/presentation_app.rs`
- Create: `docs/validation/hpa-632.md`

**Interfaces:**
- Consumes: `BattleState::viability_fixture`, `BattleState::move_unit`, `UnitId`, `GridPos`, `BattleEvent::UnitMoved`.
- Produces: `ScorpiusPlugin`, `BattleRuntime`, `CellVisual`, `UnitVisual`, `SelectedCell`, `grid_to_world`, `apply_unit_transforms`, and the checked-in glTF scene labels used by later presentation tasks.

- [x] **Step 1: Write the failing renderer-free transform handoff test**

Create `tests/presentation_app.rs`:

```rust
use bevy::prelude::*;
use scorpius::{
    domain::{battle::BattleState, board::GridPos, model::UnitId},
    presentation::{BattleRuntime, UnitVisual, grid_to_world, sync::apply_unit_transforms},
};

#[test]
fn canonical_move_drives_visual_transform_without_renderer() {
    let mut app = App::new();
    app.insert_resource(BattleRuntime(BattleState::viability_fixture()))
        .add_systems(Update, apply_unit_transforms);
    app.world_mut().spawn((
        UnitVisual(UnitId(1)),
        Transform::from_translation(grid_to_world(GridPos::new(1, 1))),
    ));

    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .move_unit(UnitId(1), GridPos::new(1, 2))
        .unwrap();
    app.update();

    let transform = app.world_mut().query::<&Transform>().single(app.world()).unwrap();
    assert_eq!(transform.translation, grid_to_world(GridPos::new(1, 2)));
}
```

- [x] **Step 2: Run the test and confirm missing presentation symbols**

Run: `cargo test --test presentation_app canonical_move_drives_visual_transform_without_renderer`

Expected: FAIL because the presentation module and types do not exist.

- [x] **Step 3: Add the real app/plugin boundary and renderer-free sync**

Implement `src/lib.rs` and `src/main.rs` as:

```rust
// lib.rs
pub mod app;
pub mod domain;
pub mod presentation;

use bevy::prelude::*;

pub fn run() {
    App::new().add_plugins(app::ScorpiusPlugin).run();
}

// main.rs
fn main() { scorpius::run(); }
```

Define presentation markers/resources and transform sync:

```rust
#[derive(Resource)]
pub struct BattleRuntime(pub BattleState);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitVisual(pub UnitId);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVisual(pub GridPos);

#[derive(Resource, Default)]
pub struct SelectedCell(pub Option<GridPos>);

pub fn grid_to_world(pos: GridPos) -> Vec3 {
    Vec3::new(pos.x as f32 - 1.0, 0.2, pos.y as f32 - 1.0)
}

pub fn apply_unit_transforms(
    battle: Res<BattleRuntime>,
    mut visuals: Query<(&UnitVisual, &mut Transform)>,
) {
    for (visual, mut transform) in &mut visuals {
        if let Some(unit) = battle.0.unit(visual.0) {
            transform.translation = grid_to_world(unit.position);
        }
    }
}
```

Keep this system in `presentation/sync.rs` and re-export the stable types from `presentation/mod.rs`.

- [x] **Step 4: Make the renderer-free handoff test pass**

Run: `cargo test --test presentation_app canonical_move_drives_visual_transform_without_renderer`

Expected: PASS without initializing `DefaultPlugins`, Winit, a window, or a renderer.

- [x] **Step 5: Add a valid checked-in glTF with stable scene indices**

Create one glTF 2.0 document with an embedded binary buffer and these scene indices/names:

```text
0 Vanguard
1 Gunner
2 Interceptor
3 Rifleman
4 Striker
5 Artillery
6 BlockingTerrain
7 Explosive
8 Hazard
9 ImpactMarker
```

Compose each scene from reused low-poly cuboid/prism meshes, distinct transforms, and these unlit base colors: Vanguard navy, Gunner ochre, Interceptor cyan, Rifleman gray, Striker crimson, Artillery violet, terrain slate, explosive orange, hazard acid green, impact white. Store the buffer as a `data:application/octet-stream;base64,...` URI so `mission_one.gltf` is the only required model file. Validate it before app integration:

Run: `python3 -m json.tool assets/models/mission_one.gltf >/dev/null`

Expected: exit 0.

- [x] **Step 6: Build the real checkpoint scene**

Implement `ScorpiusPlugin` with `DefaultPlugins`, `MeshPickingPlugin`, a 1280×720 window, and startup systems. Spawn:

- an orthographic `Camera3d` using `ScalingMode::FixedVertical { viewport_height: 13.0 }`, positioned at `(11, 13, 11)` and looking at the board center
- a directional light
- a 3×3 compact viability board of pickable `Cuboid` cell meshes tagged with `CellVisual`
- scene 0 from `mission_one.gltf` through `AssetServer` and `GltfAssetLabel::Scene(0)`, tagged with `UnitVisual(UnitId(1))`
- an absolute native UI panel reading `HPA-632 · Bevy viability` and a selected-cell label

Use Bevy 0.19's opt-in mesh picking pattern:

```rust
app.add_plugins((
    DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Scorpius — Mission 1".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }),
    MeshPickingPlugin,
))
.insert_resource(MeshPickingSettings {
    require_markers: true,
    ..default()
});
```

Attach `Pickable::default()` to cell meshes and an `On<Pointer<Click>>` observer that copies the clicked entity's `CellVisual.0` into `SelectedCell`. A second click on an orthogonally adjacent cell calls the viability `move_unit`; do not derive logical coordinates from transforms.

- [x] **Step 7: Compile and run the real app**

Run: `cargo check --all-targets`

Expected: exit 0 on Bevy 0.19.x.

Run: `cargo run`

Expected manual evidence: the app opens with an orthographic 3D grid, the checked-in glTF mech, a native UI overlay, clickable logical cells, and one unit whose transform follows `BattleState` after an adjacent click.

- [x] **Step 8: Record the viability evidence**

Create `docs/validation/hpa-632.md` with a dated checklist containing the seven HPA-632 viability requirements, the exact commands run, Bevy version from `Cargo.lock`, and observed results. Mark only evidence actually observed as passed.

- [x] **Step 9: Run the checkpoint gates and commit**

Run: `cargo fmt --check`

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Run: `cargo test --all-targets`

Run: `cargo build --release`

Expected: all exit 0.

```bash
git add src assets tests docs/validation/hpa-632.md Cargo.lock
git commit -m "feat: prove Bevy combat presentation viability"
```

Do not begin Task 3 if the real checkpoint exposes an impractical 3D/UI composition or picking boundary; revise the design and checkpoint in this branch first.

---

### Task 3: Replace the viability fixture with typed Mission 1 content

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/board.rs`
- Modify: `src/domain/battle.rs`
- Create: `src/mission/mod.rs`
- Create: `src/mission/mission_one.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `GridPos`, `UnitId`, the viability movement behavior.
- Produces: `WeaponId`, `Faction`, `UnitArchetype`, `UnitStats`, `WeaponShape`, `WeaponSpec`, expanded `UnitState`, `BoardState`, `MissionOneIds`, `mission_one(seed: u64) -> BattleState`, and immutable weapon/unit lookup methods.

- [x] **Step 1: Write failing authored-content tests**

Add to `src/mission/mission_one.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_one_has_the_locked_roster_and_nine_player_weapons() {
        let battle = mission_one(7);
        let players: Vec<_> = battle.units().filter(|u| u.faction == Faction::Player).collect();
        let enemies: Vec<_> = battle.units().filter(|u| u.faction == Faction::Enemy).collect();

        assert_eq!(players.len(), 3);
        assert_eq!(enemies.len(), 4);
        assert_eq!(players.iter().map(|u| u.weapons.len()).sum::<usize>(), 9);
        assert_eq!(battle.board().width(), 9);
        assert_eq!(battle.board().height(), 9);
        assert!(battle.board().is_blocking(GridPos::new(3, 5)));
        assert!(battle.board().is_hazard(GridPos::new(2, 6)));
        assert_eq!(battle.board().explosive_at(GridPos::new(6, 6)).unwrap().hp, 4);
    }

    #[test]
    fn weapon_values_match_the_approved_design() {
        let battle = mission_one(7);
        let rail = battle.weapon(ids::RAIL_RIFLE).unwrap();
        assert_eq!((rail.min_range, rail.max_range), (3, 6));
        assert_eq!(rail.base_damage, 7);
        assert_eq!(rail.hit_modifier, 15);
        assert_eq!(rail.crit_chance, 20);
        assert_eq!(rail.en_cost, 0);
        assert_eq!(rail.shape, WeaponShape::Single);
        assert!(rail.counter_weapon);
    }
}
```

- [x] **Step 2: Run the tests and confirm missing typed content**

Run: `cargo test mission::mission_one::tests`

Expected: FAIL because the mission module and expanded model do not exist.

- [x] **Step 3: Expand the model with explicit authored values**

Use these core types in `model.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WeaponId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Faction { Player, Enemy }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitArchetype { Vanguard, Gunner, Interceptor, Rifleman, Striker, Artillery }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitStats {
    pub max_hp: i16,
    pub armor: i16,
    pub movement: u8,
    pub accuracy: i16,
    pub evasion: i16,
    pub max_en: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponShape { Single, Cross1 }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeaponSpec {
    pub id: WeaponId,
    pub name: &'static str,
    pub min_range: u8,
    pub max_range: u8,
    pub shape: WeaponShape,
    pub base_damage: i16,
    pub hit_modifier: i16,
    pub crit_chance: u8,
    pub en_cost: i16,
    pub push: bool,
    pub counter_weapon: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reaction { Counter, Guard, Evade }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActivationState {
    pub moved: bool,
    pub acted: bool,
    pub finished: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitState {
    pub id: UnitId,
    pub name: &'static str,
    pub archetype: UnitArchetype,
    pub faction: Faction,
    pub stats: UnitStats,
    pub hp: i16,
    pub en: i16,
    pub position: GridPos,
    pub weapons: Vec<WeaponId>,
    pub activation: ActivationState,
    pub reaction: Option<Reaction>,
}

impl UnitState {
    pub fn is_knocked_out(&self) -> bool { self.hp == 0 }
}
```

Represent blocking cells, one hazard, and one `ExplosiveState { position, hp, exploded }` in `BoardState`. Expose read-only iterators/lookups from `BattleState`; do not expose its collections mutably.

- [x] **Step 4: Author the exact Mission 1 constants**

Create stable IDs in `mission_one::ids`: player IDs 1–3, Riflemen 11–12, Striker 13, Artillery 14; player weapon IDs 101–109; enemy weapon IDs 201–203.

Create `mission_one(seed)` with the approved table values and initial positions before first-round movement:

```rust
let units = [
    unit(ids::VANGUARD, "Vanguard", UnitArchetype::Vanguard, Faction::Player,
         stats(20, 3, 3, 78, 5, 7), GridPos::new(4, 7),
         vec![ids::PILE_LANCE, ids::REPULSOR_RAM, ids::ANCHOR_CANNON]),
    unit(ids::GUNNER, "Gunner", UnitArchetype::Gunner, Faction::Player,
         stats(12, 1, 2, 86, 10, 9), GridPos::new(3, 8),
         vec![ids::RAIL_RIFLE, ids::BURST_MISSILE, ids::OVERCHARGE_SHOT]),
    unit(ids::INTERCEPTOR, "Interceptor", UnitArchetype::Interceptor, Faction::Player,
         stats(15, 1, 4, 82, 20, 8), GridPos::new(5, 8),
         vec![ids::ARC_BLADE, ids::PULSE_CARBINE, ids::VECTOR_PULSE]),
    unit(ids::RIFLEMAN_LEFT, "Rifleman L", UnitArchetype::Rifleman, Faction::Enemy,
         stats(9, 1, 2, 72, 5, 0), GridPos::new(2, 3), vec![ids::SERVICE_RIFLE]),
    unit(ids::RIFLEMAN_RIGHT, "Rifleman R", UnitArchetype::Rifleman, Faction::Enemy,
         stats(9, 1, 2, 72, 5, 0), GridPos::new(6, 3), vec![ids::SERVICE_RIFLE]),
    unit(ids::STRIKER, "Striker", UnitArchetype::Striker, Faction::Enemy,
         stats(12, 2, 2, 78, 10, 0), GridPos::new(4, 4), vec![ids::SHOCK_CLAW]),
    unit(ids::ARTILLERY, "Artillery", UnitArchetype::Artillery, Faction::Enemy,
         stats(10, 1, 1, 90, 0, 0), GridPos::new(4, 0), vec![ids::SIEGE_MORTAR]),
];
```

Author the nine player weapons and three enemy weapons verbatim from the spec tables. Use blocking cells `(2,1)`, `(6,1)`, `(1,4)`, `(7,4)`, `(3,5)`, `(5,5)`, hazard `(2,6)`, and explosive `(6,6)` with 4 HP.

- [x] **Step 5: Preserve the viability fixture as a real `BattleState`**

Rebuild `BattleState::viability_fixture()` with the expanded types and one Vanguard on a 3×3 empty board. Keep Task 1 and Task 2 tests passing while `mission_one(seed)` becomes the production constructor.

- [x] **Step 6: Run content and regression tests**

Run: `cargo test mission::mission_one::tests`

Run: `cargo test pure_battle_state_moves_without_bevy`

Run: `cargo test --test presentation_app`

Expected: all pass.

- [x] **Step 7: Commit typed Mission 1 content**

```bash
git add src/domain src/mission src/lib.rs
git commit -m "feat: author Mission 1 combat data"
```

---

### Task 4: Enforce free activation order and Move/Action limits

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/board.rs`
- Modify: `src/domain/battle.rs`

**Interfaces:**
- Consumes: Mission 1 units, stats, board occupancy and reactions.
- Produces: `BattlePhase`, `BattleState::begin_activation`, `reachable_cells`, full `move_unit`, `choose_reaction`, `finish_activation`, and `ready_to_resolve`.

- [ ] **Step 1: Write failing activation tests**

Add to `battle.rs`:

```rust
#[test]
fn player_chooses_free_order_but_each_unit_moves_once() {
    let mut battle = mission_one(7);
    battle.enter_player_phase_for_test();
    battle.begin_activation(ids::INTERCEPTOR).unwrap();
    battle.move_unit(ids::INTERCEPTOR, GridPos::new(5, 7)).unwrap();

    assert_eq!(
        battle.move_unit(ids::INTERCEPTOR, GridPos::new(6, 7)),
        Err(BattleError::MoveAlreadySpent(ids::INTERCEPTOR))
    );
    assert_eq!(
        battle.begin_activation(ids::VANGUARD),
        Err(BattleError::ActivationInProgress(ids::INTERCEPTOR))
    );

    battle.choose_reaction(ids::INTERCEPTOR, Reaction::Evade).unwrap();
    battle.finish_activation(ids::INTERCEPTOR).unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
}

#[test]
fn finishing_skips_unused_move_and_action() {
    let mut battle = mission_one(7);
    battle.enter_player_phase_for_test();
    battle.begin_activation(ids::GUNNER).unwrap();
    battle.choose_reaction(ids::GUNNER, Reaction::Guard).unwrap();
    battle.finish_activation(ids::GUNNER).unwrap();

    let gunner = battle.unit(ids::GUNNER).unwrap();
    assert_eq!(gunner.activation, ActivationState { moved: true, acted: true, finished: true });
}
```

- [ ] **Step 2: Run the focused tests and confirm red state**

Run: `cargo test domain::battle::tests`

Expected: FAIL because phases and activation transitions do not exist.

- [ ] **Step 3: Add phase and activation validation**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattlePhase { EnemyPlanning, Player, EnemyResolution, Victory, Defeat }

pub struct BattleState {
    // existing board/units/weapons/rng fields
    phase: BattlePhase,
    round: u16,
    active_unit: Option<UnitId>,
}
```

`begin_activation` accepts only a living, unfinished player during `Player`, and rejects a second active unit. `choose_reaction` accepts only the active unit. `finish_activation` requires a reaction, sets skipped Move/Action to spent, marks finished, and clears `active_unit`.

- [ ] **Step 4: Replace one-cell viability movement with path-based movement**

Implement `reachable_cells(UnitId) -> Result<BTreeSet<GridPos>, BattleError>` with a breadth-first traversal capped by `unit.stats.movement`. A legal path remains in bounds and cannot pass through blocking terrain, a live explosive, or another living unit. The unit's origin is not returned as a destination.

Production `move_unit` must require the active unit during `Player`, reject `moved == true`, verify the requested destination is reachable, update position once, set `moved = true`, and emit one `UnitMoved` event. Keep a private viability-only single-step transition or configure the viability fixture as an active player so its existing test still exercises the same production rule.

- [ ] **Step 5: Run movement, activation and presentation regressions**

Run: `cargo test domain::battle::tests`

Run: `cargo test --test presentation_app`

Expected: all pass.

- [ ] **Step 6: Commit activation rules**

```bash
git add src/domain
git commit -m "feat: enforce player activation rules"
```

---

### Task 5: Implement previews, seeded attack RNG, EN and knockout

**Files:**
- Create: `src/domain/rng.rs`
- Create: `src/domain/combat.rs`
- Modify: `src/domain/mod.rs`
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/domain/board.rs`

**Interfaces:**
- Consumes: active-unit state, weapon/unit data, board distance and occupancy.
- Produces: `BattleRng`, `AttackPreview`, `DamageSource`, `preview_attack`, `attack`, `apply_damage`, attack/damage/knockout events.

- [ ] **Step 1: Write failing RNG, preview and attack tests**

Add focused tests in `rng.rs` and `combat.rs`:

```rust
#[test]
fn splitmix_rolls_are_stable_for_known_seeds() {
    assert_eq!(BattleRng::seeded(2).roll_percent(), 11);
    assert_eq!(BattleRng::seeded(6).roll_percent(), 93);
    let mut crit_seed = BattleRng::seeded(0);
    assert_eq!((crit_seed.roll_percent(), crit_seed.roll_percent()), (36, 11));
}

#[test]
fn preview_and_resolution_share_inputs_and_charge_en_once() {
    let mut battle = adjacent_vanguard_and_striker(2);
    battle.begin_activation(ids::VANGUARD).unwrap();
    let preview = battle.preview_attack(ids::VANGUARD, ids::REPULSOR_RAM, GridPos::new(4, 6)).unwrap();
    let en_before = battle.unit(ids::VANGUARD).unwrap().en;
    let events = battle.attack(ids::VANGUARD, ids::REPULSOR_RAM, GridPos::new(4, 6)).unwrap();

    assert_eq!(preview.hit_chance, 83);
    assert_eq!(preview.normal_damage, 3);
    assert_eq!(battle.unit(ids::VANGUARD).unwrap().en, en_before - 2);
    assert!(events.iter().any(|event| matches!(event, BattleEvent::AttackRolled { roll: 11, hit: true, .. })));
    assert_eq!(
        battle.attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6)),
        Err(BattleError::ActionAlreadySpent(ids::VANGUARD))
    );
}

#[test]
fn seeded_miss_crit_and_knockout_are_deterministic() {
    let mut miss = adjacent_vanguard_and_striker(6);
    miss.begin_activation(ids::VANGUARD).unwrap();
    miss.attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6)).unwrap();
    assert_eq!(miss.unit(ids::STRIKER).unwrap().hp, 12);

    let mut crit = low_hp_striker_fixture(0);
    crit.begin_activation(ids::VANGUARD).unwrap();
    crit.attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6)).unwrap();
    assert!(crit.unit(ids::STRIKER).unwrap().is_knocked_out());
    assert_eq!(crit.occupant_at(GridPos::new(4, 6)), None);
}

#[test]
fn move_and_action_work_in_either_order_once() {
    let mut action_first = adjacent_vanguard_and_striker(2);
    action_first.begin_activation(ids::VANGUARD).unwrap();
    action_first.attack(ids::VANGUARD, ids::REPULSOR_RAM, GridPos::new(4, 6)).unwrap();
    action_first.move_unit(ids::VANGUARD, GridPos::new(3, 7)).unwrap();

    let mut move_first = adjacent_vanguard_and_striker(2);
    move_first.begin_activation(ids::VANGUARD).unwrap();
    move_first.move_unit(ids::VANGUARD, GridPos::new(4, 8)).unwrap();
    move_first.attack(ids::VANGUARD, ids::ANCHOR_CANNON, GridPos::new(4, 6)).unwrap();

    assert!(action_first.unit(ids::VANGUARD).unwrap().activation.moved);
    assert!(move_first.unit(ids::VANGUARD).unwrap().activation.acted);
}
```

Use test fixture constructors under `#[cfg(test)]`; they must build real `BattleState` values with the production rules and the indicated seed.

- [ ] **Step 2: Run the tests and verify red state**

Run: `cargo test domain::`

Expected: FAIL because RNG, preview and attack functions are absent.

- [ ] **Step 3: Implement a seedable SplitMix64 PRNG**

```rust
#[derive(Clone, Debug)]
pub struct BattleRng { state: u64 }

impl BattleRng {
    pub const fn seeded(seed: u64) -> Self { Self { state: seed } }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn roll_percent(&mut self) -> u8 { (self.next_u64() % 100 + 1) as u8 }
}
```

Create production seeds from `SystemTime::now()` in the app layer and continue passing `u64` into `mission_one`; do not add a replay API.

- [ ] **Step 4: Implement the one preview calculation**

Define:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttackPreview {
    pub attacker: UnitId,
    pub weapon: WeaponId,
    pub target: GridPos,
    pub footprint: Vec<GridPos>,
    pub hit_chance: u8,
    pub normal_damage: i16,
    pub critical_damage: i16,
    pub en_cost: i16,
    pub push_destination: Option<GridPos>,
}
```

Calculate `hit_chance = clamp(accuracy + hit_modifier - evasion, 5, 95)`, `normal_damage = max(1, base_damage - armor)`, and critical damage from `base + floor(base / 2)` before armor. `Cross1` returns the target plus in-bounds neighbors in stable top/left/right/bottom order. Push weapons reject targets not sharing the attacker's row or column.

- [ ] **Step 5: Implement atomic player attacks**

Validate phase, active unit, Action allowance, ownership, weapon ownership, range/shape, target and EN before mutation. Deduct EN and mark Action spent exactly once, then for every unique enemy occupant in the footprint:

1. consume one hit roll
2. on hit, consume one crit roll
3. emit `AttackRolled`
4. apply preview-derived damage and emit `DamageApplied`
5. clamp HP to zero and emit `UnitKnockedOut` once

Skip allied player occupants. Keep push as an emitted `PushRequested` event for Task 6 so this task cannot duplicate environment resolution.

- [ ] **Step 6: Run all combat and activation tests**

Run: `cargo test domain::`

Expected: hit, miss, critical, EN, knockout, Move→Action and Action→Move cases pass.

- [ ] **Step 7: Commit combat resolution**

```bash
git add src/domain
git commit -m "feat: resolve seeded player attacks"
```

---

### Task 6: Resolve Push 1, collision, hazard and explosive damage exactly once

**Files:**
- Create: `src/domain/environment.rs`
- Modify: `src/domain/mod.rs`
- Modify: `src/domain/model.rs`
- Modify: `src/domain/board.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/domain/combat.rs`

**Interfaces:**
- Consumes: successful player attack events, board occupancy, props and `apply_damage`.
- Produces: `resolve_push`, `damage_explosive`, `resolve_explosion`, collision/hazard/explosion events, and Turnabout-relevant `DamageSource` variants.

- [ ] **Step 1: Write failing single-resolution environment tests**

Add to `environment.rs`:

```rust
#[test]
fn push_moves_once_then_hazard_damages_once() {
    let mut battle = hazard_push_fixture();
    let events = battle.resolve_push(ids::INTERCEPTOR, ids::RIFLEMAN_LEFT).unwrap();

    assert_eq!(battle.unit(ids::RIFLEMAN_LEFT).unwrap().position, GridPos::new(2, 6));
    assert_eq!(battle.unit(ids::RIFLEMAN_LEFT).unwrap().hp, 6);
    assert_eq!(events.iter().filter(|e| matches!(e, BattleEvent::HazardTriggered { .. })).count(), 1);
    assert_eq!(events.iter().filter(|e| matches!(e, BattleEvent::DamageApplied { source: DamageSource::Hazard, .. })).count(), 1);
}

#[test]
fn blocked_push_deals_collision_only_to_pushed_unit() {
    let mut battle = collision_fixture();
    battle.resolve_push(ids::VANGUARD, ids::STRIKER).unwrap();
    assert_eq!(battle.unit(ids::STRIKER).unwrap().position, GridPos::new(3, 5));
    assert_eq!(battle.unit(ids::STRIKER).unwrap().hp, 9);
}

#[test]
fn explosive_applies_one_cross_event_and_cannot_repeat() {
    let mut battle = explosive_fixture();
    let first = battle.damage_explosive(GridPos::new(6, 6), 4, DamageSource::PlayerWeapon(ids::RAIL_RIFLE)).unwrap();
    let second = battle.damage_explosive(GridPos::new(6, 6), 4, DamageSource::PlayerWeapon(ids::RAIL_RIFLE)).unwrap();

    assert_eq!(first.iter().filter(|e| matches!(e, BattleEvent::ExplosionTriggered { .. })).count(), 1);
    assert!(second.is_empty());
    assert_eq!(battle.unit(ids::RIFLEMAN_RIGHT).unwrap().hp, 5);
}
```

- [ ] **Step 2: Run tests and verify the red state**

Run: `cargo test domain::environment::tests`

Expected: FAIL because environment transitions do not exist.

- [ ] **Step 3: Implement direct environment damage sources**

Expand `DamageSource` with `Collision`, `Hazard`, `Explosion`, and `EnemyWeapon(UnitId, WeaponId)`. Environment damage bypasses armor, hit and critical checks. Route it through one `apply_direct_damage` helper so HP clamp, knockout and objective observation remain identical.

- [ ] **Step 4: Implement deterministic push direction and collision**

For a same-row or same-column attacker/target pair, calculate the sign of the coordinate delta and one destination cell away from the attacker. If the destination is off-board or blocked by terrain, a live prop or a living unit, retain the target's position and apply 3 collision damage to the pushed unit only. Otherwise move once and emit `UnitPushed`.

- [ ] **Step 5: Implement exactly-once hazard and explosive transitions**

Immediately after a successful push/move transaction ends on `(2, 6)`, emit `HazardTriggered` and apply 3 direct damage once. Do not run hazards from a per-frame Bevy system.

`damage_explosive` returns an empty event list when the prop is already exploded. The first transition reducing its 4 HP to zero marks it exploded before applying 4 direct damage to living units on its own and four orthogonal cells, so nested event processing cannot retrigger it.

Connect successful player attacks to props automatically, and replace `PushRequested` with one synchronous call to `resolve_push` after a successful push-weapon hit.

- [ ] **Step 6: Run focused and full domain tests**

Run: `cargo test domain::environment::tests`

Run: `cargo test domain`

Expected: all environment cases pass with no duplicate damage; earlier attack tests remain green.

- [ ] **Step 7: Commit environment rules**

```bash
git add src/domain
git commit -m "feat: add tactical environment interactions"
```

---

### Task 7: Move enemies deterministically and commit locked intents

**Files:**
- Create: `src/domain/enemy.rs`
- Modify: `src/domain/mod.rs`
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/domain/board.rs`
- Modify: `src/mission/mission_one.rs`

**Interfaces:**
- Consumes: board path/range helpers, enemy unit/weapon data and attack preview calculations.
- Produces: `AttackProfile`, `AttackIntent`, `BattleState::begin_round`, deterministic positioning, `commit_enemy_intents`, `intents()`, and the authored resolution order.

- [ ] **Step 1: Write failing opening-plan and empty-footprint tests**

Add to `enemy.rs`:

```rust
#[test]
fn authored_opening_places_four_locked_threats() {
    let mut battle = mission_one(7);
    let events = battle.begin_round().unwrap();

    assert_eq!(battle.unit(ids::RIFLEMAN_LEFT).unwrap().position, GridPos::new(2, 5));
    assert_eq!(battle.unit(ids::RIFLEMAN_RIGHT).unwrap().position, GridPos::new(6, 5));
    assert_eq!(battle.unit(ids::STRIKER).unwrap().position, GridPos::new(4, 6));
    assert_eq!(battle.unit(ids::ARTILLERY).unwrap().position, GridPos::new(4, 0));
    assert_eq!(battle.intents().len(), 4);

    let mortar = battle.intent_for(ids::ARTILLERY).unwrap();
    assert_eq!(mortar.intended_occupant, Some(ids::VANGUARD));
    assert_eq!(mortar.footprint, vec![
        GridPos::new(4, 7),
        GridPos::new(4, 6),
        GridPos::new(3, 7),
        GridPos::new(5, 7),
        GridPos::new(4, 8),
    ]);
    assert!(events.iter().any(|e| matches!(e, BattleEvent::IntentCommitted { attacker, .. } if *attacker == ids::ARTILLERY)));
}

#[test]
fn out_of_range_enemy_still_commits_a_legal_empty_footprint() {
    let mut battle = isolated_striker_fixture();
    battle.begin_round().unwrap();
    let intent = battle.intent_for(ids::STRIKER).unwrap();
    assert!(intent.intended_occupant.is_none());
    assert!(intent.footprint.iter().all(|cell| battle.board().contains(*cell)));
}
```

- [ ] **Step 2: Run tests and confirm missing planning model**

Run: `cargo test domain::enemy::tests`

Expected: FAIL because intents and enemy planning do not exist.

- [ ] **Step 3: Define committed intent values**

```rust
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
```

`BattleState` owns a `Vec<AttackIntent>` sorted by initiative descending, then `UnitId`. Use initiatives Striker 30, left Rifleman 20, right Rifleman 19, Artillery 10.

- [ ] **Step 4: Implement the authored first-round positioning**

When `round == 0`, move the Striker `(4,4)→(4,6)`, Rifleman L `(2,3)→(2,5)`, Rifleman R `(6,3)→(6,5)`, and leave Artillery at `(4,0)`. Emit `UnitMoved` for actual moves. This is concrete Mission 1 content, not a general scripting layer.

For later rounds:

- enumerate legal destinations reachable within movement, including staying put
- Riflemen minimize distance to the nearest player while preferring cells leaving that player within range 2–4
- Striker minimizes distance to the nearest player while preferring range 1
- Artillery stays if any player is in range 3–8; otherwise it minimizes distance along the central lane
- sort equal scores by `(y, x)` so results are stable

- [ ] **Step 5: Commit one fixed footprint per surviving enemy**

For each enemy after movement, enumerate in-range legal target cells. Prefer a footprint containing the most living players, then intended occupant priority Vanguard, Gunner, Interceptor, then `(y, x)`. If no occupied target cell is in range, choose the legal target cell whose footprint is closest to the nearest player and store `intended_occupant: None`.

Snapshot the attacker's current accuracy and weapon values in `AttackProfile`. Build `intended_preview` only when the chosen footprint contains a living player. Do not mutate footprints after this point.

`begin_round` clears player reactions/activation flags and old intents, performs positioning, commits intents, increments the visible round from 0 to 1, and enters `BattlePhase::Player`.

- [ ] **Step 6: Run planning and domain regressions**

Run: `cargo test domain::enemy::tests`

Run: `cargo test domain`

Expected: exact opening positions and footprints pass; prior movement/combat/environment tests remain green.

- [ ] **Step 7: Commit locked enemy planning**

```bash
git add src/domain src/mission
git commit -m "feat: commit deterministic enemy intents"
```

---

### Task 8: Resolve committed attacks and Counter, Guard, Evade

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/domain/combat.rs`
- Modify: `src/domain/enemy.rs`

**Interfaces:**
- Consumes: sorted `AttackIntent` snapshots, seeded RNG, damage/knockout, player reactions and counter weapon flags.
- Produces: `resolve_enemy_phase`, `resolve_intent`, `resolve_counter`, `IntentCanceled`, `CounterFired`, `AttackHitEmpty` events.

- [ ] **Step 1: Write failing locked-intent and cancellation tests**

Add to `enemy.rs`:

```rust
#[test]
fn moved_victim_is_not_retargeted_and_enemy_in_footprint_is_hit() {
    let mut battle = locked_mortar_fixture(2);
    battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(3, 7));
    let striker_hp = battle.unit(ids::STRIKER).unwrap().hp;

    let events = battle.resolve_intent_for_test(ids::ARTILLERY).unwrap();

    assert_eq!(battle.unit(ids::VANGUARD).unwrap().hp, 20);
    assert!(battle.unit(ids::STRIKER).unwrap().hp < striker_hp);
    assert!(events.iter().any(|e| matches!(e, BattleEvent::AttackHitEmpty { cell, .. } if *cell == GridPos::new(4, 7))));
}

#[test]
fn knocking_out_attacker_cancels_its_pending_intent() {
    let mut battle = locked_mortar_fixture(2);
    battle.apply_direct_damage(ids::ARTILLERY, 99, DamageSource::PlayerWeapon(ids::RAIL_RIFLE));
    let events = battle.resolve_intent_for_test(ids::ARTILLERY).unwrap();
    assert_eq!(events, vec![BattleEvent::IntentCanceled { attacker: ids::ARTILLERY }]);
}
```

- [ ] **Step 2: Write failing reaction tests**

Add to `combat.rs`:

```rust
#[test]
fn guard_reduces_post_armor_damage_and_evade_changes_hit_chance() {
    let guard = incoming_preview_fixture(Reaction::Guard);
    assert_eq!(guard.normal_damage, 1); // Shock Claw 7 - Vanguard armor 3 - Guard 3

    let evade = incoming_preview_fixture(Reaction::Evade);
    let none = incoming_preview_fixture_without_reaction();
    assert_eq!(none.hit_chance.saturating_sub(evade.hit_chance), 25);
}

#[test]
fn counter_uses_authored_weapon_and_en_without_recursion() {
    let mut battle = counter_fixture(2);
    let en_before = battle.unit(ids::INTERCEPTOR).unwrap().en;
    let events = battle.resolve_intent_for_test(ids::RIFLEMAN_RIGHT).unwrap();

    assert_eq!(battle.unit(ids::INTERCEPTOR).unwrap().en, en_before - 1);
    assert_eq!(events.iter().filter(|e| matches!(e, BattleEvent::CounterFired { .. })).count(), 1);
    assert!(!events.iter().any(|e| matches!(e, BattleEvent::CounterFired { defender, .. } if *defender == ids::RIFLEMAN_RIGHT)));
}
```

- [ ] **Step 3: Run focused tests and verify red state**

Run: `cargo test domain::enemy::tests`

Run: `cargo test domain::combat::tests`

Expected: FAIL because enemy resolution and reaction effects are absent.

- [ ] **Step 4: Resolve footprints against current occupants**

`resolve_intent` first checks the attacker. If knocked out, emit only `IntentCanceled`. Otherwise, for each unique committed cell:

- emit `AttackHitEmpty` when no living occupant or un-exploded prop exists
- resolve the snapshotted profile against the current unit's armor/evasion/reaction, regardless of faction
- allow the footprint to damage the explosive prop once
- never replace the cell with an occupant's new position or select a new victim

Use the current occupant's defense but the committed attacker's profile. This is the one resolution path for intended victims, moved-in players and enemy friendly fire.

- [ ] **Step 5: Apply reaction rules after each player hit**

- Guard subtracts 3 after armor, minimum 0.
- Evade adds 25 to current evasion before the 5–95 hit clamp.
- Counter occurs only after a hit that leaves the player alive, only when the attacker remains alive, the designated counter weapon is in legal range/line, and EN is sufficient.
- Counter deducts normal EN once, uses normal hit/crit rules, targets only the attacker, and calls attack resolution with `allow_reaction = false`.
- An area intent gives each affected player at most one counter opportunity.

- [ ] **Step 6: Resolve the full enemy phase in stable order**

`resolve_enemy_phase` requires `Player`, no active unit, and all living players finished. It enters `EnemyResolution`, drains a clone of the sorted intents through `resolve_intent`, checks terminal objectives after each intent, then either stays terminal or calls `begin_round` for the next round.

- [ ] **Step 7: Run all domain tests**

Run: `cargo test domain`

Expected: fixed-footprint, empty-cell, friendly-fire, cancellation, Counter, Guard, Evade and no-recursion cases pass.

- [ ] **Step 8: Commit enemy resolution and reactions**

```bash
git add src/domain
git commit -m "feat: resolve locked attacks and reactions"
```

---

### Task 9: Complete objectives, terminal states and clean restart

**Files:**
- Modify: `src/domain/model.rs`
- Modify: `src/domain/battle.rs`
- Modify: `src/domain/combat.rs`
- Modify: `src/domain/environment.rs`
- Modify: `src/domain/enemy.rs`
- Modify: `src/mission/mission_one.rs`

**Interfaces:**
- Consumes: every damage/knockout source and phase transition.
- Produces: `ObjectiveProgress`, `MissionResult`, `observe_damage_for_objectives`, `check_terminal_state`, `restart_mission(seed)`, objective/result events.

- [ ] **Step 1: Write failing objective and restart tests**

Add to `battle.rs`:

```rust
#[test]
fn enemy_or_environment_damage_to_enemy_completes_turnabout() {
    for source in [
        DamageSource::EnemyWeapon(ids::ARTILLERY, ids::SIEGE_MORTAR),
        DamageSource::Collision,
        DamageSource::Hazard,
        DamageSource::Explosion,
    ] {
        let mut battle = mission_one(7);
        battle.apply_direct_damage(ids::STRIKER, 1, source);
        assert!(battle.objectives().turnabout_complete, "source {source:?}");
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
    knock_out_all_enemies(&mut battle);
    assert_eq!(battle.result(), Some(MissionResult { victory: true, turnabout_complete: false, rounds: 0 }));

    battle.restart_mission(11);
    assert_eq!(battle.phase(), BattlePhase::EnemyPlanning);
    assert_eq!(battle.round(), 0);
    assert!(battle.units().all(|unit| unit.hp == unit.stats.max_hp && unit.en == unit.stats.max_en));
    assert!(battle.intents().is_empty());
    assert!(!battle.objectives().turnabout_complete);

    knock_out_all_players(&mut battle);
    assert_eq!(battle.phase(), BattlePhase::Defeat);
}
```

- [ ] **Step 2: Run tests and confirm red state**

Run: `cargo test domain::battle::tests`

Expected: FAIL because objective/result state is absent.

- [ ] **Step 3: Add explicit objective and result values**

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectiveProgress { pub turnabout_complete: bool }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionResult {
    pub victory: bool,
    pub turnabout_complete: bool,
    pub rounds: u16,
}
```

Every successful damage application calls one observer. Set Turnabout only when the damaged unit is an enemy and the source is enemy weapon, collision, hazard or explosion. Emit `OptionalObjectiveCompleted` only on the false→true transition.

- [ ] **Step 4: Centralize terminal checks**

After every damage batch and phase transition:

- if no living enemies and at least one living player, enter `Victory`, create `MissionResult`, clear active selection, and emit `MissionCompleted`
- else if no living players, enter `Defeat`, create the failed `MissionResult`, clear active selection, and emit `MissionFailed`

Do not enter the next round after a terminal result.

- [ ] **Step 5: Reconstruct rather than partially reset**

Implement `restart_mission(seed)` by replacing `*self` with `mission_one(seed)`. Do not manually reset selected fields. Presentation owns transient selection/event queues and clears those when it receives the restart command.

- [ ] **Step 6: Run full domain suite and commit**

Run: `cargo test domain`

Expected: objective, terminal and clean-restart tests pass with all earlier rules.

```bash
git add src/domain src/mission
git commit -m "feat: complete Mission 1 objectives and retry"
```

---

### Task 10: Project canonical battle state into the full 3D battlefield

**Files:**
- Modify: `src/app.rs`
- Modify: `src/presentation/mod.rs`
- Modify: `src/presentation/assets.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/sync.rs`
- Modify: `assets/models/mission_one.gltf`
- Modify: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: `mission_one(seed)`, units, board cells/props, fixed intents and ordered `BattleEvent` values.
- Produces: `PresentationRoot`, `TelegraphVisual`, `PropVisual`, `BattleEventQueue`, complete board/unit spawning, state synchronization and telegraph reconciliation.

- [ ] **Step 1: Write failing renderer-free telegraph reconciliation test**

Add to `tests/presentation_app.rs`:

```rust
#[test]
fn committed_footprints_create_one_marker_per_unique_cell() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let expected: BTreeSet<_> = battle
        .intents()
        .iter()
        .flat_map(|intent| intent.footprint.iter().copied().map(move |cell| (intent.attacker, cell)))
        .collect();

    let mut app = App::new();
    app.insert_resource(BattleRuntime(battle))
        .add_systems(Update, reconcile_telegraph_markers);
    app.update();

    let actual: BTreeSet<_> = app.world_mut()
        .query::<&TelegraphVisual>()
        .iter(app.world())
        .map(|marker| (marker.attacker, marker.cell))
        .collect();
    assert_eq!(actual, expected);
}
```

- [ ] **Step 2: Run the test and confirm missing full-board projection**

Run: `cargo test --test presentation_app committed_footprints_create_one_marker_per_unique_cell`

Expected: FAIL because `TelegraphVisual` and reconciliation do not exist.

- [ ] **Step 3: Add disposable presentation roots and stable markers**

```rust
#[derive(Component)]
pub struct PresentationRoot;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelegraphVisual { pub attacker: UnitId, pub cell: GridPos }

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropVisual { Blocking(GridPos), Explosive(GridPos), Hazard(GridPos) }

#[derive(Resource, Default)]
pub struct BattleEventQueue(pub VecDeque<BattleEvent>);
```

All Mission 1 scene entities are descendants of one `PresentationRoot`. Reconciliation compares canonical `(attacker, cell)` pairs with marker components, spawning missing markers and despawning stale markers. The renderer-free test exercises components only; a rendering system attaches meshes/materials in the real app.

- [ ] **Step 4: Replace the 3×3 checkpoint board with the authored 9×9 board**

For every logical cell spawn a shallow, pickable mesh tagged `CellVisual`; alternate two dark deck materials by coordinate parity. Spawn glTF scene roots for living units using this exact mapping:

```rust
fn scene_index(archetype: UnitArchetype) -> usize {
    match archetype {
        UnitArchetype::Vanguard => 0,
        UnitArchetype::Gunner => 1,
        UnitArchetype::Interceptor => 2,
        UnitArchetype::Rifleman => 3,
        UnitArchetype::Striker => 4,
        UnitArchetype::Artillery => 5,
    }
}
```

Use scene 6 for every blocking cell, 7 for the explosive, 8 for the hazard, and 9 for short impact effects. Selection rings and cell overlays may use Bevy meshes; visible units/terrain/props/hazard/effects must have a glTF scene root.

Update `grid_to_world` for the 9×9 board:

```rust
pub fn grid_to_world(pos: GridPos) -> Vec3 {
    const HALF: f32 = 4.0;
    Vec3::new(pos.x as f32 - HALF, 0.2, pos.y as f32 - HALF)
}
```

- [ ] **Step 5: Synchronize from canonical state only**

Each update:

- set `UnitVisual` transforms from unit positions
- hide/despawn the glTF visual for knocked-out units without retaining occupancy in ECS
- hide the explosive after `exploded == true`
- attach reaction markers to living player visuals
- reconcile telegraph cells from current intents
- derive selected/reachable/attack-preview highlight materials from interaction resources

No presentation system writes a logical position, HP, EN, phase, intent or objective back into `BattleState`.

- [ ] **Step 6: Add readable world-space telegraphs**

Render committed cells as raised translucent red tiles with a pulsing scale/alpha. Place a thin white ring around each `intended_occupant` and a red line or arrow from attacker to the footprint center. Use different edge glyph meshes for single and Cross 1 footprints so shape remains legible without color.

- [ ] **Step 7: Preserve a visible asset-load failure state**

`assets.rs` owns handles for all ten glTF scenes and observes `AssetServer` load state. Do not enter battle interaction until dependencies load. On failure, set `AssetLoadStatus::Failed("assets/models/mission_one.gltf")`; `ui.rs` renders that exact path in a persistent error panel and logs it once.

- [ ] **Step 8: Run presentation tests and real-app compile**

Run: `cargo test --test presentation_app`

Run: `cargo check --all-targets`

Expected: renderer-free marker/transform tests pass and the real 9×9 app compiles.

- [ ] **Step 9: Commit full battlefield projection**

```bash
git add src/presentation src/app.rs assets tests/presentation_app.rs
git commit -m "feat: render Mission 1 battlefield and threats"
```

---

### Task 11: Implement pointer-complete commands and native Bevy HUD

**Files:**
- Modify: `src/presentation/mod.rs`
- Modify: `src/presentation/interaction.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/app.rs`
- Modify: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: domain command methods/previews, `CellVisual`, `UnitVisual`, load/battle/result state.
- Produces: `InteractionMode`, `SelectedUnit`, `StatusMessage`, `HudSnapshot`, pointer/keyboard command routing and the complete native HUD.

- [ ] **Step 1: Write failing interaction routing and HUD snapshot tests**

Add renderer-free tests:

```rust
#[test]
fn selected_unit_can_move_then_arm_a_weapon() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut interaction = InteractionState::default();

    route_cell_click(&mut battle, &mut interaction, GridPos::new(5, 8)).unwrap();
    assert_eq!(interaction.selected_unit, Some(ids::INTERCEPTOR));
    interaction.mode = InteractionMode::Move;
    route_cell_click(&mut battle, &mut interaction, GridPos::new(5, 7)).unwrap();
    assert!(battle.unit(ids::INTERCEPTOR).unwrap().activation.moved);

    interaction.mode = InteractionMode::Attack(ids::PULSE_CARBINE);
    assert_eq!(interaction.mode, InteractionMode::Attack(ids::PULSE_CARBINE));
}

#[test]
fn hud_snapshot_reports_objectives_unit_allowances_and_threats() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let hud = HudSnapshot::from_battle(&battle, Some(ids::VANGUARD));

    assert_eq!(hud.round_phase, "Round 1 · Player Phase");
    assert_eq!(hud.primary, "Eliminate all enemies · 4 remaining");
    assert_eq!(hud.optional, "Turnabout · Not yet");
    assert_eq!(hud.selected_name, Some("Vanguard"));
    assert_eq!(hud.threats.len(), 4);
}
```

- [ ] **Step 2: Run tests and confirm missing routing/view model**

Run: `cargo test --test presentation_app`

Expected: FAIL because interaction state and HUD snapshot do not exist.

- [ ] **Step 3: Define transient interaction values outside canonical state**

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InteractionMode {
    #[default]
    Inspect,
    Move,
    Attack(WeaponId),
}

#[derive(Resource, Default)]
pub struct InteractionState {
    pub selected_unit: Option<UnitId>,
    pub hovered_cell: Option<GridPos>,
    pub mode: InteractionMode,
    pub preview: Option<AttackPreview>,
}

#[derive(Resource, Default)]
pub struct StatusMessage(pub String);
```

`route_cell_click` performs only these explicit paths: select/begin an unfinished player activation; issue Move to a reachable empty cell; issue Attack against a valid occupied/prop cell; otherwise inspect. Every command calls the domain transition and copies `BattleError` display text into `StatusMessage` without mutating state on failure.

- [ ] **Step 4: Make pointer play complete**

Cell click observers call `route_cell_click`. UI buttons set `InteractionMode`, select one of the active unit's three weapons, choose Counter/Guard/Evade, finish the unit, confirm enemy resolution, or restart. Disable buttons from `HudSnapshot` eligibility, while retaining domain validation as the authority.

Hovering a cell updates inspection and, in attack mode, calls `preview_attack`; show amber footprint, exact hit chance, normal/critical damage, EN cost, and push/collision/hazard outcome. Clear stale preview whenever selection, mode, phase or canonical state changes.

- [ ] **Step 5: Build the native screen-space HUD**

Create one full-screen root with `Pickable::IGNORE` on noninteractive panels and these stable child markers:

```rust
#[derive(Component)] pub struct ObjectiveText;
#[derive(Component)] pub struct ThreatList;
#[derive(Component)] pub struct UnitSummary;
#[derive(Component)] pub struct CommandBar;
#[derive(Component)] pub struct PreviewText;
#[derive(Component)] pub struct StatusText;
#[derive(Component)] pub struct ResultOverlay;
```

Render the exact design regions:

- top-left: primary, Turnabout, round/phase
- top-right: attacker, weapon, fixed cell list, intended occupant, damage and hit chance for every intent
- bottom-left: selected mech HP/EN, Move/Action availability and stance
- bottom center: Move, three named weapons, Counter, Guard, Evade, Finish Unit, Resolve Attacks
- context preview near the command bar
- concise status line above it

Keep telegraphs unobscured and board center visible at 1280×720. Use icon-like glyphs plus short labels; do not create a dashboard-style wall of text.

- [ ] **Step 6: Add keyboard mirrors without making them required**

Map `M` Move, `1`/`2`/`3` weapons, `C` Counter, `G` Guard, `E` Evade, `F` Finish Unit, `Space` Resolve Attacks, and `R` Restart on terminal state. Ignore shortcuts when their corresponding domain command is unavailable.

- [ ] **Step 7: Run renderer-free UI/interaction tests and compile**

Run: `cargo test --test presentation_app`

Run: `cargo check --all-targets`

Expected: routing/snapshot tests pass; the native UI compiles with no second UI dependency.

- [ ] **Step 8: Commit interaction and HUD**

```bash
git add src/presentation src/app.rs tests/presentation_app.rs
git commit -m "feat: add Mission 1 controls and combat HUD"
```

---

### Task 12: Present battle events, terminal results and clean visual restart

**Files:**
- Modify: `src/presentation/mod.rs`
- Modify: `src/presentation/sync.rs`
- Modify: `src/presentation/battlefield.rs`
- Modify: `src/presentation/interaction.rs`
- Modify: `src/presentation/ui.rs`
- Modify: `src/app.rs`
- Modify: `tests/presentation_app.rs`

**Interfaces:**
- Consumes: ordered `BattleEvent` lists, terminal `MissionResult`, `restart_mission(seed)` and disposable `PresentationRoot`.
- Produces: `EventPlayback`, short event animations, result overlay, `restart_battle`, and renderer-free restart lifecycle proof.

- [ ] **Step 1: Write the failing restart lifecycle test**

Add to `tests/presentation_app.rs`:

```rust
#[test]
fn restart_replaces_presentation_root_and_transient_state() {
    let mut app = presentation_fixture_app();
    app.update();
    let old_root = app.world_mut().query_filtered::<Entity, With<PresentationRoot>>()
        .single(app.world()).unwrap();

    app.world_mut().resource_mut::<InteractionState>().selected_unit = Some(ids::VANGUARD);
    app.world_mut().resource_mut::<BattleEventQueue>().0.push_back(
        BattleEvent::OptionalObjectiveCompleted,
    );
    restart_battle(app.world_mut(), 11);
    app.update();

    let new_root = app.world_mut().query_filtered::<Entity, With<PresentationRoot>>()
        .single(app.world()).unwrap();
    assert_ne!(new_root, old_root);
    assert_eq!(app.world().resource::<InteractionState>().selected_unit, None);
    assert!(app.world().resource::<BattleEventQueue>().0.is_empty());
    assert_eq!(app.world().resource::<BattleRuntime>().0.round(), 0);
}
```

- [ ] **Step 2: Run the test and confirm missing restart coordination**

Run: `cargo test --test presentation_app restart_replaces_presentation_root_and_transient_state`

Expected: FAIL because coordinated restart does not exist.

- [ ] **Step 3: Add a small ordered event player**

```rust
#[derive(Resource, Default)]
pub struct EventPlayback {
    pub current: Option<(BattleEvent, Timer)>,
    pub input_locked: bool,
}
```

Domain commands append returned events to `BattleEventQueue`. Playback pops one event at a time, uses 0.12–0.30 second timers for transform lerp, impact flash, damage number, push, explosion and knockout, then synchronizes from canonical state. Empty-hit effects use glTF scene 9. Objective/result events update UI without delaying more than 0.2 seconds.

Input observers return early only while `input_locked`; inspection remains available when no animation is active. Canonical state is already final, so skipping an animation cannot change outcomes.

- [ ] **Step 4: Show clear terminal overlays**

Victory copy:

```text
MISSION COMPLETE
Relay Nine secured
Turnabout: Achieved | Missed
Restart Mission
```

Failure copy:

```text
MISSION FAILED
Squad knocked out
Restart Mission
```

Disable normal command buttons in terminal phases. `R` and the visible Restart button call the same restart function.

- [ ] **Step 5: Rebuild canonical and presentation state together**

`restart_battle(world, seed)` calls `BattleRuntime.0.restart_mission(seed)`, recursively despawns the old `PresentationRoot`, clears `InteractionState`, `StatusMessage`, `BattleEventQueue` and `EventPlayback`, then queues the normal spawn system. It must not patch individual unit entities or UI labels.

- [ ] **Step 6: Run lifecycle and full automated tests**

Run: `cargo test --all-targets`

Expected: restart test passes; domain and presentation suites remain green.

- [ ] **Step 7: Run the app through one failure/restart and victory/restart**

Run: `cargo run`

Expected manual evidence: both result overlays appear, Restart returns to the initial full-HP/full-EN board with no stale highlights/intents, and input never mutates state during event playback.

- [ ] **Step 8: Commit event presentation and retry**

```bash
git add src/presentation src/app.rs tests/presentation_app.rs
git commit -m "feat: present combat outcomes and clean retry"
```

---

### Task 13: Close acceptance gaps and record final validation

**Files:**
- Modify: `docs/validation/hpa-632.md`
- Create: `README.md`
- Modify: source/tests only for concrete failures found by the gates

**Interfaces:**
- Consumes: the complete Mission 1 application and every HPA-632 acceptance criterion.
- Produces: reproducible run/control documentation, acceptance matrix, automated-gate evidence and manual playtest conclusions.

- [ ] **Step 1: Add concise run and control documentation**

Create `README.md` with prerequisites (stable Rust supporting edition 2024), `cargo run`, the pointer flow, keyboard mappings, Mission 1 primary/Turnabout objectives, and the four local verification commands. State that the app starts directly in Mission 1 and contains no save/campaign flow yet.

- [ ] **Step 2: Run formatting and fix only reported formatting changes**

Run: `cargo fmt --check`

If it fails, run `cargo fmt`, inspect the formatting-only diff, then rerun `cargo fmt --check` until exit 0.

- [ ] **Step 3: Run strict Clippy and fix concrete findings**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Expected: exit 0. Fix warnings at their source; do not add broad `allow` attributes.

- [ ] **Step 4: Run the complete automated suite**

Run: `cargo test --all-targets`

Expected: exit 0 with focused coverage for movement, activation, all nine player weapons, seeded hit/miss/crit, EN, knockout, locked intents, empty/different occupants, cancelation, reactions, environment exactly-once behavior, objectives and restart.

- [ ] **Step 5: Run the release-build sanity check**

Run: `cargo build --release`

Expected: exit 0.

- [ ] **Step 6: Perform the retained Mission 1 playtest**

Run: `cargo run --release`

Play at least one complete victory and one failure/restart. Record in `docs/validation/hpa-632.md`:

- hardware/OS and exact commit
- start/end timestamps and first-clear duration
- whether every telegraph remained readable at 1280×720
- whether the opening Vanguard move caused the committed Mortar to threaten/hit the Striker without retargeting
- at least one observed push/collision, hazard and explosive event with no duplicate damage
- observed Counter, Guard and Evade behavior
- primary and Turnabout result behavior
- clean restart after failure and victory
- yes/no conclusion for Bevy 2.5D + native UI maintainability
- yes/no conclusion for intent manipulation versus pure damage optimization

If either conclusion is no, keep HPA-632 open and make the smallest evidence-driven revision inside this branch before rerunning the gates.

- [ ] **Step 7: Complete the acceptance matrix**

Copy every checkbox from the live HPA-632 acceptance criteria into `docs/validation/hpa-632.md`. For each, link or name its automated test, source seam, manual observation, or CI command. Do not mark a criterion passed without evidence.

- [ ] **Step 8: Inspect the whole diff for scope**

Run: `git status --short`

Run: `git diff --check main...HEAD`

Run: `git diff --stat main...HEAD`

Expected: only the one application crate, checked-in assets, focused tests, CI, README, design/plan and validation evidence; no campaign/save/backend/physics/editor/framework additions.

- [ ] **Step 9: Commit final validation evidence**

```bash
git add README.md docs/validation/hpa-632.md src tests assets Cargo.toml Cargo.lock .github/workflows/ci.yml
git commit -m "docs: record HPA-632 validation evidence"
```

- [ ] **Step 10: Re-run all four gates on the final commit**

Run: `cargo fmt --check`

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Run: `cargo test --all-targets`

Run: `cargo build --release`

Expected: all exit 0. Do not claim HPA-632 complete while the manual validation document has an unanswered or failed product question.
