use crate::bottles::BottleLayout;

use crate::{
    bottles::Bottle,
    constants::BottleColor,
    solver::{
        discovery::{
            DiscoverResult, count_total_mystery_colors, find_best_discovery_moves,
            improve_best_revealed_state,
        },
        run_solver,
    },
};

fn get_layout_from_bottle_count(count: usize) -> BottleLayout {
    for ele in BottleLayout::get_layouts() {
        if ele.bottle_count() == count {
            return ele;
        }
    }

    panic!("No layout found for bottle count: {}", count);
}

macro_rules! create_test_level {
    ($level:literal, $bottles:expr) => {
        create_test_level!(false, $level, $bottles);
    };
    (NO_SOLVE, $level:literal, $bottles:expr) => {
        create_test_level!(true, $level, $bottles);
    };
    ($no_solve:expr, $level:literal, $bottles:expr) => {
        paste::paste! {
            mod [<level_ $level>] {
                use super::*;
                lazy_static::lazy_static! {
                    static ref PARSED_BOTTLES: Vec<crate::bottles::Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($bottles);
                }

                #[test]
                fn solve() {
                    if $no_solve {
                        return;
                    }

                    use crate::bottles::Bottle;
                    let mut bottles_parsed: Vec<Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($bottles);

                    let solution = crate::solver::run_solver(&bottles_parsed).expect("No solution found");

                    for m in solution {
                        println!("Move from bottle {} to bottle {}", m.source_index(), m.destination_index());
                        m.perform_move_on_bottles(&mut bottles_parsed);
                    }
                }

                #[test]
                fn layout_detection() {
                    let image = match crate::bottles::test_utils::TestUtils::load_test_image(&format!("level-{}.png", $level)) {
                        Ok(img) => img,
                        Err(_) => {
                            println!("Warning: Could not load level-{}.png, skipping test", $level);
                            return;
                        }
                    };

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    let detected_layout = crate::bottles::BottleLayout::detect_layout(&image)
                        .expect("Failed to detect bottle layout");

                    assert_eq!(detected_layout, expected_layout, "Detected layout does not match expected layout for level {}", $level);
                }

                #[test]
                fn bottle_detection() {
                    let image = match crate::bottles::test_utils::TestUtils::load_test_image(&format!("level-{}.png", $level)) {
                        Ok(img) => img,
                        Err(_) => {
                            println!("Warning: Could not load level-{}.png, skipping test", $level);
                            return;
                        }
                    };

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    let detected_bottles = crate::bottles::test_utils::TestUtils::detect_bottles_from_image(&image, &expected_layout)
                        .expect("Failed to detect bottles from image");

                    assert_eq!(detected_bottles.len(), PARSED_BOTTLES.len(), "Detected bottle count does not match expected for level {}", $level);

                    for (idx, (detected, expected)) in detected_bottles.iter().zip(PARSED_BOTTLES.iter()).enumerate() {
                        assert_eq!(
                            detected.get_fills(),
                            expected.get_fills(),
                            "Bottle {} does not match expected. Detected: {:?}, Expected: {:?}",
                            idx,
                            detected.get_fills(),
                            expected.get_fills()
                        );
                    }
                }
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! create_generated_test_level {
    ($module_suffix:literal, $capture_id:expr, $image_filename:expr, $initial_bottles:expr, $resolved_bottles:expr) => {
        paste::paste! {
            mod [<captured_level_ $module_suffix>] {
                use super::*;
                lazy_static::lazy_static! {
                    static ref PARSED_BOTTLES: Vec<crate::bottles::Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($initial_bottles);
                    static ref RESOLVED_BOTTLES: Vec<crate::bottles::Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($resolved_bottles);
                }

                #[test]
                fn discovery_reveal_and_solve() {
                    let final_revealed = run_discovery_simulation(PARSED_BOTTLES.as_slice(), RESOLVED_BOTTLES.as_slice());
                    assert_eq!(
                        count_total_mystery_colors(&final_revealed),
                        0,
                        "Discovery simulation should reveal all mystery colors for captured level {}",
                        $capture_id
                    );
                    assert_eq!(
                        final_revealed,
                        RESOLVED_BOTTLES.as_slice(),
                        "Discovery simulation should match the captured resolved state for level {}",
                        $capture_id
                    );

                    solve_and_assert(final_revealed);
                }

                #[test]
                fn layout_detection() {
                    let image = match crate::bottles::test_utils::TestUtils::load_test_image($image_filename) {
                        Ok(img) => img,
                        Err(_) => {
                            println!("Warning: Could not load {}, skipping test", $image_filename);
                            return;
                        }
                    };

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    let detected_layout = crate::bottles::BottleLayout::detect_layout(&image)
                        .expect("Failed to detect bottle layout");

                    assert_eq!(detected_layout, expected_layout, "Detected layout does not match expected layout for captured level {}", $capture_id);
                }

                #[test]
                fn bottle_detection() {
                    let image = match crate::bottles::test_utils::TestUtils::load_test_image($image_filename) {
                        Ok(img) => img,
                        Err(_) => {
                            println!("Warning: Could not load {}, skipping test", $image_filename);
                            return;
                        }
                    };

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    println!("Expected layout for captured level {}: {:?}", $capture_id, expected_layout.name);
                    let detected_bottles = crate::bottles::test_utils::TestUtils::detect_bottles_from_image(&image, &expected_layout)
                        .expect("Failed to detect bottles from image");

                    assert_eq!(detected_bottles.len(), PARSED_BOTTLES.len(), "Detected bottle count does not match expected for captured level {}", $capture_id);

                    for (idx, (detected, expected)) in detected_bottles.iter().zip(PARSED_BOTTLES.iter()).enumerate() {
                        assert_eq!(
                            detected.get_fills(),
                            expected.get_fills(),
                            "Bottle {} does not match expected. Detected: {:?}, Expected: {:?}",
                            idx,
                            detected.get_fills(),
                            expected.get_fills()
                        );
                    }
                }
            }
        }
    };
}

#[allow(dead_code)]
fn solve_and_assert(mut bottles: Vec<Bottle>) {
    let solution = run_solver(&bottles).expect("No solution found after discovery");
    for mv in solution {
        mv.perform_move_on_bottles(&mut bottles);
    }

    assert!(
        bottles.iter().all(|b| b.is_solved() || b.is_empty()),
        "Final bottle state should be solved"
    );
}

fn run_discovery_simulation(initial: &[Bottle], resolved: &[Bottle]) -> Vec<Bottle> {
    let mut max_revealed = initial.to_vec();

    for _ in 0..300 {
        if count_total_mystery_colors(&max_revealed) == 0 {
            break;
        }

        let mut current_moves = Vec::new();
        let mut current_state = initial.to_vec();
        match find_best_discovery_moves(&current_state, &max_revealed) {
            DiscoverResult::MoveToDiscover(moves_to_apply) => {
                if moves_to_apply.is_empty() {
                    break;
                }

                for mv in moves_to_apply {
                    let previous_state = current_state.clone();
                    mv.perform_move_on_bottles(&mut current_state);
                    current_moves.push(mv);

                    reveal_observed(&mut current_state, resolved);
                    improve_best_revealed_state(
                        &mut max_revealed,
                        &previous_state,
                        &current_state,
                    );
                }
            }
            DiscoverResult::NoMove => {
                println!("No more discovery moves found, simulating restart...");
            }
            DiscoverResult::AlreadySolved => {
                break;
            }
        }
    }

    max_revealed
}

#[allow(dead_code)]
fn reveal_observed(current: &mut [Bottle], fully_resolved: &[Bottle]) {
    current
        .iter_mut()
        .zip(fully_resolved.iter())
        .for_each(|(current_bottle, resolved_bottle)| {
            let observed = current_bottle.get_fills_mut();
            let resolved = resolved_bottle.get_fills();

            // Simulate game reveal: only mystery colors currently exposed on top become known.
            let mut index = observed.len();
            while index > 0 {
                let fill_index = index - 1;
                if observed[fill_index] != BottleColor::Mystery {
                    break;
                }

                observed[fill_index] = resolved[fill_index];
                index -= 1;
            }
        });
}

create_test_level!(213, "YRGM BPWO OBPG ROPM YRPW MWBG BGRY YMWO EEEE EEEE");
create_test_level!(
    214,
    "POGR LMOR GYPO GYGB WBRL MLRY WMLP POMW BWBY EEEE EEEE"
);
create_test_level!(
    NO_SOLVE,
    215,
    "W??? B??? G??? P??? O??? G??? g??? O??? O??? L??? EEEE EEEE"
);

include!(concat!(env!("OUT_DIR"), "/generated_level_tests.rs"));
