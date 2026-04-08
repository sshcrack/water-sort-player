use std::collections::HashSet;

use crate::{Move, get_possible_moves};
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
) -> DiscoverResult {
    let already_solved = max_revealed_bottle_state
        .iter()
        .all(|b| b.is_solved() || b.is_empty());
    if already_solved {
        return DiscoverResult::AlreadySolved;
    }

    match inner_discovery_mode(current_bottles.to_vec(), Vec::new(), &mut HashSet::new()) {
        Some(moves) => DiscoverResult::MoveToDiscover(moves),
        None => DiscoverResult::NoMove,
    }
}

pub fn improve_best_revealed_state(
    initial_revealed_bottle_state: &mut [Bottle],
    previous_bottles: &[Bottle],
    current_bottles: &[Bottle],
) {
    initial_revealed_bottle_state
        .iter_mut()
        .zip(current_bottles.iter())
        .zip(previous_bottles.iter())
        .for_each(|((revealed_bottle, current_bottle), previous_bottle)| {
            revealed_bottle
                .get_fills_mut()
                .iter_mut()
                .zip(current_bottle.get_fills().iter())
                .zip(previous_bottle.get_fills().iter())
                .for_each(|((revealed_color, current_color), previous_color)| {
                    if *revealed_color == BottleColor::Mystery
                        && previous_color == &BottleColor::Mystery
                        && current_color != &BottleColor::Mystery
                    {
                        *revealed_color = *current_color;
                    }
                });
        });
}

pub fn improve_current_bottles_with_revealed_state(
    current_bottles: &mut [Bottle],
    max_revealed_bottle_state: &[Bottle],
) {
    current_bottles
        .iter_mut()
        .zip(max_revealed_bottle_state.iter())
        .for_each(|(current_bottle, revealed_bottle)| {
            current_bottle
                .get_fills_mut()
                .iter_mut()
                .zip(revealed_bottle.get_fills().iter())
                .for_each(|(current_color, revealed_color)| {
                    if *current_color == BottleColor::Mystery {
                        *current_color = *revealed_color;
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use crate::discovery::{count_total_mystery_colors, improve_best_revealed_state};
    use water_sort_core::bottles::test_utils::TestUtils;

    #[test]
    fn test_count_total_mystery_colors() {
        let bottles = TestUtils::parse_bottles_sequence("P??? YGBR G???");

        assert_eq!(count_total_mystery_colors(&bottles), 6);
    }

    #[test]
    fn test_improve_best_revealed_state() {
        let mut revealed_state = TestUtils::parse_bottles_sequence("PY?? Y??? G???");
        let previous_bottles = TestUtils::parse_bottles_sequence("PY?? Y??? G???");
        let current_bottles = TestUtils::parse_bottles_sequence("P??? YG?? G???");
        improve_best_revealed_state(&mut revealed_state, &previous_bottles, &current_bottles);

        let expected_revealed_state = TestUtils::parse_bottles_sequence("PY?? YG?? G???");
        assert_eq!(revealed_state, expected_revealed_state);
    }
}
