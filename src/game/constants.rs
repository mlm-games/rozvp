//! Board layout + timing constants. Canonical PvZ numbers on a 100 Hz tick.
//! Sources tracked in docs/PARITY.md: W=wiki, D=decomp still needed, E=engine choice.
#![allow(dead_code)]

pub const BOARD_WIDTH: f32 = 800.0;
pub const BOARD_HEIGHT: f32 = 600.0;

pub const LAWN_XMIN: f32 = 40.0;
pub const LAWN_YMIN: f32 = 80.0;
pub const GRID_CELL_W: f32 = 80.0;
pub const GRID_CELL_H: f32 = 100.0;

pub const LAWN_COLS: usize = 9;
pub const LAWN_ROWS: usize = 5;

/// Adventure levels per area (1-x, 2-x, ...).
pub const LEVELS_PER_AREA: u32 = 10;

/// Zombie x at which the house is considered breached.
pub const HOUSE_X_LOGIC: f32 = LAWN_XMIN;

pub const PLANT_Z: f32 = 10.0;
pub const MOWER_Z: f32 = 15.0;
pub const SUN_Z: f32 = 20.0;
pub const ZOMBIE_Z: f32 = 12.0;

pub const TICK_HZ: f32 = 100.0;
pub const TICK: f32 = 1.0 / TICK_HZ;

#[inline]
pub fn ticks_to_secs(t: i32) -> f32 {
    t as f32 * TICK
}

#[inline]
pub fn secs_to_ticks(s: f32) -> i32 {
    (s * TICK_HZ).round() as i32
}

// Recharge tiers (100 Hz ticks) ───────────────────────────
pub const FAST_RECHARGE_TICKS: i32 = 750; // 7.5 s
pub const SLOW_RECHARGE_TICKS: i32 = 3000; // 30 s
pub const VERY_SLOW_RECHARGE_TICKS: i32 = 5000; // 50 s

// Economy
pub const STARTING_SUN: i32 = 50;
pub const SUN_VALUE: i32 = 25;
pub const SUN_LIFETIME_TICKS: i32 = 800; // ~8 s uncollected
pub const SKY_SUN_INTERVAL_TICKS: i32 = 425; // engine-side until decomp read

// Sunflower
// Wiki: ~24.25 s production. First sun shorter in practice.
pub const SUNFLOWER_FIRST_SUN_TICKS: i32 = 700; // ~7.0 s (approx, mark D)
pub const SUNFLOWER_SUN_INTERVAL_TICKS: i32 = 2425; // 24.25 s
pub const SUNFLOWER_SUN_VALUE: i32 = 25;

// Peashooter
// Wiki: 20 dmg, average interval 1.425 s
pub const PEA_DAMAGE: i32 = 20;
pub const PEASHOOTER_ATTACK_COOLDOWN_TICKS: i32 = 143; // 1.425 s @ 100 Hz
pub const PEA_SPEED_PPS: f32 = 260.0; // engine feel; not a wiki number
pub const PEA_HIT_RADIUS: f32 = 22.0; // engine feel

// Snow Pea
pub const SNOW_PEA_COST: i32 = 175;
pub const SNOW_PEA_ATTACK_COOLDOWN_TICKS: i32 = 150; // ~1.5 s
pub const CHILLED_SPEED_MULTIPLIER: f32 = 0.5;

// TODO(D): confirm exact PvZ1 decomp duration in Board.cpp / projectile hit logic.
// Using the common community value for now.
pub const SNOW_PEA_CHILL_TICKS: i32 = 1000; // 10 s

// Plant HP
pub const PLANT_HP_DEFAULT: i32 = 300;
pub const WALLNUT_HP: i32 = 4000;

// Visual crack thresholds for Wall-nut (fractions of max HP).
pub const WALLNUT_CRACK1_FRAC: f32 = 0.66;
pub const WALLNUT_CRACK2_FRAC: f32 = 0.33;

// Potato Mine ─────────────────────────────────────────────
pub const POTATO_MINE_COST: i32 = 25;
pub const POTATO_MINE_ARM_TICKS: i32 = 1500; // 15 s @ 100 Hz
pub const POTATO_MINE_DAMAGE: i32 = 1800;
pub const POTATO_MINE_TRIGGER_DIST: f32 = 30.0;

// Chomper
pub const CHOMPER_REACH: f32 = 90.0;
pub const CHOMPER_CHEW_TICKS: i32 = 4200; // 42 s digest

// Squash
pub const SQUASH_COST: i32 = 50;
pub const SQUASH_TRIGGER_RANGE_PX: f32 = 1.2 * GRID_CELL_W;
pub const SQUASH_DAMAGE: i32 = 1800;

