use std::cmp::Ordering;
use std::collections::HashSet;

use crate::{bottles::Bottle, constants::BottleColor};

/// Indicates the move to perform: pour from bottle at index 0 to bottle at index 1
#[derive(Debug, Clone)]
pub struct Move(usize, usize);

#[cfg(test)]
mod tests;

fn get_two_mut_from_vec<'a>(
    bottles: &'a mut [Bottle],
    a: usize,
    b: usize,
) -> (&'a mut Bottle, &'a mut Bottle) {
    assert_ne!(a, b, "source and destination must be different");

    if a < b {
        let (left, right) = bottles.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = bottles.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

impl Move {
    pub fn perform_move_on_device(&self) {
        crate::scrcpy::click_at_position(crate::position::get_bottle_position(self.0));
        crate::scrcpy::click_at_position(crate::position::get_bottle_position(self.1));
    }

    pub fn perform_move_on_bottles(&self, bottles: &mut [Bottle]) {
        let (source_bottle, destination_bottle) = get_two_mut_from_vec(bottles, self.0, self.1);
        if !destination_bottle.can_fill_from(source_bottle) {
            panic!(
                "Invalid move: cannot pour from bottle {} to bottle {}",
                self.0, self.1
            );
        }

        destination_bottle.fill_from(source_bottle);
    }
}

pub fn run_solver(bottles: &Vec<Bottle>) -> Option<Vec<Move>> {
    println!("Solving puzzle with initial state: {:?}", bottles);

    let mut visited_states = HashSet::new();
    inner_solver(bottles.clone(), Vec::new(), 0, &mut visited_states)
}

fn inner_solver(
    bottles: Vec<Bottle>,
    moves_so_far: Vec<Move>,
    level: usize,
    visited_states: &mut HashSet<Vec<Bottle>>,
) -> Option<Vec<Move>> {
    if !visited_states.insert(bottles.clone()) {
        return None;
    }

    if bottles.iter().all(|b| b.is_solved()) {
        return Some(moves_so_far);
    }

    let mut possible_moves = vec![];
    for source_idx in 0..bottles.len() {
        for destination_idx in 0..bottles.len() {
            if source_idx == destination_idx {
                continue;
            }

            let source_bottle = &bottles[source_idx];
            let destination_bottle = &bottles[destination_idx];

            if source_bottle.is_solved()
                || source_bottle.is_empty()
                || destination_bottle.is_solved()
            {
                continue;
            }

            if !destination_bottle.can_fill_from(source_bottle) {
                continue;
            }

            let mut new_bottles = bottles.clone();
            let move_to_try = Move(source_idx, destination_idx);
            move_to_try.perform_move_on_bottles(&mut new_bottles);

            if visited_states.contains(&new_bottles) {
                continue;
            }

            possible_moves.push((move_to_try, new_bottles));
        }
    }

    sort_moves_by_heuristic(&mut possible_moves);

    for (m, new_bottles) in possible_moves {
        let mut new_moves_so_far = moves_so_far.clone();
        let level_indent = "  ".repeat(level);
        println!("{}Trying move: {:?}", level_indent, m);
        std::thread::sleep(std::time::Duration::from_millis(50));

        new_moves_so_far.push(m);
        if let Some(solution) = inner_solver(
            new_bottles,
            new_moves_so_far,
            level + 1,
            visited_states,
        ) {
            return Some(solution);
        }
    }

    None
}

fn sort_moves_by_heuristic(possible_moves: &mut [(Move, Vec<Bottle>)]) {
    possible_moves.sort_by(|(_, a), (_, b)| {
    let a_solved = a.iter().filter(|x| x.is_solved()).count();
    let b_solved = b.iter().filter(|x| x.is_solved()).count();

    match b_solved.cmp(&a_solved) {
        Ordering::Equal => {
            let unique_a = get_unique_colors_sum(a);
            let unique_b = get_unique_colors_sum(b);
            unique_a.cmp(&unique_b)
        }
        other => other,
    }
    })
}

fn get_unique_colors_sum(a: &[Bottle]) -> usize {
    a.iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let fills = b.get_fills();
            let hash_set = std::collections::HashSet::<&BottleColor>::from_iter(fills.iter());

            hash_set.len()
        })
        .count()
}
