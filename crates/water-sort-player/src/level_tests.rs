use std::collections::HashSet;

use water_sort_solver::build_solver_initial_bottle_state;
use water_sort_solver::discovery::improve_current_bottles_with_revealed_state;

use crate::bottles::BottleLayout;

use crate::{
    bottles::Bottle,
    constants::BottleColor,
    solver::{
        discovery::{
            DiscoverResult, count_hidden_bottles, count_total_mystery_colors,
            find_best_discovery_moves, find_best_hidden_unlock_moves, improve_best_revealed_state,
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
                macro_rules! load_level_image {
                    () => {{
                        match crate::bottles::test_utils::TestUtils::load_test_image(&format!("level-{}.png", $level)) {
                            Ok(img) => img,
                            Err(_) => {
                                println!("Warning: Could not load level-{}.png, skipping test", $level);
                                return;
                            }
                        }
                    }};
                }

                lazy_static::lazy_static! {
                    static ref PARSED_BOTTLES: Vec<crate::bottles::Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($bottles);
                }

                #[test_log::test]
                fn solve() {
                    if $no_solve {
                        return;
                    }

                    use crate::bottles::Bottle;
                    let mut bottles_parsed: Vec<Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($bottles);

                    let solution = crate::solver::run_solver(&bottles_parsed, &bottles_parsed).expect("No solution found");

                    for m in solution {
                        println!("Move from bottle {} to bottle {}", m.source_index(), m.destination_index());
                        m.perform_move_on_bottles(&mut bottles_parsed);
                    }
                }

                #[test_log::test]
                fn layout_detection() {
                    let image = load_level_image!();

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    let detected_layout = crate::bottles::BottleLayout::detect_layout(&image)
                        .expect("Failed to detect bottle layout");

                    assert_eq!(detected_layout, expected_layout, "Detected layout does not match expected layout for level {}", $level);
                }

                #[test_log::test]
                fn bottle_detection() {
                    let image = load_level_image!();

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    let detected_bottles = crate::bottles::test_utils::TestUtils::detect_bottles_from_image(
                        &image,
                        &expected_layout,
                        &format!("level-{}-bottle-detection", $level),
                    )
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
    ($capture_id:literal, $image_filename:expr, $initial_bottles:expr, $resolved_bottles:expr) => {
        paste::paste! {
            mod [<captured_level_ $capture_id>] {
                use super::*;
                macro_rules! load_capture_image {
                    () => {{
                        match crate::bottles::test_utils::TestUtils::load_test_image($image_filename) {
                            Ok(img) => img,
                            Err(_) => {
                                println!("Warning: Could not load {}, skipping test", $image_filename);
                                return;
                            }
                        }
                    }};
                }

                lazy_static::lazy_static! {
                    static ref PARSED_BOTTLES: Vec<crate::bottles::Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($initial_bottles);
                    static ref RESOLVED_BOTTLES: Vec<crate::bottles::Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($resolved_bottles);
                }

                #[test_log::test]
                fn discovery_reveal_and_solve() {
                    let initial_mystery_count = count_total_mystery_colors(PARSED_BOTTLES.as_slice());
                    let final_revealed = run_discovery_simulation(PARSED_BOTTLES.as_slice(), RESOLVED_BOTTLES.as_slice());

                    // Hidden-only captures do not need discovery-validation assertions.
                    if initial_mystery_count == 0 {
                        return;
                    }

                    let are_equal = crate::bottles::test_utils::TestUtils::are_bottles_equal(final_revealed.as_slice(), RESOLVED_BOTTLES.as_slice());
                    assert!(
                        are_equal,
                        "Final revealed bottles do not match expected resolved bottles for captured level {}. Final revealed: {}, Expected resolved: {}",
                        $capture_id,
                        final_revealed.iter().map(|b| b.to_string()).collect::<Vec<String>>().join(" "),
                        RESOLVED_BOTTLES.iter().map(|b| b.to_string()).collect::<Vec<String>>().join(" ")
                    );

                    solve_and_assert(&final_revealed, PARSED_BOTTLES.as_slice());
                }

                #[test_log::test]
                fn run_solver() {
                    solve_and_assert(RESOLVED_BOTTLES.as_slice(), PARSED_BOTTLES.as_slice());
                }

                #[test_log::test]
                fn invalid_level_test() {
                    let is_valid = water_sort_capture::is_level_valid(PARSED_BOTTLES.as_slice(), RESOLVED_BOTTLES.as_slice());
                    assert!(is_valid, "Level should be valid for captured level {}", $capture_id);
                }

                #[test_log::test]
                fn layout_detection() {
                    let image = load_capture_image!();

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    let detected_layout = crate::bottles::BottleLayout::detect_layout(&image)
                        .expect("Failed to detect bottle layout");

                    assert_eq!(detected_layout, expected_layout, "Detected layout does not match expected layout for captured level {}", $capture_id);
                }

                #[test_log::test]
                fn bottle_detection() {
                    let image = load_capture_image!();

                    let expected_layout = get_layout_from_bottle_count(PARSED_BOTTLES.len());
                    println!("Expected layout for captured level {}: {:?}", $capture_id, expected_layout.name);
                    let detected_bottles = crate::bottles::test_utils::TestUtils::detect_bottles_from_image(
                        &image,
                        &expected_layout,
                        &format!("captured-level-{}-bottle-detection", $capture_id),
                    )
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
fn solve_and_assert(max_revealed_bottles: &[Bottle], initial_bottles: &[Bottle]) {
    let mut current_bottles =
        build_solver_initial_bottle_state(max_revealed_bottles, initial_bottles);

    log::debug!(
        "Running solver on final revealed state: {}",
        max_revealed_bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    );
    log::debug!(
        "Initial state for solver: {}",
        initial_bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    );

    let solution = run_solver(max_revealed_bottles, initial_bottles)
        .expect("No solution found after discovery");
    println!(
        "Solution moves: {}",
        solution
            .iter()
            .map(|m| format!("{}->{}", m.source_index(), m.destination_index()))
            .collect::<Vec<String>>()
            .join(", ")
    );
    for mv in solution {
        assert!(
            mv.can_perform_on_bottles(&current_bottles),
            "Solver produced an invalid replay move {}->{} on state {}",
            mv.source_index(),
            mv.destination_index(),
            current_bottles
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<String>>()
                .join(" ")
        );

        mv.perform_move_on_bottles(&mut current_bottles);
    }

    assert!(
        current_bottles
            .iter()
            .all(|b| b.is_solved() || (b.is_empty() && !b.is_hidden_and_locked())),
        "Final bottle state should be solved after replay. Final state: {}",
        current_bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    );
}

fn run_discovery_simulation(initial: &[Bottle], resolved: &[Bottle]) -> Vec<Bottle> {
    let mut max_revealed = initial.to_vec();

    let mut current_state = initial.to_vec();
    for _ in 0..300 {
        let hidden_count = count_hidden_bottles(&max_revealed);
        let mystery_count = count_total_mystery_colors(&max_revealed);
        if hidden_count == 0 && mystery_count == 0 {
            break;
        }

        reveal_hidden_observed(&mut current_state, resolved);
        improve_revealed_hidden_bottles(&mut max_revealed, &current_state);

        improve_current_bottles_with_revealed_state(&mut current_state, &max_revealed);
        if count_total_mystery_colors(&current_state) > 0 {
            match find_best_discovery_moves(&current_state, &max_revealed) {
                DiscoverResult::MoveToDiscover(moves_to_apply) => {
                    if moves_to_apply.is_empty() {
                        panic!("Moves should not be empty");
                    }

                    for mv in moves_to_apply {
                        if !mv.can_perform_on_bottles(&current_state) {
                            panic!(
                                "Discovery move became invalid during simulation ({}->{}), restarting...",
                                mv.source_index(),
                                mv.destination_index()
                            );
                        }

                        mv.perform_move_on_bottles(&mut current_state);

                        reveal_observed(&mut current_state, resolved);
                        improve_best_revealed_state(&mut max_revealed, initial, &current_state);
                    }
                }
                DiscoverResult::NoMove => {
                    println!("No more discovery moves found, simulating restart...");
                    current_state = initial.to_vec();
                    improve_current_bottles_with_revealed_state(&mut current_state, &max_revealed);
                    println!(
                        "State after restart: {}",
                        current_state
                            .iter()
                            .map(|b| b.to_string())
                            .collect::<Vec<String>>()
                            .join(" ")
                    );
                }
                DiscoverResult::AlreadySolved => {
                    break;
                }
            }
        }

        if count_hidden_bottles(&max_revealed) > 0 && count_total_mystery_colors(&max_revealed) == 0
        {
            match find_best_hidden_unlock_moves(&current_state) {
                DiscoverResult::MoveToDiscover(moves_to_apply) => {
                    if moves_to_apply.is_empty() {
                        panic!("Moves should not be empty");
                    }

                    for mv in moves_to_apply {
                        log::debug!(
                            "Applying hidden unlock move {}->{} on state {}",
                            mv.source_index(),
                            mv.destination_index(),
                            current_state
                                .iter()
                                .map(|b| b.to_string())
                                .collect::<Vec<String>>()
                                .join(" ")
                        );
                        if !mv.can_perform_on_bottles(&current_state) {
                            panic!(
                                "Hidden unlock move became invalid during simulation ({}->{}), restarting...",
                                mv.source_index(),
                                mv.destination_index()
                            );
                        }

                        mv.perform_move_on_bottles(&mut current_state);

                        reveal_hidden_observed(&mut current_state, resolved);
                        improve_revealed_hidden_bottles(&mut max_revealed, &current_state);
                    }
                }
                DiscoverResult::NoMove => {
                    println!("No hidden unlock moves found, simulating restart...");
                    current_state = initial.to_vec();
                    improve_current_bottles_with_revealed_state(&mut current_state, &max_revealed);
                    println!(
                        "State after restart: {}",
                        current_state
                            .iter()
                            .map(|b| b.to_string())
                            .collect::<Vec<String>>()
                            .join(" ")
                    );
                }
                DiscoverResult::AlreadySolved => {
                    reveal_hidden_observed(&mut current_state, resolved);
                    improve_revealed_hidden_bottles(&mut max_revealed, &current_state);
                }
            }

            continue;
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
                if observed[fill_index].0 != BottleColor::Mystery {
                    break;
                }

                if fill_index >= resolved.len() {
                    break;
                }

                let was_mystery = observed[fill_index].1;
                observed[fill_index] = (resolved[fill_index], was_mystery);
                index -= 1;
            }
        });
}

fn reveal_hidden_observed(current: &mut [Bottle], fully_resolved: &[Bottle]) {
    let solved_colors: HashSet<BottleColor> =
        current.iter().filter_map(Bottle::solved_color).collect();

    current
        .iter_mut()
        .zip(fully_resolved.iter())
        .for_each(|(current_bottle, resolved_bottle)| {
            let Some(requirement) = current_bottle.locked_hidden_requirement() else {
                return;
            };

            if solved_colors.contains(&requirement) && current_bottle.is_hidden_and_locked() {
                *current_bottle = Bottle::from_fills(resolved_bottle.get_fills());
                current_bottle.set_hidden_requirement(crate::bottles::HiddenRequirement::Unlocked(
                    requirement,
                ));
            }
        });
}

fn improve_revealed_hidden_bottles(max_revealed: &mut [Bottle], current: &[Bottle]) {
    max_revealed
        .iter_mut()
        .zip(current.iter())
        .for_each(|(revealed_bottle, current_bottle)| {
            if revealed_bottle.is_hidden_and_locked() && !current_bottle.is_hidden_and_locked() {
                *revealed_bottle = current_bottle.clone();
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
