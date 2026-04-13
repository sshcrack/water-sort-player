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
        .filter(|bottle| bottle.is_hidden_and_locked())
        .filter_map(Bottle::hidden_requirement)
        .collect()
}

#[derive(Debug)]
pub enum DiscoverResult {
    NoMove,
    MoveToDiscover(Vec<Move>),
    AlreadySolved,
}

pub fn find_best_hidden_unlock_moves(current_bottles: &[Bottle]) -> DiscoverResult {
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
        |state, move_count| {
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

pub fn improve_current_bottles_with_revealed_state(
    current_bottles: &mut [Bottle],
    max_revealed_bottle_state: &[Bottle],
) {
    let solved_bottles = max_revealed_bottle_state
        .iter()
        .filter_map(|bottle| bottle.solved_color())
        .collect::<Vec<_>>();

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
                && revealed_bottle.is_hidden_and_locked()
            {
                current_bottle.set_fills_from_bottle(revealed_bottle);
                current_bottle.set_hidden_requirement(revealed_bottle.hidden_requirement_state());

                if let Some(c) = current_bottle.hidden_requirement()
                    && solved_bottles.contains(&c)
                {
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
        improve_best_revealed_state,
    };
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
            BottleColor::Orange,
        ));

        assert_eq!(count_hidden_bottles(&bottles), 1);
        assert!(collect_hidden_requirements(&bottles).contains(&BottleColor::Orange));
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
                        .any(|bottle| bottle.solved_color() == Some(BottleColor::Orange))
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
    fn test_discovery_algorithm() {
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
}
