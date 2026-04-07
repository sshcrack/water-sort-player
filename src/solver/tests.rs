macro_rules! create_test_level {
    ($level:literal, $bottles:expr) => {
        paste::paste! {
            #[test]
            fn [<solver_level_ $level>]() {
                use crate::solver::Bottle;
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
                    println!("Move from bottle {} to bottle {}", m.0, m.1);
                    m.perform_move_on_bottles(&mut bottles_parsed);
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
