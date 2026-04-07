use crate::bottles::BottleLayout;

fn get_layout_from_bottle_count(count: usize) -> BottleLayout {
    match count {
        10 => BottleLayout::ten_bottle_layout(),
        11 => BottleLayout::eleven_bottle_layout(),
        _ => panic!("Unsupported bottle count: {}", count),
    }
}

macro_rules! create_test_level {
    ($level:literal, $bottles:expr) => {
        paste::paste! {
            mod [<level_ $level>] {
                use super::*;
                lazy_static::lazy_static! {
                    static ref PARSED_BOTTLES: Vec<crate::bottles::Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($bottles)
                        .into_iter()
                        .map(|fills| crate::bottles::Bottle::from_fills(fills))
                        .collect();
                }

                #[test]
                fn solve() {
                    use crate::bottles::Bottle;
                    let mut bottles_parsed: Vec<Bottle> = crate::bottles::test_utils::TestUtils::parse_bottles_sequence($bottles)
                        .into_iter()
                        .map(|fills| Bottle::from_fills(fills))
                        .collect();

                    #[cfg(feature = "solver-visualization")]
                    let solution = {

                        let mut window = minifb::Window::new(
                            "Solver Visualization",
                            800,
                            540,
                            minifb::WindowOptions::default(),
                        )
                        .expect("failed to create solver visualization window");

                        let mut render_step = move |state: &[Bottle], active_move: Option<crate::solver::Move>| {
                            let buffer = crate::solver::visualization::render_solver_view(
                                800,
                                540,
                                state,
                                active_move,
                            );

                            window
                                .update_with_buffer(&buffer, 800, 540)
                                .expect("failed to update solver visualization window");
                        };

                        crate::solver::run_solver_with_visualization(&bottles_parsed, &mut render_step)
                            .expect("No solution found")
                    };

                    #[cfg(not(feature = "solver-visualization"))]
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
                        assert_eq!(detected.get_fills(), expected.get_fills(), "Bottle {} fills do not match expected for level {}", idx, $level);
                    }
                }
            }
        }
    };
}

create_test_level!(213, "YRGM BPWO OBPG ROPM YRPW MWBG BGRY YMWO EEEE EEEE");
create_test_level!(
    214,
    "POGR LMOR GYPO GYGB WBRL MLRY WMLP POMW BWBY EEEE EEEE"
);
