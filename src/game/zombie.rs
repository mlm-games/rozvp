//! Zombies: wave director, walking/eating, lose & win conditions.

use bevy::prelude::*;
use rand::RngExt;

use crate::app::{OverlayMenu, Paused};
use crate::game::board::Board;
use crate::game::constants::{
    GRID_CELL_H, GRID_CELL_W, HOUSE_X_LOGIC, INTER_WAVE_DELAY_TICKS, LAWN_COLS, LAWN_ROWS,
    LAWN_XMIN, LAWN_YMIN, PLANT_Z, POINT_BUCKET, POINT_CONE, POINT_FLAG, POINT_NORMAL,
    SPAWN_STAGGER_TICKS, TICK_HZ, ZOMBIE_EAT_DAMAGE, ZOMBIE_EAT_INTERVAL_TICKS, ZOMBIE_EAT_REACH,
};
use crate::game::systems::{GameplayCleanup, Plant};
use crate::game::tick::FrameTicks;
use crate::game::zombie_anim::{Dying, Eating};
use crate::menus::UiBridge;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Flag spawns with real wave composition (WAVE-001)
pub enum ZombieKind {
    Normal,
    Flag,
    Conehead,
    Buckethead,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Zombie {
    #[allow(dead_code)] // read by armor visuals + almanac in later phases
    pub kind: ZombieKind,
    pub row: usize,
    pub body_hp: i32,
    pub armor_hp: i32,
    pub speed_pps: f32,
    pub eat_cooldown_remaining: i32,
    /// Hypnotized zombies walk right and brawl their former allies.
    pub hypnotized: bool,
}

impl Zombie {
    pub fn new(kind: ZombieKind, row: usize) -> Self {
        use crate::game::constants::{
            BUCKET_ARMOR_HP, CONE_ARMOR_HP, ZOMBIE_BODY_HP, ZOMBIE_SPEED_PPS,
        };

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
            // Flag speed left equal until verified; do not invent a multiplier.
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
                self.body_hp += self.armor_hp; // armor_hp is negative overflow
                self.armor_hp = 0;
            }
        } else {
            self.body_hp -= amount;
        }

        self.body_hp <= 0
    }

    #[allow(dead_code)] // convenience for future systems
    pub fn is_dead(&self) -> bool {
        self.body_hp <= 0
    }
}

fn zombie_color(kind: ZombieKind) -> Color {
    match kind {
        ZombieKind::Normal => Color::srgb(0.42, 0.62, 0.36),
        ZombieKind::Flag => Color::srgb(0.80, 0.20, 0.20),
        ZombieKind::Conehead => Color::srgb(0.90, 0.50, 0.10),
        ZombieKind::Buckethead => Color::srgb(0.60, 0.60, 0.70),
    }
}

/// Snow Pea status: halves movement immediately and doubles the NEXT bite
/// cooldown while present. Refreshed by every snow pea hit.
#[derive(Component, Clone, Copy, Debug)]
pub struct Chilled {
    pub remaining_ticks: i32,
}

/// Ice-shroom stun: fully halts movement and eating, then converts into a
/// lingering [`Chilled`] on thaw (`chill_after_ticks`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Frozen {
    pub remaining_ticks: i32,
    pub chill_after_ticks: i32,
}

#[derive(Resource, Debug)]
pub struct WaveState {
    /// All planned waves for this level.
    pub wave_plans: Vec<Vec<ZombieKind>>,

    /// Whether each wave is a huge wave (index-aligned with `wave_plans`).
    pub huge_flags: Vec<bool>,

    /// 0-based index of the wave currently being played or waiting to start.
    pub current_wave: u32,

    /// Total waves in this level.
    pub num_waves: u32,

    /// Whether a wave is actively spawning / on the lawn.
    pub started: bool,

    /// Whether we are in the inter-wave gap between two waves.
    pub between_waves: bool,

    /// Whether every wave is finished.
    pub cleared: bool,

    /// Whether the current wave is huge (drives the advice line).
    pub huge_wave: bool,

    /// Countdown before the first wave of the level.
    pub start_delay_remaining: i32,

    /// Countdown during inter-wave gaps.
    pub inter_wave_remaining: i32,

