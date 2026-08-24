//! Adventure level progression + seed unlock gating.
//!
//! `adventure_level` convention: 0-based index into the 1-x grid,
//! i.e. 0 => level "1-1", 1 => "1-2", 10 => "2-1".

use crate::app::SharedUi;
use crate::game::board::Stage;
use crate::game::constants::LEVELS_PER_AREA;
use crate::game::zombie::WaveRecipe;

pub fn level_label(adventure_level: u32) -> String {
    let area = adventure_level / LEVELS_PER_AREA + 1;
    let stage = adventure_level % LEVELS_PER_AREA + 1;
    format!("{area}-{stage}")
}

/// Day for area 1, Night from area 2 on (until pool/fog/roof areas exist).
pub fn stage_for_level(adventure_level: u32) -> Stage {
    match adventure_level / LEVELS_PER_AREA {
        0 => Stage::Day,
        _ => Stage::Night,
    }
}

/// Current implemented unlock path.
/// TODO(D): replace with exact 0.9.9 adventure unlock table.
pub fn starting_unlocked_seeds() -> Vec<String> {
    vec!["Sunflower".to_string(), "Peashooter".to_string()]
}

/// Award seed for completing a level.
///
/// Intentionally only implemented plants. TODO(D): exact PvZ order
/// (1-1 Peashooter, 1-2 Sunflower, 1-3 Cherry Bomb, ...) once those
/// levels exist; RoZVP starts with Pea+Sunflower for the playable slice,
/// so implemented additions are awarded next.
pub fn award_for_completed_level(completed_level: u32) -> Option<&'static str> {
    match completed_level {
        0 => Some("Wall-nut"),
        1 => Some("Cherry Bomb"),
        2 => Some("Potato Mine"),
        3 => Some("Snow Pea"),
        4 => Some("Chomper"),
        5 => Some("Repeater"),
        6 => Some("Squash"),
        7 => Some("Threepeater"),
        8 => Some("Tall-nut"),
        9 => Some("Garlic"),
        10 => Some("Spikeweed"),
        11 => Some("Torchwood"),
        12 => Some("Hypno-shroom"),
        13 => Some("Ice-shroom"),
        14 => Some("Doom-shroom"),
        15 => Some("Jalapeno"),
        _ => None,
    }
}

pub fn ensure_seed_unlocked(ui: &mut SharedUi, seed: &str) {
    if !ui.unlocked_seed_names.iter().any(|s| s == seed) {
        ui.unlocked_seed_names.push(seed.to_string());
    }
}

/// How many waves before the level is cleared.
/// Early levels in an area are short; later ones get a full flag cycle.
/// TODO(D): replace with exact decomp data when porting gZombieWaves.
pub fn level_wave_count(adventure_level: u32) -> u32 {
    match adventure_level % LEVELS_PER_AREA {
        0 => 4,
        1 => 6,
        2 => 8,
        _ => 10,
    }
}

/// Point budget for wave index `w` (0-based) on this adventure level.
/// Pattern mirrors community Day curves: slow ramp, then a flag spike.
pub fn wave_point_budget(adventure_level: u32, wave_index: u32) -> u32 {
    let n = level_wave_count(adventure_level);
    let last = n.saturating_sub(1);

    // Final wave = huge wave budget.
    if wave_index == last && n >= 6 {
        return if adventure_level == 0 { 4 } else { 10 };
    }

    // 1-1: tiny waves.
    if adventure_level == 0 {
        return match wave_index {
            0 => 1,
            1 => 1,
            2 => 2,
            _ => 3,
        };
    }

    // Generic early Day ramp (capped).
    let base = match wave_index {
        0 => 1,
        1 => 1,
        2 => 1,
        3 => 2,
        4 => 2,
        5 => 2,
        6 => 3,
        7 => 3,
        8 => 3,
        _ => 4,
    };

    // Slight level scaling after 1-3.
    let scale = 1 + (adventure_level.saturating_sub(2) / 3);
    base * scale
}

pub fn wave_is_huge(adventure_level: u32, wave_index: u32) -> bool {
    let n = level_wave_count(adventure_level);
    n >= 6 && wave_index + 1 == n
}

/// Which armored types may appear on this level.
pub fn level_allows_cone(adventure_level: u32) -> bool {
    adventure_level >= 1 // 1-2+
}

pub fn level_allows_bucket(adventure_level: u32) -> bool {
    adventure_level >= 3 // 1-4+
}

/// Build recipes for every wave of the level (used by enter_level).
pub fn build_level_recipes(adventure_level: u32) -> Vec<WaveRecipe> {
    let n = level_wave_count(adventure_level);
    (0..n)
        .map(|w| WaveRecipe {
            budget: wave_point_budget(adventure_level, w),
            max_cone: if level_allows_cone(adventure_level) {
                if wave_is_huge(adventure_level, w) {
                    4
                } else {
                    2
                }
            } else {
                0
            },
            max_bucket: if level_allows_bucket(adventure_level) {
                if wave_is_huge(adventure_level, w) {
                    2
                } else {
                    1
                }
            } else {
                0
            },
            final_flag: wave_is_huge(adventure_level, w),
            huge: wave_is_huge(adventure_level, w),
        })
        .collect()
}

