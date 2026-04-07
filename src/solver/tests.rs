use crate::{bottles::Bottle, constants::BottleColor};

macro_rules! create_test_level {
    ($level:literal, $bottles:expr) => {
        paste::paste! {
            #[test]
            fn [<solver_level_ $level>]() {
                let mut bottles_parsed: Vec<Bottle> = $bottles
                    .split_whitespace()
                    .map(|bottle_str| {
                        let fills = bottle_str
                            .chars()
                            .filter_map(|c| match c {
                                'Y' => Some(BottleColor::Yellow),
                                'R' => Some(BottleColor::Red),
                                'G' => Some(BottleColor::Green),
                                'L' => Some(BottleColor::LightBlue),
                                'M' => Some(BottleColor::MediumBlue),
                                'B' => Some(BottleColor::Blue),
                                'P' => Some(BottleColor::Purple),
                                'O' => Some(BottleColor::Orange),
                                'W' => Some(BottleColor::Pink),
                                'E' => None,
                                _ => panic!("Invalid character in bottle string: {}", c),
                            })
                            .collect();

                        Bottle::from_fills(fills)
                    })
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

                    let mut render_step = |state: &[Bottle], active_move: Option<crate::solver::Move>| {
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
                    println!("Move from bottle {} to bottle {}", m.0, m.1);
                    m.perform_move_on_bottles(&mut bottles_parsed);
                }
            }
        }
    };
}

create_test_level!(213, "YRGM BPWO OBPG ROPM YRPW MWBG BGRY YMWO EEEE EEEE");
create_test_level!(214, "POGR LMOR GYPO GYGB WBRL MLRY WMLP POMW BWBY EEEE EEEE");