    /// Spawn cadence inside the active wave.
    pub spawn_timer_remaining: i32,

    /// Current wave plan.
    pub planned: Vec<ZombieKind>,

    /// Remaining zombies to spawn in current wave.
    pub queued: u32,

    /// Spawned in current wave.
    pub spawned: u32,

    /// Total zombies across the whole level.
    pub total_in_level: u32,

    /// Total zombies spawned across all waves so far.
    pub total_spawned_all: u32,

    /// For debugging / parity tracking.
    pub budget_spent: u32,
}

impl Default for WaveState {
    fn default() -> Self {
        Self {
            wave_plans: Vec::new(),
            huge_flags: Vec::new(),
            current_wave: 0,
            num_waves: 1,
            started: false,
            between_waves: false,
            cleared: false,
            huge_wave: false,
            start_delay_remaining: crate::game::constants::FIRST_WAVE_DELAY_TICKS,
            inter_wave_remaining: 0,
            spawn_timer_remaining: 0,
            planned: Vec::new(),
            queued: 0,
            spawned: 0,
            total_in_level: 0,
            total_spawned_all: 0,
            budget_spent: 0,
        }
    }
}

impl WaveState {
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.started && self.queued > 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WaveRecipe {
    pub budget: u32,
    pub max_cone: u32,
    pub max_bucket: u32,
    pub final_flag: bool,
    /// Huge wave: flag leads the pack, denser caps, advice line.
    pub huge: bool,
}

fn zombie_point_cost(kind: ZombieKind) -> u32 {
    match kind {
        ZombieKind::Normal => POINT_NORMAL,
        ZombieKind::Flag => POINT_FLAG,
        ZombieKind::Conehead => POINT_CONE,
        ZombieKind::Buckethead => POINT_BUCKET,
    }
}

/// Budget-based ZombiePicker-lite.
///
/// Intentionally simple:
/// - spends a point budget using wiki point costs,
/// - respects per-level caps,
/// - shuffles in cone/bucket only if budget/caps allow,
/// - on huge waves the flag zombie leads and does not consume combat budget.
///
/// TODO(D): replace weights with exact Board.cpp PickZombieWaves data.
fn build_wave_plan(recipe: WaveRecipe) -> (Vec<ZombieKind>, u32) {
    let mut rng = rand::rng();

    let mut remaining = recipe.budget;
    let mut plan = Vec::new();
    let mut cone_used = 0u32;
    let mut bucket_used = 0u32;
    let mut spent = 0u32;

    // Flag leads huge waves (original: flag comes out with the wave) but
    // does not consume the horde's combat budget.
    if recipe.final_flag {
        plan.push(ZombieKind::Flag);
        spent += zombie_point_cost(ZombieKind::Flag);
    }

    while remaining >= POINT_NORMAL {
        let can_bucket = bucket_used < recipe.max_bucket && remaining >= POINT_BUCKET;
        let can_cone = cone_used < recipe.max_cone && remaining >= POINT_CONE;

        // Weighted, budget-aware candidate choice (rand 0.10 RngExt).
        let roll = rng.random_range(0..100);

        let kind = if can_bucket && roll < 12 {
            bucket_used += 1;
            ZombieKind::Buckethead
        } else if can_cone && roll < 40 {
            cone_used += 1;
            ZombieKind::Conehead
        } else {
            ZombieKind::Normal
        };

        let cost = zombie_point_cost(kind);

        if cost > remaining {
            // Shouldn't happen due to guards, but keep it safe.
            plan.push(ZombieKind::Normal);
            remaining -= POINT_NORMAL;
            spent += POINT_NORMAL;
        } else {
            plan.push(kind);
            remaining -= cost;
            spent += cost;
        }
    }

    (plan, spent)
}

/// Builds every wave plan for a level up front from
/// [`crate::game::level_flow::build_level_recipes`]. Returns the plans plus
/// the total point budget spent across all waves.
pub fn build_level_waves(adventure_level: u32) -> (Vec<Vec<ZombieKind>>, u32) {
    let recipes = crate::game::level_flow::build_level_recipes(adventure_level);
    let mut all = Vec::with_capacity(recipes.len());
    let mut total_spent = 0u32;

    for r in recipes {
        let (plan, spent) = build_wave_plan(r);
        total_spent += spent;
        all.push(plan);
    }

    (all, total_spent)
}

/// Wave director. Owns the pre-first-wave countdown and inter-wave gaps;
/// wave-to-wave advancement happens in [`advance_or_complete_level`] once a
/// wave's zombies are all gone.
pub fn start_wave_if_needed(frame_ticks: Res<FrameTicks>, mut wave: ResMut<WaveState>) {
    if frame_ticks.0 <= 0 || wave.cleared || wave.wave_plans.is_empty() {
        return;
    }

    // Active wave: spawning handled by spawn_wave_zombies; advancement by
    // advance_or_complete_level once the lawn is clear.
    if wave.started {
        return;
    }

    // Inter-wave gap countdown.
    if wave.between_waves {
        wave.inter_wave_remaining -= frame_ticks.0;
        if wave.inter_wave_remaining > 0 {
            return;
        }

        let next = wave.current_wave + 1;
        if next < wave.num_waves {
            begin_wave_index(&mut wave, next);
        } else {
            wave.between_waves = false;
        }
        return;
    }

    // Pre-first-wave countdown.
    wave.start_delay_remaining -= frame_ticks.0;
    if wave.start_delay_remaining > 0 {
        return;
    }
    begin_wave_index(&mut wave, 0);
}

fn begin_wave_index(wave: &mut WaveState, idx: u32) {
    wave.current_wave = idx;
    wave.started = true;
    wave.between_waves = false;
    wave.planned = wave
        .wave_plans
        .get(idx as usize)
        .cloned()
        .unwrap_or_default();
    wave.queued = wave.planned.len() as u32;
    wave.spawned = 0;
    wave.spawn_timer_remaining = 0;
    wave.huge_wave = wave.huge_flags.get(idx as usize).copied().unwrap_or(false);
}

pub fn spawn_wave_zombies(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut wave: ResMut<WaveState>,
) {
    if frame_ticks.0 <= 0 || !wave.started || wave.queued == 0 {
        return;
    }

    wave.spawn_timer_remaining -= frame_ticks.0;

    if wave.spawn_timer_remaining > 0 {
        return;
    }

    wave.spawn_timer_remaining += SPAWN_STAGGER_TICKS;

    let spawn_idx = wave.spawned as usize;
    let Some(kind) = wave.planned.get(spawn_idx).copied() else {
        wave.queued = 0;
        return;
    };

    let mut rng = rand::rng();
    let row = rng.random_range(0..LAWN_ROWS);

    let logic_x = 760.0;
    let logic_y = LAWN_YMIN + row as f32 * GRID_CELL_H + GRID_CELL_H * 0.5;

    commands.spawn((
        GameplayCleanup,
        Zombie::new(kind, row),
        Sprite {
            color: zombie_color(kind),
            custom_size: Some(Vec2::new(48.0, 72.0)),
            ..default()
        },
        Board::logic_to_world(Vec2::new(logic_x, logic_y), PLANT_Z - 1.0),
    ));

    wave.spawned += 1;
    wave.total_spawned_all += 1;
    wave.queued = wave.queued.saturating_sub(1);
}

/// Walks zombies left; stops and eats the plant blocking them in-row.
///
/// Chill handling: decrement duration inline (remove when expired), halve
/// movement while chilled, double the post-bite cooldown while chilled.
///
/// The `Without<>` filters are load-bearing: both queries touch `Transform`,
/// and without them Bevy panics at system-init ("conflicts with a previous
/// access") since nothing else proves a Plant entity is never a Zombie entity.
pub fn move_and_eat_zombies(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut board: ResMut<Board>,
    mut zombies: Query<
        (
            Entity,
            &mut Zombie,
            &mut Transform,
            Option<&mut Chilled>,
            Option<&mut Frozen>,
            Has<Dying>,
        ),
        Without<Plant>,
    >,
    mut plants: Query<
        (
            Entity,
            &mut Plant,
            &Transform,
            Option<&crate::game::specials::PotatoMineState>,
        ),
        Without<Zombie>,
    >,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let dt = frame_ticks.0 as f32 / TICK_HZ;

    for (z_e, mut zombie, mut z_tf, chilled_opt, frozen_opt, dying) in &mut zombies {
        commands.entity(z_e).remove::<Eating>();
        if dying {
            continue;
        }
        let chilled_now = if let Some(mut chilled) = chilled_opt {
            chilled.remaining_ticks = (chilled.remaining_ticks - frame_ticks.0).max(0);
            if chilled.remaining_ticks <= 0 {
                commands.entity(z_e).remove::<Chilled>();
                false
            } else {
                true
            }
        } else {
            false
        };

        // Frozen solid: no movement, no eating. Thaw converts into a
        // lingering chill.
        if let Some(mut frozen) = frozen_opt {
            frozen.remaining_ticks -= frame_ticks.0;
            if frozen.remaining_ticks <= 0 {
                let chill_after = frozen.chill_after_ticks;
                commands.entity(z_e).remove::<Frozen>();
                if chill_after > 0 {
                    commands.entity(z_e).insert(Chilled {
                        remaining_ticks: chill_after,
                    });
                }
            } else {
                continue;
            }
        }

        if zombie.hypnotized {
            // Walk right, off the lawn; combat is handled by
            // resolve_hypno_combat.
            z_tf.translation.x += zombie.speed_pps * dt;
            let logic = Board::world_to_logic(z_tf.translation.truncate());
            if logic.x > LAWN_XMIN + LAWN_COLS as f32 * GRID_CELL_W {
                commands.entity(z_e).try_despawn();
            }
            continue;
        }

        let z_x = Board::world_to_logic(z_tf.translation.truncate()).x;

        // Closest plant in this row at or left of the zombie's mouth.
        // Spikeweed is ground cover: walked over, never blocked or eaten.
        let mut blocking: Option<Entity> = None;
        for (p_e, plant, _p_tf, _mine) in &mut plants {
            if plant.row != zombie.row
                || plant.kind == crate::game::defs::PlantKind::Spikeweed
            {
                continue;
            }
            let p_x = Board::grid_center_logic(plant.col, plant.row).x;
            if p_x <= z_x + 6.0 && z_x - p_x <= ZOMBIE_EAT_REACH {
                blocking = Some(p_e);
                break;
            }
        }

        if let Some(p_e) = blocking {
            commands.entity(z_e).insert(crate::game::zombie_anim::Eating);
            zombie.eat_cooldown_remaining =
                (zombie.eat_cooldown_remaining - frame_ticks.0).max(0);

            if let Ok((plant_e, mut plant, _p_tf, mine_state)) = plants.get_mut(p_e) {
                // Armed Potato Mine explodes on contact before normal bite logic.
                if plant.kind == crate::game::defs::PlantKind::PotatoMine {
                    if let Some(state) = mine_state {
                        if state.armed {
                            if zombie.take_damage(crate::game::constants::POTATO_MINE_DAMAGE) {
                                crate::game::zombie_anim::kill_zombie(&mut commands, z_e);
                            }
                            board.cells[plant.row][plant.col] = None;
                            commands.entity(plant_e).try_despawn();
                            continue;
                        }
                    }
                }

                if zombie.eat_cooldown_remaining == 0 {
                    // Hypno-shroom: eater is converted, shroom consumed.
                    if plant.kind == crate::game::defs::PlantKind::HypnoShroom {
                        zombie.hypnotized = true;
                        zombie.eat_cooldown_remaining = ZOMBIE_EAT_INTERVAL_TICKS;
                        board.cells[plant.row][plant.col] = None;
                        commands.entity(plant_e).try_despawn();
                        continue;
                    }

                    // Garlic: bite diverts the zombie to an adjacent lane
                    // (random direction like the original, clamped to bounds).
                    if plant.kind == crate::game::defs::PlantKind::Garlic {
                        plant.hp -= ZOMBIE_EAT_DAMAGE;
                        zombie.eat_cooldown_remaining = ZOMBIE_EAT_INTERVAL_TICKS;

                        let mut rng = rand::rng();
                        let new_row = if rng.random_bool(0.5) {
                            zombie.row.saturating_sub(1)
                        } else {
                            (zombie.row + 1).min(LAWN_ROWS - 1)
                        };
                        // Degenerate when already at a boundary: go the only way.
                        let new_row = if new_row == zombie.row {
                            if zombie.row == 0 {
                                1
                            } else {
                                zombie.row - 1
                            }
                        } else {
                            new_row
                        };

                        zombie.row = new_row;
                        let wt = Board::logic_to_world(
                            Vec2::new(0.0, Board::row_center_y(new_row)),
                            0.0,
                        );
                        z_tf.translation.y = wt.translation.y;

                        if plant.hp <= 0 {
                            board.cells[plant.row][plant.col] = None;
                            commands.entity(plant_e).try_despawn();
                        }
                        continue;
                    }

                    plant.hp -= ZOMBIE_EAT_DAMAGE; // 50

                    zombie.eat_cooldown_remaining = if chilled_now {
                        ZOMBIE_EAT_INTERVAL_TICKS * 2 // 100 dps -> 50 dps while chilled
                    } else {
                        ZOMBIE_EAT_INTERVAL_TICKS // 0.5 s => 100 dps
                    };

                    if plant.hp <= 0 {
                        board.cells[plant.row][plant.col] = None;
                        commands.entity(plant_e).try_despawn();
                    }
                }
            }
        } else {
            let speed_mult = if chilled_now {
                crate::game::constants::CHILLED_SPEED_MULTIPLIER
            } else {
                1.0
            };
            zombie.eat_cooldown_remaining = 0;
            z_tf.translation.x -= zombie.speed_pps * speed_mult * dt;
        }
    }
}

/// A zombie reaching the house loses the game only if its row has no mower left:
/// an armed or mid-sweep mower counts as protection; a despawned (spent) mower
/// means the lane is wide open.
pub fn lose_on_house_reach(
    zombies: Query<(&Zombie, &Transform), Without<crate::game::zombie_anim::Dying>>,
    mowers: Query<&crate::game::mower::LawnMower>,
    mut overlay: ResMut<OverlayMenu>,
    mut paused: ResMut<Paused>,
) {
    if *overlay != OverlayMenu::None {
        return;
    }
    for (zombie, tf) in &zombies {
        let logic = Board::world_to_logic(tf.translation.truncate());
        if logic.x <= HOUSE_X_LOGIC {
            let row_protected = mowers.iter().any(|m| m.row == zombie.row);
            if !row_protected {
                *overlay = OverlayMenu::GameOver;
                paused.0 = true;
                break;
            }
        }
    }
}

/// Wave resolution: when the current wave is fully spawned and its zombies are
/// all gone, either open the inter-wave gap or complete the level.
///
/// Note: deferred despawns from earlier systems in the chain mean "all dead" can
/// be detected one frame late. Imperceptible in play; not worth apply_deferred.
/// Hypnotized zombies brawl their former allies: while in contact, both sides
/// bite at normal eat cadence (50 dmg / 0.5 s each) using the shared
/// `eat_cooldown_remaining` field. Contact pairs are snapshotted first to
/// avoid double-borrow issues.
pub fn resolve_hypno_combat(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform), Without<crate::game::zombie_anim::Dying>>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }

    struct Snap {
        e: Entity,
        hyp: bool,
        row: usize,
        x: f32,
    }
    let snap: Vec<Snap> = zombies
        .iter()
        .map(|(e, z, tf)| Snap {
            e,
            hyp: z.hypnotized,
            row: z.row,
            x: tf.translation.x,
        })
        .collect();

    // Each hypnotized zombie pairs with the nearest enemy in bite reach.
    let mut pairs: Vec<(Entity, Entity)> = Vec::new();
    for h in &snap {
        if !h.hyp {
            continue;
        }
        let mut best: Option<(Entity, f32)> = None;
        for v in &snap {
            if v.hyp || v.row != h.row {
                continue;
            }
            let d = (v.x - h.x).abs();
            if d <= ZOMBIE_EAT_REACH && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((v.e, d));
            }
        }
        if let Some((v_e, _)) = best {
            pairs.push((h.e, v_e));
        }
    }

    for (h_e, v_e) in pairs {
        hypno_bite(&mut zombies, &mut commands, h_e, v_e, frame_ticks.0);
        hypno_bite(&mut zombies, &mut commands, v_e, h_e, frame_ticks.0);
    }
}

