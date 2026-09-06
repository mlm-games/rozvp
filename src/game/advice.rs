//! Level advice: optional night intro, then click sun -> plant sunflower ->
//! zombies coming -> huge wave announcement -> done.

use bevy::prelude::*;

use crate::game::board::{Board, Stage};
use crate::game::constants::{ADVICE_HOLD_TICKS, HUGE_WAVE_ADVICE_TICKS};
use crate::game::defs::PlantKind;
use crate::game::systems::{Plant, SunStats};
use crate::game::tick::FrameTicks;
use crate::game::zombie::WaveState;
use crate::menus::UiBridge;

#[derive(Resource, Debug, Default)]
pub struct AdviceState {
    pub stage: AdviceStage,
    pub hold_remaining: i32,
    /// Set once the huge-wave line has been shown so it never repeats.
    pub huge_announced: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AdviceStage {
    NightIntro,
    #[default]
    ClickSun,
    PlantSunflower,
    ZombiesComing,
    HugeWave,
    Done,
}

/// Night levels open with the no-sky-sun warning before the tutorial chain;
/// Day levels start straight at ClickSun. Shared by `reset_advice` and
/// restart so both paths stay in sync.
pub fn apply_intro_stage(advice: &mut AdviceState, stage: Stage) {
    if stage == Stage::Night {
        advice.stage = AdviceStage::NightIntro;
        advice.hold_remaining = HUGE_WAVE_ADVICE_TICKS;
    }
}

pub fn reset_advice(mut advice: ResMut<AdviceState>, board: Res<Board>) {
    *advice = AdviceState::default();
    apply_intro_stage(&mut advice, board.stage);
}

/// Stage transitions use explicit facts (a real collection counter, an actual
/// Sunflower) so spending sun or planting two peashooters can't skip beats.
pub fn tick_advice(
    frame_ticks: Res<FrameTicks>,
    mut advice: ResMut<AdviceState>,
    stats: Res<SunStats>,
    plants: Query<&Plant>,
    wave: Res<WaveState>,
    bridge: Res<UiBridge>,
) {
    // Huge-wave announcement preempts whatever stage is playing (once).
    if wave.huge_wave && !advice.huge_announced {
        advice.stage = AdviceStage::HugeWave;
        advice.hold_remaining = HUGE_WAVE_ADVICE_TICKS;
        advice.huge_announced = true;
    }

    match advice.stage {
        AdviceStage::NightIntro => {
            advice.hold_remaining -= frame_ticks.0.max(0);
            if advice.hold_remaining <= 0 {
                advice.stage = AdviceStage::ClickSun;
            }
        }
        AdviceStage::ClickSun => {
            if stats.collected_total > 0 {
                advice.stage = AdviceStage::PlantSunflower;
            }
        }
        AdviceStage::PlantSunflower => {
            if plants.iter().any(|p| p.kind == PlantKind::Sunflower) {
                advice.stage = AdviceStage::ZombiesComing;
                advice.hold_remaining = ADVICE_HOLD_TICKS;
            }
        }
        AdviceStage::ZombiesComing => {
            if wave.started {
                advice.hold_remaining -= frame_ticks.0.max(0);
                if advice.hold_remaining <= 0 {
                    advice.stage = AdviceStage::Done;
                }
            }
        }
        AdviceStage::HugeWave => {
            advice.hold_remaining -= frame_ticks.0.max(0);
            if advice.hold_remaining <= 0 {
                advice.stage = AdviceStage::Done;
            }
        }
        AdviceStage::Done => {}
    }

    // Published as FTL keys; views translate at render time so a
    // mid-level language switch re-renders without re-ticking.
    let (text, visible) = match advice.stage {
        AdviceStage::NightIntro => ("advice-night-intro", true),
        AdviceStage::ClickSun => ("advice-click-sun", true),
        AdviceStage::PlantSunflower => ("advice-plant-sunflower", true),
        AdviceStage::ZombiesComing => ("advice-zombies-coming", true),
        AdviceStage::HugeWave => ("advice-huge-wave", true),
        AdviceStage::Done => ("", false),
    };
    if let Ok(mut ui) = bridge.shared.lock() {
        ui.advice.text = text.to_string();
        ui.advice.visible = visible;
    }
}
