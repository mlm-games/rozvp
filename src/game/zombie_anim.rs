//! Zombie sprite animation: bakes the renamite rig (`tools/pvz-gen`) to a
//! sprite atlas and drives walk/eat/fall states from gameplay markers.
//!
//! State mapping: `Eating` marker (set by [`crate::game::zombie::move_and_eat_zombies`])
//! -> eat loop; `Dying` -> fall one-shot then despawn; otherwise walk loop.
//! The rig faces right, so non-hypnotized zombies (walking left) flip X.

use bevy::prelude::*;

use crate::game::constants::{TICK_HZ, TICK};
use crate::game::systems::Plant;
use crate::game::tick::FrameTicks;
use crate::game::zombie::Zombie;

pub const ATLAS_COLS: u32 = 8;
pub const ATLAS_ROWS: u32 = 9;
pub const FRAME_W: u32 = 128;
pub const FRAME_H: u32 = 160;
pub const WALK_START: usize = 0;
pub const WALK_COUNT: usize = 30;
pub const EAT_START: usize = 30;
pub const EAT_COUNT: usize = 15;
pub const FALL_START: usize = 45;
pub const FALL_COUNT: usize = 24;
pub const ANIM_FPS: f32 = 30.0;

/// Displayed size in world units (cell is 80x100).
const DISPLAY_W: f32 = 64.0;
const DISPLAY_H: f32 = 80.0;

/// Fall clip length (24 frames @ 30 fps) in 100 Hz ticks.
const DYING_TICKS: i32 = (FALL_COUNT as f32 / ANIM_FPS * TICK_HZ).round() as i32;

#[derive(Resource)]
pub struct ZombieSheet {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimState {
    Walk,
    Eat,
    Fall,
}

impl AnimState {
    fn range(self) -> (usize, usize) {
        match self {
            AnimState::Walk => (WALK_START, WALK_COUNT),
            AnimState::Eat => (EAT_START, EAT_COUNT),
            AnimState::Fall => (FALL_START, FALL_COUNT),
        }
    }
}

#[derive(Component)]
pub struct ZombieAnim {
    state: AnimState,
    frame: usize,
    acc: f32,
}

/// Set while a zombie is blocked by a plant and biting it.
#[derive(Component)]
pub struct Eating;

/// Corpse playing the fall one-shot; despawned when the timer runs out.
#[derive(Component)]
pub struct Dying {
    pub remaining_ticks: i32,
}

/// Convert a lethal damage kill into the fall animation. Explosive/devour
/// kills keep their instant despawns (vanishing in the blast reads correctly).
pub fn kill_zombie(commands: &mut Commands, e: Entity) {
    commands
        .entity(e)
        .insert(Dying {
            remaining_ticks: DYING_TICKS,
        })
        .remove::<Eating>();
}

pub struct ZombieAnimPlugin;

impl Plugin for ZombieAnimPlugin {
    fn build(&self, app: &mut App) {
        let image = app
            .world()
            .resource::<AssetServer>()
            .load("anims/zombie_atlas.png");
        let layout = app
            .world_mut()
            .resource_mut::<Assets<TextureAtlasLayout>>()
            .add(TextureAtlasLayout::from_grid(
                UVec2::new(FRAME_W, FRAME_H),
                ATLAS_COLS,
                ATLAS_ROWS,
                None,
                None,
            ));
        app.insert_resource(ZombieSheet { image, layout });
    }
}

/// Swap placeholder rectangles for atlas sprites once the sheet is loaded.
/// Runs every frame until converted; new spawns convert here too.
pub fn apply_zombie_sheet(
    sheet: Res<ZombieSheet>,
    server: Res<AssetServer>,
    mut commands: Commands,
    mut announced: Local<bool>,
    mut zombies: Query<(Entity, &mut Sprite), (With<Zombie>, Without<ZombieAnim>, Without<Plant>)>,
) {
    if let bevy::asset::LoadState::Failed(err) = server.load_state(&sheet.image)
        && !*announced
    {
        *announced = true;
        error!("zombie atlas failed to load: {err}");
    }
    if !matches!(
        server.load_state(&sheet.image),
        bevy::asset::LoadState::Loaded
    ) {
        return;
    }
    for (e, mut sprite) in &mut zombies {
        let tint = sprite.color;
        *sprite = Sprite::from_atlas_image(
            sheet.image.clone(),
            TextureAtlas {
                layout: sheet.layout.clone(),
                index: WALK_START,
            },
        );
        sprite.color = tint;
        sprite.custom_size = Some(Vec2::new(DISPLAY_W, DISPLAY_H));
        commands
            .entity(e)
            .insert(ZombieAnim {
                state: AnimState::Walk,
                frame: 0,
                acc: 0.0,
            });
    }
}

/// Select the clip from gameplay state and orient the sprite.
pub fn sync_zombie_anim(
    mut zombies: Query<
        (&Zombie, &mut ZombieAnim, &mut Sprite, Has<Eating>, Has<Dying>),
        Without<Plant>,
    >,
) {
    for (zombie, mut anim, mut sprite, eating, dying) in &mut zombies {
        let target = if dying {
            AnimState::Fall
        } else if eating {
            AnimState::Eat
        } else {
            AnimState::Walk
        };
        if anim.state != target {
            anim.state = target;
            anim.frame = 0;
            anim.acc = 0.0;
        }
        sprite.flip_x = !zombie.hypnotized;
    }
}

/// Advance frame counters on the 100 Hz tick; expire corpses.
pub fn step_zombie_anim(
    mut commands: Commands,
    frame_ticks: Res<FrameTicks>,
    mut zombies: Query<(Entity, &mut ZombieAnim, &mut Sprite, Option<&mut Dying>)>,
) {
    if frame_ticks.0 <= 0 {
        return;
    }
    let advance = frame_ticks.0 as f32 * ANIM_FPS * TICK;

    for (e, mut anim, mut sprite, dying) in &mut zombies {
        if let Some(mut d) = dying {
            d.remaining_ticks -= frame_ticks.0;
            if d.remaining_ticks <= 0 {
                commands.entity(e).try_despawn();
                continue;
            }
        }

        anim.acc += advance;
        while anim.acc >= 1.0 {
            anim.acc -= 1.0;
            anim.frame = match anim.state {
                AnimState::Fall => (anim.frame + 1).min(FALL_COUNT - 1),
                other => (anim.frame + 1) % other.range().1,
            };
        }

        let (start, _) = anim.state.range();
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = start + anim.frame;
        }
    }
}