// Threepeater ─────────────────────────────────────────────
pub const THREEPEATER_COST: i32 = 325;

// Jalapeño
pub const JALAPENO_COST: i32 = 125;
pub const JALAPENO_FUSE_TICKS: i32 = 100; // 1.0 s
pub const JALAPENO_DAMAGE: i32 = 1800;

// Spikeweed
pub const SPIKEWEED_COST: i32 = 100;
pub const SPIKEWEED_DAMAGE: i32 = 20;
pub const SPIKEWEED_HIT_INTERVAL_TICKS: i32 = 100; // 1 s per hit per zombie standing on it

// Torchwood
pub const TORCHWOOD_COST: i32 = 175;
pub const FIRE_PEA_DAMAGE: i32 = 40; // 2x pea

// Tall-nut
pub const TALLNUT_COST: i32 = 125;
pub const TALLNUT_HP: i32 = 8000;

// Garlic
pub const GARLIC_COST: i32 = 50;
pub const GARLIC_HP: i32 = 400;

// Hypno-shroom ────────────────────────────────────────────
pub const HYPNO_SHROOM_COST: i32 = 75;

// Ice-shroom
pub const ICE_SHROOM_COST: i32 = 75;
pub const ICE_SHROOM_FUSE_TICKS: i32 = 100; // 1.0 s
/// True freeze-stun before the lingering chill (approx; TODO(D) exact).
pub const ICE_SHROOM_FREEZE_TICKS: i32 = 400; // 4 s frozen solid
pub const ICE_SHROOM_CHILL_TICKS: i32 = 1000; // 10 s slow after thaw

// Doom-shroom ─────────────────────────────────────────────
pub const DOOM_SHROOM_COST: i32 = 125;
pub const DOOM_SHROOM_FUSE_TICKS: i32 = 100; // 1.0 s
pub const DOOM_SHROOM_DAMAGE: i32 = 9000;
pub const DOOM_CRATER_TICKS: i32 = 18000; // 180 s unplantable

// Cherry Bomb ─────────────────────────────────────────────
// Wiki: 1.2 s fuse, 1800 dmg, 3x3 tiles, very slow recharge
pub const CHERRY_BOMB_FUSE_TICKS: i32 = 120; // 1.2 s
pub const CHERRY_BOMB_DAMAGE: i32 = 1800;
/// Final-window flash before detonation.
pub const CHERRY_FLASH_TICKS: i32 = 40;
// Area is grid Chebyshev <= 1 tile around the bomb cell (see specials.rs).

// Zombies
// Common PvZ1 damage table: normal 270, cone +370, bucket +1100
pub const ZOMBIE_BODY_HP: i32 = 270;
pub const CONE_ARMOR_HP: i32 = 370; // total 640
pub const BUCKET_ARMOR_HP: i32 = 1100; // total 1370

/// ~4.7 s per 80 px tile ≈ 17 px/s (approx until decomp walk table)
pub const ZOMBIE_SPEED_PPS: f32 = 17.0;

/// 100 dps => 50 dmg every 0.5 s (6 bites clear a 300 HP plant in 3 s)
pub const ZOMBIE_EAT_DAMAGE: i32 = 50;
pub const ZOMBIE_EAT_INTERVAL_TICKS: i32 = 50;
pub const ZOMBIE_EAT_REACH: f32 = 40.0;

// Mowers (layout/feel; not almanac) ───────────────────────
pub const MOWER_X_LOGIC: f32 = 20.0;
pub const MOWER_TRIGGER_SLACK: f32 = 24.0;
pub const MOWER_KILL_HALF_WIDTH: f32 = 34.0;
pub const MOWER_SPEED_PPS: f32 = 280.0;

// Advice
pub const ADVICE_HOLD_TICKS: i32 = 600;

// Wave timing (100 Hz ticks) ──────────────────────────────
pub const FIRST_WAVE_DELAY_TICKS: i32 = 1800; // ~18 s — matches decomp-style first countdown
pub const INTER_WAVE_DELAY_TICKS: i32 = 600; // ~6 s between waves (engine feel; TODO(D) exact)
pub const SPAWN_STAGGER_TICKS: i32 = 80; // gap between individuals in a wave

// Zombie point costs (wiki / Wave Points table) ───────────
pub const POINT_NORMAL: u32 = 1;
pub const POINT_FLAG: u32 = 1;
pub const POINT_CONE: u32 = 2;
pub const POINT_BUCKET: u32 = 4;

// Huge-wave advice hold
pub const HUGE_WAVE_ADVICE_TICKS: i32 = 400;

// Misc engine feel ────────────────────────────────────────
pub const SUN_PICK_RADIUS: f32 = 28.0;
