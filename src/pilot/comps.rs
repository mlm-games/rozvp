//! Pilot components + resources. Manual `Component`/`Resource` impls
//! throughout: the graph carries two bevy_ecs versions (0.19 via
//! repame-sim, 0.20-dev via bevy) and `#[derive(Component)]` would bind
//! to the wrong one (see `pilot.rs` prototype notes).

use repame_sim::bevy_ecs::component::{Mutable, StorageType};
use repame_sim::bevy_ecs::prelude::*;

macro_rules! component {
    ($($t:ty),*) => {
        $(
            impl Component for $t {
                const STORAGE_TYPE: StorageType = StorageType::Table;
                type Mutability = Mutable;
            }
            impl Resource for $t {}
        )*
    };
}

/// Logic-space position (top-left origin, y-down, pixels). Replaces bevy
/// `Transform` (world space) everywhere in the pilot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pos {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZombieKind {
    Normal,
    Flag,
    Conehead,
    Buckethead,
}

#[derive(Clone, Copy, Debug)]
pub struct Zombie {
    pub kind: ZombieKind,
    pub row: usize,
    pub body_hp: i32,
    pub armor_hp: i32,
    pub speed_pps: f32,
    pub eat_cooldown_remaining: i32,
    pub hypnotized: bool,
}

impl Zombie {
    pub fn new(kind: ZombieKind, row: usize) -> Self {
        use crate::game::constants::ZOMBIE_SPEED_PPS;
        use crate::game::constants::{BUCKET_ARMOR_HP, CONE_ARMOR_HP, ZOMBIE_BODY_HP};
        let armor_hp = match kind {
            ZombieKind::Normal | ZombieKind::Flag => 0,
            ZombieKind::Conehead => CONE_ARMOR_HP,
            ZombieKind::Buckethead => BUCKET_ARMOR_HP,
        };
        Self {
            kind,
            row,
            body_hp: ZOMBIE_BODY_HP,
            armor_hp,
            speed_pps: ZOMBIE_SPEED_PPS,
            eat_cooldown_remaining: 0,
            hypnotized: false,
        }
    }

