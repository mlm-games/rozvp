//! Seed packet definitions. Canonical numbers; sources in docs/PARITY.md.

use bevy::color::Color;

use crate::game::constants::{
    FAST_RECHARGE_TICKS, PEASHOOTER_ATTACK_COOLDOWN_TICKS, PLANT_HP_DEFAULT, POTATO_MINE_COST,
    SLOW_RECHARGE_TICKS, SNOW_PEA_ATTACK_COOLDOWN_TICKS, SNOW_PEA_COST, SQUASH_COST,
    THREEPEATER_COST, TALLNUT_COST, TALLNUT_HP, TORCHWOOD_COST, SPIKEWEED_COST, GARLIC_COST,
    GARLIC_HP, HYPNO_SHROOM_COST, ICE_SHROOM_COST, DOOM_SHROOM_COST, VERY_SLOW_RECHARGE_TICKS,
    JALAPENO_COST, WALLNUT_HP,
};

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
pub struct SeedDef {
    pub kind: PlantKind,
    pub name: &'static str,
    pub cost: i32,
    pub recharge_ticks: i32,
    pub color: Color,
    pub hp: i32,
    pub attack_cooldown_ticks: i32,
}

pub const SEED_DEFS: &[SeedDef] = &[
    SeedDef {
        kind: PlantKind::Sunflower,
        name: "Sunflower",
        cost: 50,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.95, 0.82, 0.20),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::Peashooter,
        name: "Peashooter",
        cost: 100,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.25, 0.72, 0.28),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: PEASHOOTER_ATTACK_COOLDOWN_TICKS,
    },
    SeedDef {
        kind: PlantKind::Repeater,
        name: "Repeater",
        cost: 200,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.20, 0.58, 0.22),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: PEASHOOTER_ATTACK_COOLDOWN_TICKS,
    },
    SeedDef {
        kind: PlantKind::SnowPea,
        name: "Snow Pea",
        cost: SNOW_PEA_COST, // 175
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.55, 0.82, 0.95),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: SNOW_PEA_ATTACK_COOLDOWN_TICKS,
    },
    // Wall-nut: SLOW 30 s — NOT fast.
    SeedDef {
        kind: PlantKind::WallNut,
        name: "Wall-nut",
        cost: 50,
        recharge_ticks: SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.58, 0.39, 0.20),
        hp: WALLNUT_HP,
        attack_cooldown_ticks: 0,
    },
    // Cherry Bomb: VERY SLOW 50 s
    SeedDef {
        kind: PlantKind::PotatoMine,
        name: "Potato Mine",
        cost: POTATO_MINE_COST, // 25
        recharge_ticks: SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.60, 0.45, 0.25),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::CherryBomb,
        name: "Cherry Bomb",
        cost: 150,
        recharge_ticks: VERY_SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.78, 0.16, 0.16),
        // Single-use; HP only matters if eaten mid-fuse.
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::Chomper,
        name: "Chomper",
        cost: 150,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.65, 0.25, 0.80),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    // Squash: one-time crush (wiki PvZ1: slow recharge).
    SeedDef {
        kind: PlantKind::Squash,
        name: "Squash",
        cost: SQUASH_COST,
        recharge_ticks: SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.35, 0.55, 0.20),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::Threepeater,
        name: "Threepeater",
        cost: THREEPEATER_COST,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.15, 0.60, 0.30),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: PEASHOOTER_ATTACK_COOLDOWN_TICKS,
    },
    SeedDef {
        kind: PlantKind::Jalapeno,
        name: "Jalapeno",
        cost: JALAPENO_COST,
        recharge_ticks: VERY_SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.90, 0.25, 0.10),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    // Spikeweed is walked over, never eaten; HP only matters vs instant kills.
    SeedDef {
        kind: PlantKind::Spikeweed,
        name: "Spikeweed",
        cost: SPIKEWEED_COST,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.30, 0.45, 0.25),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::Torchwood,
        name: "Torchwood",
        cost: TORCHWOOD_COST,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.70, 0.40, 0.15),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::TallNut,
        name: "Tall-nut",
        cost: TALLNUT_COST,
        recharge_ticks: SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.62, 0.45, 0.25),
        hp: TALLNUT_HP,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::Garlic,
        name: "Garlic",
        cost: GARLIC_COST,
        recharge_ticks: FAST_RECHARGE_TICKS,
        color: Color::srgb(0.90, 0.88, 0.75),
        hp: GARLIC_HP,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::HypnoShroom,
        name: "Hypno-shroom",
        cost: HYPNO_SHROOM_COST,
        recharge_ticks: SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.75, 0.35, 0.60),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::IceShroom,
        name: "Ice-shroom",
        cost: ICE_SHROOM_COST,
        recharge_ticks: VERY_SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.60, 0.80, 0.95),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
    SeedDef {
        kind: PlantKind::DoomShroom,
        name: "Doom-shroom",
        cost: DOOM_SHROOM_COST,
        recharge_ticks: VERY_SLOW_RECHARGE_TICKS,
        color: Color::srgb(0.25, 0.20, 0.35),
        hp: PLANT_HP_DEFAULT,
        attack_cooldown_ticks: 0,
    },
];

pub fn seed_def(name: &str) -> Option<&'static SeedDef> {
    SEED_DEFS.iter().find(|d| d.name == name)
}
