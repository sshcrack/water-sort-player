use std::collections::HashSet;

use crate::{
    bottles::Bottle,
    constants::BottleColor,
    solver::{Move, get_possible_moves},
};

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

/// Replaces the mystery colors in the already visited states with the revealed colors from the max revealed bottle state, and checks if any of those states are now solved. If so, it returns the solution to that state.
pub fn reveal_mystery_colors_in_already_visited(
    max_revealed_bottle_state: &[Bottle],
    already_visited_states: &mut HashSet<Vec<Bottle>>,
) {
    let mut new_visited_states = HashSet::new();

    for state in already_visited_states.iter() {
        let zipped = state.iter().zip(max_revealed_bottle_state.iter());
        let new_state: Vec<Bottle> = zipped
            .map(|(current_state_bottle, bottle_with_revealed_colors)| {
                let fills = current_state_bottle
                    .get_fills()
                    .iter()
                    .zip(bottle_with_revealed_colors.get_fills().iter())
                    .map(|(color_in_state, revealed_color)| {
                        if color_in_state == &BottleColor::Mystery {
                            *revealed_color
                        } else {
                            *color_in_state
                        }
                    })
                    .collect();

                Bottle::from_fills(fills)
            })
            .collect();

        new_visited_states.insert(new_state);
    }

    *already_visited_states = new_visited_states;
}

pub enum DiscoverResult {
    NoMove,
    MoveToDiscover(Vec<Move>),
    AlreadySolved,
}

fn inner_discovery_mode(
    current_state: Vec<Bottle>,
    current_moves: Vec<Move>,
    already_visited_states: &mut HashSet<Vec<Bottle>>,
) -> Option<Vec<Move>> {
    if !already_visited_states.insert(current_state.clone()) {
        return None;
    }

    let possible_moves = get_possible_moves(&current_state, already_visited_states);

    for (m, new_state) in &possible_moves {
        let has_any_mystery_on_top = new_state.iter().any(|b| {
            if let Some((_, top_color)) = b.get_top_fill() {
                top_color == BottleColor::Mystery
            } else {
                false
            }
        });

        if has_any_mystery_on_top {
            let mut new_moves = current_moves.clone();
            new_moves.push(*m);
            return Some(new_moves);
        }
    }

    for (m, new_state) in possible_moves {
        let mut new_moves = current_moves.clone();
        new_moves.push(m);

        if let Some(result) = inner_discovery_mode(new_state, new_moves, already_visited_states) {
            return Some(result);
        }
    }

    None
}

pub fn find_best_discovery_moves(
    current_bottles: &[Bottle],
    max_revealed_bottle_state: &[Bottle],
    already_visited_states: &mut HashSet<Vec<Bottle>>,
) -> DiscoverResult {
    let already_solved = max_revealed_bottle_state
        .iter()
        .all(|b| b.is_solved() || b.is_empty());
    if already_solved {
        return DiscoverResult::AlreadySolved;
    }

    match inner_discovery_mode(current_bottles.to_vec(), Vec::new(), already_visited_states) {
        Some(moves) => DiscoverResult::MoveToDiscover(moves),
        None => DiscoverResult::NoMove,
    }
}

pub fn improve_best_revealed_state(
    initial_revealed_bottle_state: &mut [Bottle],
    current_bottles: &[Bottle],
) {
    initial_revealed_bottle_state
        .iter_mut()
        .zip(current_bottles.iter())
        .for_each(|(revealed_bottle, current_bottle)| {
            revealed_bottle
                .get_fills_mut()
                .iter_mut()
                .zip(current_bottle.get_fills().iter())
                .for_each(|(revealed_color, current_color)| {
                    if *revealed_color == BottleColor::Mystery {
                        *revealed_color = *current_color;
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use crate::{
        bottles::test_utils::TestUtils,
        solver::discovery::{count_total_mystery_colors, improve_best_revealed_state},
    };

    #[test]
    fn test_count_total_mystery_colors() {
        let bottles = TestUtils::parse_bottles_sequence("P??? YGBR G???");

        assert_eq!(count_total_mystery_colors(&bottles), 6);
    }

    #[test]
    fn test_improve_best_revealed_state() {
        let mut revealed_state = TestUtils::parse_bottles_sequence("PY?? Y??? G???");
        let current_bottles = TestUtils::parse_bottles_sequence("P??? YG?? G???");
        improve_best_revealed_state(&mut revealed_state, &current_bottles);

        let expected_revealed_state = TestUtils::parse_bottles_sequence("PY?? YG?? G???");
        assert_eq!(revealed_state, expected_revealed_state);
    }
}