    /// Armor first, overflow to body. Returns true if dead.
    pub fn take_damage(&mut self, amount: i32) -> bool {
        if amount <= 0 {
            return self.body_hp <= 0;
        }
        if self.armor_hp > 0 {
            self.armor_hp -= amount;
            if self.armor_hp < 0 {
                self.body_hp += self.armor_hp;
                self.armor_hp = 0;
            }
        } else {
            self.body_hp -= amount;
        }
        self.body_hp <= 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Chilled {
    pub remaining_ticks: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct Frozen {
    pub remaining_ticks: i32,
    pub chill_after_ticks: i32,
}

/// Corpse playing the fall one-shot. Replaces the baked-atlas `Dying`
/// countdown; the live rig listens for the same transition.
#[derive(Clone, Copy, Debug)]
pub struct Dying {
    pub remaining_ticks: i32,
}

/// Biting marker, set/cleared per tick. Drives the rig `eating` input.
#[derive(Clone, Copy, Debug)]
pub struct Eating;

/// Plant kinds. Mirrors `game::defs::PlantKind` without bevy types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlantKind {
    Sunflower,
    Peashooter,
    SnowPea,
    Repeater,
    WallNut,
    CherryBomb,
    PotatoMine,
    Chomper,
    Squash,
    Threepeater,
    Jalapeno,
    Spikeweed,
    Torchwood,
    TallNut,
    Garlic,
    HypnoShroom,
    IceShroom,
    DoomShroom,
}

#[derive(Clone, Copy, Debug)]
pub struct Plant {
    pub kind: PlantKind,
    pub row: usize,
    pub col: usize,
    pub hp: i32,
    pub attack_cooldown_remaining: i32,
    pub attack_cooldown_max: i32,
}

/// Seed packet definition with render-ready color. Mirrors `SEED_DEFS`
/// (same numbers, `[f32; 4]` linear-ish tint instead of bevy `Color`).
#[derive(Clone, Copy, Debug)]
pub struct SeedDef {
    pub kind: PlantKind,
    pub name: &'static str,
    pub cost: i32,
    pub recharge_ticks: i32,
    pub color: [f32; 4],
    pub hp: i32,
    pub attack_cooldown_ticks: i32,
    /// Display size in logic px (matches current sprite custom_size).
    pub size: (f32, f32),
}

pub const SEED_DEFS: &[SeedDef] = &[
    SeedDef {
        kind: PlantKind::Sunflower,
        name: "Sunflower",
        cost: 50,
        recharge_ticks: 750,
        color: [0.95, 0.82, 0.20, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::Peashooter,
        name: "Peashooter",
        cost: 100,
        recharge_ticks: 750,
        color: [0.25, 0.72, 0.28, 1.0],
        hp: 300,
        attack_cooldown_ticks: 143,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::Repeater,
        name: "Repeater",
        cost: 200,
        recharge_ticks: 750,
        color: [0.20, 0.58, 0.22, 1.0],
        hp: 300,
        attack_cooldown_ticks: 143,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::SnowPea,
        name: "Snow Pea",
        cost: 175,
        recharge_ticks: 750,
        color: [0.55, 0.82, 0.95, 1.0],
        hp: 300,
        attack_cooldown_ticks: 150,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::WallNut,
        name: "Wall-nut",
        cost: 50,
        recharge_ticks: 3000,
        color: [0.58, 0.39, 0.20, 1.0],
        hp: 4000,
        attack_cooldown_ticks: 0,
        size: (58.0, 64.0),
    },
    SeedDef {
        kind: PlantKind::PotatoMine,
        name: "Potato Mine",
        cost: 25,
        recharge_ticks: 3000,
        color: [0.60, 0.45, 0.25, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::CherryBomb,
        name: "Cherry Bomb",
        cost: 150,
        recharge_ticks: 5000,
        color: [0.78, 0.16, 0.16, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (54.0, 54.0),
    },
    SeedDef {
        kind: PlantKind::Chomper,
        name: "Chomper",
        cost: 150,
        recharge_ticks: 750,
        color: [0.65, 0.25, 0.80, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::Squash,
        name: "Squash",
        cost: 50,
        recharge_ticks: 3000,
        color: [0.35, 0.55, 0.20, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::Threepeater,
        name: "Threepeater",
        cost: 325,
        recharge_ticks: 750,
        color: [0.15, 0.60, 0.30, 1.0],
        hp: 300,
        attack_cooldown_ticks: 143,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::Jalapeno,
        name: "Jalapeno",
        cost: 125,
        recharge_ticks: 5000,
        color: [0.90, 0.25, 0.10, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::Spikeweed,
        name: "Spikeweed",
        cost: 100,
        recharge_ticks: 750,
        color: [0.30, 0.45, 0.25, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (64.0, 22.0),
    },
    SeedDef {
        kind: PlantKind::Torchwood,
        name: "Torchwood",
        cost: 175,
        recharge_ticks: 750,
        color: [0.70, 0.40, 0.15, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::TallNut,
        name: "Tall-nut",
        cost: 125,
        recharge_ticks: 3000,
        color: [0.62, 0.45, 0.25, 1.0],
        hp: 8000,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::Garlic,
        name: "Garlic",
        cost: 50,
        recharge_ticks: 750,
        color: [0.90, 0.88, 0.75, 1.0],
        hp: 400,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::HypnoShroom,
        name: "Hypno-shroom",
        cost: 75,
        recharge_ticks: 3000,
        color: [0.75, 0.35, 0.60, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::IceShroom,
        name: "Ice-shroom",
        cost: 75,
        recharge_ticks: 5000,
        color: [0.60, 0.80, 0.95, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
    SeedDef {
        kind: PlantKind::DoomShroom,
        name: "Doom-shroom",
        cost: 125,
        recharge_ticks: 5000,
        color: [0.25, 0.20, 0.35, 1.0],
        hp: 300,
        attack_cooldown_ticks: 0,
        size: (56.0, 72.0),
    },
];

pub fn seed_def(name: &str) -> Option<&'static SeedDef> {
    SEED_DEFS.iter().find(|d| d.name == name)
}

/// FTL key for a seed's display name. Mirrors `game::defs::seed_key`;
/// internal names stay the save/logic identity.
pub fn seed_key(name: &str) -> String {
    let mut slug = String::with_capacity(name.len() + 5);
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if c == ' ' || c == '_' || c == '-' {
            slug.push('-');
        }
    }
    format!("seed-{slug}")
}

pub fn seed_def_by_kind(kind: PlantKind) -> Option<&'static SeedDef> {
    SEED_DEFS.iter().find(|d| d.kind == kind)
}

#[derive(Clone, Copy, Debug)]
pub struct SunDrop {
    pub born_tick: i64,
    pub value: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct SkySun {
    pub target_logic_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeaKind {
    Normal,
    Snow,
    Fire,
}

#[derive(Clone, Copy, Debug)]
pub struct PeaProjectile {
    pub kind: PeaKind,
    pub row: usize,
    pub damage: i32,
    pub speed_pps: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SunProducer {
    pub countdown: i32,
}

impl Default for SunProducer {
    fn default() -> Self {
        Self { countdown: 700 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CherryBombFuse {
    pub remaining: i32,
}

impl Default for CherryBombFuse {
    fn default() -> Self {
        Self { remaining: 120 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PotatoMineState {
    pub armed: bool,
    pub arm_timer: i32,
}

impl Default for PotatoMineState {
    fn default() -> Self {
        Self {
            armed: false,
            arm_timer: 1500,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ChomperState {
    pub chewing_timer: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct JalapenoFuse {
    pub remaining: i32,
}

impl Default for JalapenoFuse {
    fn default() -> Self {
        Self { remaining: 100 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpikeweedState {
    pub cooldowns: Vec<(Entity, i32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct IceShroomFuse {
    pub remaining: i32,
}

impl Default for IceShroomFuse {
    fn default() -> Self {
        Self { remaining: 100 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DoomShroomFuse {
    pub remaining: i32,
}

impl Default for DoomShroomFuse {
    fn default() -> Self {
        Self { remaining: 100 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Crater {
    pub remaining: i32,
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct LawnMower {
    pub row: usize,
    pub active: bool,
}

/// Marks level-owned entities for teardown on exit/restart.
#[derive(Clone, Copy, Debug)]
pub struct GameplayCleanup;

component!(
    Pos,
    Zombie,
    Chilled,
    Frozen,
    Dying,
    Eating,
    Plant,
    SunDrop,
    SkySun,
    PeaProjectile,
    SunProducer,
    CherryBombFuse,
    PotatoMineState,
    ChomperState,
    JalapenoFuse,
    SpikeweedState,
    IceShroomFuse,
    DoomShroomFuse,
    Crater,
    LawnMower,
    GameplayCleanup
);