pub fn advance_after_level_complete(ui: &mut SharedUi) {
    ui.adventure_level += 1;
    ui.level_name = level_label(ui.adventure_level);
}

/// Keeps the displayed label correct and the active bank free of locked seeds
/// (while guaranteeing at least one playable pair).
pub fn normalize_progress_ui(ui: &mut SharedUi) {
    if ui.unlocked_seed_names.is_empty() {
        ui.unlocked_seed_names = starting_unlocked_seeds();
    }

    ui.level_name = level_label(ui.adventure_level);

    ui.seed_bank
        .retain(|s| ui.unlocked_seed_names.iter().any(|u| u == &s.seed_name));

    if ui.seed_bank.is_empty() {
        for name in starting_unlocked_seeds() {
            if ui.unlocked_seed_names.iter().any(|u| u == &name) {
                ui.seed_bank.push(crate::app::SeedSlotUi {
                    cost: crate::game::defs::seed_def(&name)
                        .map(|d| d.cost)
                        .unwrap_or(100),
                    seed_name: name,
                    ready: 1.0,
                    affordable: true,
                    selected: false,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::SeedSlotUi;

    #[test]
    fn labels_match_grid() {
        assert_eq!(level_label(0), "1-1");
        assert_eq!(level_label(4), "1-5");
        assert_eq!(level_label(9), "1-10");
        assert_eq!(level_label(10), "2-1");
    }

    #[test]
    fn stage_for_level_maps_area_2_to_night() {
        assert_eq!(stage_for_level(0), Stage::Day);
        assert_eq!(stage_for_level(9), Stage::Day);
        assert_eq!(stage_for_level(10), Stage::Night);
        assert_eq!(stage_for_level(19), Stage::Night);
    }

    #[test]
    fn award_order_covers_implemented_plants_then_stops() {
        assert_eq!(award_for_completed_level(0), Some("Wall-nut"));
        assert_eq!(award_for_completed_level(1), Some("Cherry Bomb"));
        assert_eq!(award_for_completed_level(2), Some("Potato Mine"));
        assert_eq!(award_for_completed_level(3), Some("Snow Pea"));
        assert_eq!(award_for_completed_level(4), Some("Chomper"));
        assert_eq!(award_for_completed_level(5), Some("Repeater"));
        // Batch 2.
        assert_eq!(award_for_completed_level(6), Some("Squash"));
        assert_eq!(award_for_completed_level(7), Some("Threepeater"));
        assert_eq!(award_for_completed_level(8), Some("Tall-nut"));
        assert_eq!(award_for_completed_level(9), Some("Garlic"));
        assert_eq!(award_for_completed_level(10), Some("Spikeweed"));
        assert_eq!(award_for_completed_level(11), Some("Torchwood"));
        assert_eq!(award_for_completed_level(12), Some("Hypno-shroom"));
        assert_eq!(award_for_completed_level(13), Some("Ice-shroom"));
        assert_eq!(award_for_completed_level(14), Some("Doom-shroom"));
        assert_eq!(award_for_completed_level(15), Some("Jalapeno"));
        assert_eq!(award_for_completed_level(16), None);
    }

    #[test]
    fn normalize_prunes_locked_bank_and_keeps_a_pair() {
        let mut ui = SharedUi::default();
        ui.unlocked_seed_names = vec!["Sunflower".into()];
        ui.seed_bank = vec![
            SeedSlotUi {
                seed_name: "Peashooter".into(),
                ..Default::default()
            },
            SeedSlotUi {
                seed_name: "Sunflower".into(),
                ..Default::default()
            },
        ];
        normalize_progress_ui(&mut ui);
        assert!(ui.seed_bank.iter().all(|s| s.seed_name == "Sunflower"));
    }

    #[test]
    fn one_one_waves_are_short_normals_only() {
        assert_eq!(level_wave_count(0), 4);
        assert_eq!(
            (0..4).map(|w| wave_point_budget(0, w)).collect::<Vec<_>>(),
            vec![1, 1, 2, 3]
        );

        // No armor types and no huge flag wave on the tutorial level.
        assert!(!level_allows_cone(0));
        assert!(!level_allows_bucket(0));
        let recipes = build_level_recipes(0);
        assert!(
            recipes
                .iter()
                .all(|r| !r.huge && r.max_cone == 0 && r.max_bucket == 0)
        );
    }

    #[test]
    fn longer_levels_ramp_and_end_in_a_huge_wave() {
        assert_eq!(level_wave_count(1), 6);
        assert!(!wave_is_huge(1, 0));
        assert!(wave_is_huge(1, 5)); // final wave of 6
        assert_eq!(wave_point_budget(1, 5), 10); // huge-wave budget
        assert!(level_allows_cone(1));
        assert!(!level_allows_bucket(1));

        // Buckets open up from 1-4; full flag cycle from 1-4..1-6.
        assert_eq!(level_wave_count(3), 10);
        assert!(level_allows_bucket(3));
        assert!(wave_is_huge(3, 9));
    }
}
