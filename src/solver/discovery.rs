use std::collections::HashSet;

use crate::{bottles::Bottle, constants::BottleColor, solver::Move};

/// Counts the number of mystery colors remaining in all bottles
pub fn count_mystery_colors(bottles: &[Bottle]) -> usize {
    bottles
        .iter()
        .flat_map(|bottle| bottle.get_fills())
        .filter(|color| **color == BottleColor::Mystery)
        .count()
}

/// Generates legal moves, prioritizing those involving mystery colors.
/// Returns moves ordered by: moves with mystery source/dest first, then others.
fn get_moves_prioritizing_mystery(bottles: &[Bottle]) -> Vec<Move> {
    let mut mystery_moves = Vec::new();
    let mut other_moves = Vec::new();

    for source_idx in 0..bottles.len() {
        for destination_idx in 0..bottles.len() {
            if source_idx == destination_idx {
                continue;
            }

            let source_bottle = &bottles[source_idx];
            let destination_bottle = &bottles[destination_idx];

            // Skip invalid moves
            if source_bottle.is_solved()
                || source_bottle.is_empty()
                || destination_bottle.is_solved()
            {
                continue;
            }

            let is_bottle_of_one_color = |bottle: &Bottle| {
                let fills = bottle.get_fills();
                let hash_set = std::collections::HashSet::<&BottleColor>::from_iter(fills.iter());
                hash_set.len() == 1
            };

            if is_bottle_of_one_color(source_bottle) && destination_bottle.is_empty() {
                continue;
            }

            if !destination_bottle.can_fill_from(source_bottle) {
                continue;
            }

            let has_mystery = |bottle: &Bottle| {
                bottle
                    .get_fills()
                    .iter()
                    .any(|color| *color == BottleColor::Mystery)
            };

            let m = Move(source_idx, destination_idx);

            // Prioritize moves involving mystery bottles
            if has_mystery(source_bottle) || has_mystery(destination_bottle) {
                mystery_moves.push(m);
            } else {
                other_moves.push(m);
            }
        }
    }

    // Combine: mystery moves first, then others
    mystery_moves.extend(other_moves);
    mystery_moves
}

/// Main discovery algorithm: returns the next move to try if mysteries remain, or None if all revealed/stuck.
/// This is meant to be called iteratively as moves are executed and re-detected on device.
pub fn get_next_discovery_move(
    bottles: &[Bottle],
    visited_states: &mut HashSet<Vec<Bottle>>,
) -> Result<Option<Move>, String> {
    // If no mysteries remain, discovery is complete
    if count_mystery_colors(bottles) == 0 {
        return Ok(None);
    }

    // Mark this state as visited
    if !visited_states.insert(bottles.to_vec()) {
        return Err("Discovery stuck: revisited state, no new moves possible".to_string());
    }

    // Generate moves prioritizing mystery bottles
    let possible_moves = get_moves_prioritizing_mystery(bottles);

    if possible_moves.is_empty() {
        return Err(format!(
            "Discovery stuck: no legal moves available, mysteries remaining: {}",
            count_mystery_colors(bottles)
        ));
    }

    // For discovery, we prioritize moves that involve mystery bottles
    // Return the first one (already prioritized)
    Ok(possible_moves.first().copied())
}
