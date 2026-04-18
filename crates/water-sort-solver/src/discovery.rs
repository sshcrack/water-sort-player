use std::collections::HashSet;

use crate::{Move, find_shortest_move_sequence};
use water_sort_core::{bottles::Bottle, constants::BottleColor};

pub fn count_total_mystery_colors(bottles: &[Bottle]) -> usize {
    bottles
        .iter()
        .map(|b| {
            b.get_fills()
                .iter()
                .filter(|&color| color == &BottleColor::Mystery)
                .count()
        })
        .sum()
}

pub fn count_hidden_bottles(bottles: &[Bottle]) -> usize {
    bottles
        .iter()
        .filter(|bottle| bottle.is_hidden_and_locked())
        .count()
}

pub fn collect_hidden_requirements(bottles: &[Bottle]) -> HashSet<BottleColor> {
    bottles
        .iter()
        .filter(|bottle| bottle.is_hidden_and_locked() && bottle.is_empty())
        .filter_map(Bottle::hidden_requirement)
        .filter(|color| *color != BottleColor::Empty)
        .collect()
}

#[derive(Debug)]
pub enum DiscoverResult {
    NoMove,
    MoveToDiscover(Vec<Move>),
    AlreadySolved,
}

pub fn find_best_hidden_unlock_moves(current_bottles: &[Bottle]) -> DiscoverResult {
    log::debug!(
        "Finding best hidden unlock moves for current bottles: {}",
        current_bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let hidden_requirements = collect_hidden_requirements(current_bottles);
    if hidden_requirements.is_empty() {
        return DiscoverResult::AlreadySolved;
    }

    if current_bottles.iter().any(|bottle| {
        bottle
            .solved_color()
            .is_some_and(|color| hidden_requirements.contains(&color))
    }) {
        return DiscoverResult::AlreadySolved;
    }

    let best_moves = find_shortest_move_sequence(
        current_bottles.to_vec(),
        None,
        |state, move_count| {
            println!("Checking state: {}, move_count: {}", state.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(" "), move_count);
            move_count > 0
                && state.iter().any(|bottle| {
                    bottle
                        .solved_color()
                        .is_some_and(|color| hidden_requirements.contains(&color))
                })
        },
        #[cfg(feature = "solver-visualization")]
        None,
    );

    match best_moves {
        Some(moves) => DiscoverResult::MoveToDiscover(moves),
        None => DiscoverResult::NoMove,
    }
}

pub fn find_best_discovery_moves(
    current_bottles: &[Bottle],
    max_revealed_bottle_state: &[Bottle],
) -> DiscoverResult {
    let already_solved = max_revealed_bottle_state
        .iter()
        .all(|b| b.is_solved() || b.is_empty());
    if already_solved {
        return DiscoverResult::AlreadySolved;
    }

    let best_moves = find_shortest_move_sequence(
        current_bottles.to_vec(),
        Some(&|prev_state, new_state| {
            let prev_hidden = prev_state
                .iter()
                .filter(|b| b.is_hidden_and_locked() && b.is_empty())
                .count();
            let new_hidden = new_state
                .iter()
                .filter(|b| b.is_hidden_and_locked() && b.is_empty())
                .count();

            if new_hidden != prev_hidden {
                log::trace!(
                    "Hidden bottle count changed from {} to {} after move. New state: {}",
                    prev_hidden,
                    new_hidden,
                    new_state
                        .iter()
                        .map(|b| b.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
            new_hidden == prev_hidden
        }),
        |state, move_count| {
            move_count > 0
                && state.iter().any(|bottle| {
                    if let Some((_, top_color)) = bottle.get_top_fill() {
                        top_color == BottleColor::Mystery
                    } else {
                        false
                    }
                })
        },
        #[cfg(feature = "solver-visualization")]
        None,
    );

    match best_moves {
        Some(moves) => DiscoverResult::MoveToDiscover(moves),
        None => DiscoverResult::NoMove,
    }
}

pub fn improve_best_revealed_state(
    initial_revealed_bottle_state: &mut [Bottle],
    initial_bottles: &[Bottle],
    current_bottles: &[Bottle],
) {
    initial_revealed_bottle_state
        .iter_mut()
        .zip(current_bottles.iter())
        .zip(initial_bottles.iter())
        .for_each(|((revealed_bottle, current_bottle), initial_bottle)| {
            // We'll need to fill initial hidden bottle fills
            let mut initial_bottle = initial_bottle.clone();
            if initial_bottle.is_hidden_and_locked() && initial_bottle.is_empty() {
                initial_bottle.set_fills_from_bottle(revealed_bottle);
            }

            revealed_bottle
                .get_fills_mut()
                .iter_mut()
                .zip(current_bottle.get_fills().iter())
                .zip(initial_bottle.get_fills().iter())
                .for_each(|(((revealed_color, _), current_color), initial_color)| {
                    if *revealed_color == BottleColor::Mystery
                        && initial_color == &BottleColor::Mystery
                        && current_color != &BottleColor::Mystery
                    {
                        *revealed_color = *current_color;
                    }
                });

            if revealed_bottle.is_empty()
                && revealed_bottle.is_hidden_and_locked()
                && !current_bottle.is_hidden_and_locked()
            {
                revealed_bottle.set_fills_from_bottle(current_bottle);
                // We are unlocking it to show that we have discovered the hidden bottle, we'll need to reset when solving
                revealed_bottle.unlock_hidden_requirement();
            }
        });
}

pub fn improve_current_and_initial_bottles_with_revealed_state(
    current_bottles: &mut [Bottle],
    initial_bottles: &mut [Bottle],
    max_revealed_bottle_state: &[Bottle],
) {
    // We are only modifying the initial bottles for the following edge case:
    // There were hidden bottles that contained mystery colors. The initial state
    // doesn't have any fills / mystery colors at all
    // so the first chance we get, we set the initial_bottle hidden bottle fills.

    let solved_bottles = max_revealed_bottle_state
        .iter()
        .filter_map(|bottle| bottle.solved_color())
        .collect::<Vec<_>>();

    initial_bottles
        .iter_mut()
        .zip(max_revealed_bottle_state.iter())
        .for_each(|(initial_bottle, revealed_bottle)| {
            if initial_bottle.is_hidden_and_locked() && initial_bottle.is_empty() && !revealed_bottle.is_empty() {
                println!(
                    "Setting initial bottle fills from revealed bottle. Initial bottle: {}, Revealed bottle: {}",
                    initial_bottle, revealed_bottle
                );
                initial_bottle.set_fills_from_bottle(revealed_bottle);
            }
        });

    current_bottles
        .iter_mut()
        .zip(max_revealed_bottle_state.iter())
        .for_each(|(current_bottle, revealed_bottle)| {
            current_bottle
                .get_fills_mut()
                .iter_mut()
                .zip(revealed_bottle.get_fills().iter())
                .for_each(|((current_color, _), revealed_color)| {
                    if *current_color == BottleColor::Mystery {
                        *current_color = *revealed_color;
                    }
                });

            if current_bottle.is_empty()
                && current_bottle.is_hidden_and_locked()
                && let Some(req_color) = revealed_bottle.hidden_requirement()
            {
                current_bottle.set_fills_from_bottle(revealed_bottle);
                // We are unlocking it to show that we have discovered the hidden bottle, we'll need to reset when solving
                if solved_bottles.contains(&req_color) {
                    current_bottle.unlock_hidden_requirement();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use crate::discovery::{
        DiscoverResult, collect_hidden_requirements, count_hidden_bottles,
        count_total_mystery_colors, find_best_discovery_moves, find_best_hidden_unlock_moves,
        improve_best_revealed_state, improve_current_and_initial_bottles_with_revealed_state,
    };
    use crate::{run_solver, unlock_hidden_bottles_with_solved_colors};
    use water_sort_core::bottles::test_utils::TestUtils;
    use water_sort_core::constants::BottleColor;

    #[test_log::test]
    fn test_count_total_mystery_colors() {
        let bottles = TestUtils::parse_bottles_sequence("P??? YGBR G???");

        assert_eq!(count_total_mystery_colors(&bottles), 6);
    }

    #[test_log::test]
    fn test_hidden_requirement_helpers() {
        let mut bottles = TestUtils::parse_bottles_sequence("OOOR B??R EEEE");
        bottles.push(water_sort_core::bottles::Bottle::from_hidden_requirement(
            BottleColor::orange(),
            None,
        ));

        assert_eq!(count_hidden_bottles(&bottles), 1);
        assert!(collect_hidden_requirements(&bottles).contains(&BottleColor::orange()));
    }

    #[test_log::test]
    fn test_find_best_hidden_unlock_moves() {
        let bottles = TestUtils::parse_bottles_sequence("OOOR EEEO EEEE EEEE !O");

        match find_best_hidden_unlock_moves(&bottles) {
            DiscoverResult::MoveToDiscover(moves) => {
                assert_eq!(moves.len(), 1);
                let mut next_state = bottles.clone();
                moves[0].perform_move_on_bottles(&mut next_state);
                assert!(
                    next_state
                        .iter()
                        .any(|bottle| bottle.solved_color() == Some(BottleColor::orange()))
                );
            }
            other => panic!(
                "Expected a move sequence to unlock hidden bottle, got {:?}",
                other
            ),
        }
    }

    #[test_log::test]
    fn test_improve_best_revealed_state() {
        let mut revealed_state = TestUtils::parse_bottles_sequence("PY?? Y??? G???");
        let previous_bottles = TestUtils::parse_bottles_sequence("PY?? Y??? G???");
        let current_bottles = TestUtils::parse_bottles_sequence("P??? YG?? G???");
        improve_best_revealed_state(&mut revealed_state, &previous_bottles, &current_bottles);

        let expected_revealed_state = TestUtils::parse_bottles_sequence("PY?? YG?? G???");
        assert!(
            TestUtils::are_bottles_equal(&revealed_state, &expected_revealed_state),
            "Revealed state should be improved with newly discovered colors. Actual: {:?}, Expected: {:?}",
            revealed_state,
            expected_revealed_state
        );
    }

    #[test_log::test]
    fn test_find_best_discovery_moves_for_level24_restart_state() {
        let bottles = TestUtils::parse_bottles_sequence("RRRR !G !G O??G G??B EEEE EEEE OYYW");

        match find_best_discovery_moves(&bottles, &bottles) {
            DiscoverResult::MoveToDiscover(moves) => {
                assert!(!moves.is_empty());
            }
            other => panic!("Expected discovery moves, got {:?}", other),
        }
    }

    #[test_log::test]
    fn test_find_best_discovery_moves_for_level25_restart_state() {
        let bottles = TestUtils::parse_bottles_sequence("!R !R PYBB OYBG R?YR RY?R EEEE EEEE");

        match find_best_discovery_moves(&bottles, &bottles) {
            DiscoverResult::MoveToDiscover(moves) => {
                assert!(!moves.is_empty());
            }
            other => panic!("Expected discovery moves, got {:?}", other),
        }
    }

    #[test_log::test]
    fn test_discovery_with_bottles_that_can_be_unlocked() {
        let current_bottles =
            TestUtils::parse_bottles_sequence("POG? !B WGP? !Y YPW? BRYO YYBR EEEE EEEE");
        let max_revealed_bottle_state =
            TestUtils::parse_bottles_sequence("POG? !B WGP? !Y YPW? BRYO YYBR EEEE EEEE");

        match find_best_discovery_moves(&current_bottles, &max_revealed_bottle_state) {
            DiscoverResult::NoMove => println!("NoMove"),
            DiscoverResult::MoveToDiscover(items) => {
                println!("MoveToDiscover with moves:");
                let mut new_state = current_bottles.clone();
                for m in items {
                    println!("{:?}", m);
                    m.perform_move_on_bottles(&mut new_state);
                    log::debug!(
                        "State after move: {}",
                        new_state
                            .iter()
                            .map(|b| b.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                }
            }
            DiscoverResult::AlreadySolved => println!("AlreadySolved"),
        }
    }

    #[test_log::test]
    fn test_discovery_with_mystery() {
        //let current_bottles = TestUtils::parse_bottles_sequence("POG? !B WGP? !Y Y??? BRYO YYBR EEEE EEEE");
        let mut current_bottles =
            TestUtils::parse_bottles_sequence("P??? !B W??? !Y Y??? BRYO YYBR EEEE EEEE");
        let mut initial = current_bottles.clone();
        let max_revealed_bottles =
            TestUtils::parse_bottles_sequence("POGW !B,ORRG WGPP !Y,BOB? YPWG BRYO YYBR EEEE EEEE");

        improve_current_and_initial_bottles_with_revealed_state(
            &mut current_bottles,
            &mut initial,
            &max_revealed_bottles,
        );
        log::debug!(
            "Current bottles after improving with revealed state: {}",
            current_bottles
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
        match find_best_discovery_moves(&current_bottles, &max_revealed_bottles) {
            DiscoverResult::NoMove => panic!("Should find a move to unlock hidden bottle"),
            DiscoverResult::MoveToDiscover(items) => {
                println!("MoveToDiscover with moves:");
                let mut new_state = current_bottles.clone();
                for m in items {
                    println!("{:?}", m);
                    m.perform_move_on_bottles(&mut new_state);
                    unlock_hidden_bottles_with_solved_colors(&mut new_state);

                    log::debug!(
                        "State after move: {}",
                        new_state
                            .iter()
                            .map(|b| b.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                }
            }
            DiscoverResult::AlreadySolved => println!("AlreadySolved"),
        }
    }

    #[test_log::test]
    fn test_discov_1() {
        let initial = TestUtils::parse_bottles_sequence("!P !W !W !B B??G W??B B??Y EEEE EEEE");
        let max = TestUtils::parse_bottles_sequence(
            "!P,GOGO !W,PPYP !W,RGOO !B,RRWP BWRG WYYB BWBY EEEE EEEE",
        );

        let m = run_solver(&max, &initial);

        match m {
            None => println!("NoMove"),
            Some(items) => {
                println!("MoveToDiscover with moves:");
                let mut new_state = max.clone();
                for m in items {
                    println!("{:?}", m);
                    m.perform_move_on_bottles(&mut new_state);
                    log::debug!(
                        "State after move: {}",
                        new_state
                            .iter()
                            .map(|b| b.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                }
            }
        }
    }
}