/// One bite attempt by `att_e` on `vic_e` at standard eat cadence.
fn hypno_bite(
    zombies: &mut Query<(Entity, &mut Zombie, &Transform), Without<Dying>>,
    commands: &mut Commands,
    att_e: Entity,
    vic_e: Entity,
    frame_ticks: i32,
) {
    let Ok((_, mut att, _)) = zombies.get_mut(att_e) else {
        return;
    };
    att.eat_cooldown_remaining = (att.eat_cooldown_remaining - frame_ticks).max(0);
    if att.eat_cooldown_remaining > 0 {
        return;
    }
    att.eat_cooldown_remaining = ZOMBIE_EAT_INTERVAL_TICKS;

    let Ok((_, mut vic, _)) = zombies.get_mut(vic_e) else {
        return;
    };
    if vic.take_damage(ZOMBIE_EAT_DAMAGE) {
        crate::game::zombie_anim::kill_zombie(commands, vic_e);
    }
}

pub fn advance_or_complete_level(
    mut wave: ResMut<WaveState>,
    zombies: Query<(), With<Zombie>>,
    mut overlay: ResMut<OverlayMenu>,
    mut paused: ResMut<Paused>,
    bridge: Res<UiBridge>,
) {
    if wave.cleared || !wave.started || *overlay != OverlayMenu::None {
        return;
    }

    // Still spawning this wave.
    if wave.queued > 0 {
        return;
    }
    // Zombies still on the lawn.
    if !zombies.is_empty() {
        return;
    }

    let next = wave.current_wave + 1;
    if next >= wave.num_waves {
        wave.cleared = true;
        if let Ok(mut ui) = bridge.shared.lock() {
            ui.flags_done = wave.num_waves.max(1);
        }
        *overlay = OverlayMenu::LevelComplete;
        paused.0 = true;
        return;
    }

    // Inter-wave gap before the next wave.
    wave.started = false;
    wave.between_waves = true;
    wave.inter_wave_remaining = INTER_WAVE_DELAY_TICKS;
    wave.planned.clear();

    if let Ok(mut ui) = bridge.shared.lock() {
        ui.flags_done = next; // completed waves
        ui.flags_total = wave.num_waves.max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::constants::PEA_DAMAGE;

    /// Ledger: armor soaks first, overflow carries into the body.
    /// Counts every pea INCLUDING the killing one.
    fn peas_to_kill(kind: ZombieKind) -> u32 {
        let mut z = Zombie::new(kind, 0);
        let mut n = 0;
        loop {
            n += 1;
            assert!(n < 1000, "zombie never died");
            if z.take_damage(PEA_DAMAGE) {
                break;
            }
        }
        n
    }

    #[test]
    fn normal_takes_14_peas() {
        assert_eq!(peas_to_kill(ZombieKind::Normal), 14); // ceil(270/20)
    }

    #[test]
    fn conehead_takes_32_peas() {
        assert_eq!(peas_to_kill(ZombieKind::Conehead), 32); // ceil(640/20)
    }

    #[test]
    fn buckethead_takes_69_peas() {
        assert_eq!(peas_to_kill(ZombieKind::Buckethead), 69); // ceil(1370/20)
    }

    #[test]
    fn wiki_point_costs() {
        use crate::game::constants::{POINT_BUCKET, POINT_CONE, POINT_FLAG, POINT_NORMAL};
        assert_eq!(zombie_point_cost(ZombieKind::Normal), POINT_NORMAL);
        assert_eq!(zombie_point_cost(ZombieKind::Flag), POINT_FLAG);
        assert_eq!(zombie_point_cost(ZombieKind::Conehead), POINT_CONE);
        assert_eq!(zombie_point_cost(ZombieKind::Buckethead), POINT_BUCKET);
    }

    #[test]
    fn cherry_bomb_kills_every_kind_in_one_blast() {
        for kind in [ZombieKind::Normal, ZombieKind::Conehead, ZombieKind::Buckethead] {
            let mut z = Zombie::new(kind, 0);
            assert!(z.take_damage(super::super::super::game::constants::CHERRY_BOMB_DAMAGE));
        }
    }
}
