use std::cmp::Ordering;
use std::collections::HashSet;
#[cfg(feature = "solver-visualization")]
use std::thread;
#[cfg(feature = "solver-visualization")]
use std::time::Duration;

use crate::{
    bottles::{Bottle, BottleLayout},
    constants::BottleColor,
};

/// Indicates the move to perform: pour from bottle at index 0 to bottle at index 1
#[derive(Debug, Clone, Copy)]
pub struct Move(usize, usize);

#[cfg(feature = "solver-visualization")]
#[cfg(test)]
pub mod visualization;

#[cfg(test)]
mod tests;

fn get_two_mut_from_vec(bottles: &mut [Bottle], a: usize, b: usize) -> (&mut Bottle, &mut Bottle) {
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
    pub fn source_index(&self) -> usize {
        self.0
    }

    pub fn destination_index(&self) -> usize {
        self.1
    }

    pub fn perform_move_on_device(&self, layout: &BottleLayout) {
        crate::scrcpy::click_at_position(crate::position::get_bottle_position(layout, self.0));
        crate::scrcpy::click_at_position(crate::position::get_bottle_position(layout, self.1));
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

pub type CallbackFn = dyn FnMut(&[Bottle], Option<Move>);
struct SolverObserver<'a> {
    callback: Option<&'a mut CallbackFn>,
}

impl<'a> SolverObserver<'a> {
    fn new(callback: Option<&'a mut CallbackFn>) -> Self {
        Self { callback }
    }

    fn render(&mut self, bottles: &[Bottle], active_move: Option<Move>) {
        if let Some(callback) = self.callback.as_mut() {
            (**callback)(bottles, active_move);
        }
    }
}

#[allow(dead_code)]
pub fn run_solver(bottles: &[Bottle]) -> Option<Vec<Move>> {
    println!("Solving puzzle with initial state: {:?}", bottles);

    let mut visited_states = HashSet::new();
    let mut observer = SolverObserver::new(None);
    inner_solver(
        bottles.to_vec(),
        Vec::new(),
        &mut visited_states,
        &mut observer,
    )
}

#[allow(dead_code)]
#[cfg(feature = "solver-visualization")]
pub fn run_solver_with_visualization<F>(bottles: &[Bottle], callback: &mut F) -> Option<Vec<Move>>
where
    F: FnMut(&[Bottle], Option<Move>) + 'static,
{
    println!("Solving puzzle with initial state: {:?}", bottles);

    let mut visited_states = HashSet::new();
    let mut observer = SolverObserver::new(Some(callback));
    inner_solver(
        bottles.to_vec(),
        Vec::new(),
        &mut visited_states,
        &mut observer,
    )
}

fn inner_solver(
    bottles: Vec<Bottle>,
    moves_so_far: Vec<Move>,
    visited_states: &mut HashSet<Vec<Bottle>>,
    observer: &mut SolverObserver<'_>,
) -> Option<Vec<Move>> {
    observer.render(&bottles, None);

    if !visited_states.insert(bottles.clone()) {
        return None;
    }

    if bottles.iter().all(|b| b.is_solved() || b.is_empty()) {
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
        #[cfg(feature = "solver-visualization")]
        thread::sleep(Duration::from_millis(50));

        new_moves_so_far.push(m);
        observer.render(&new_bottles, Some(m));
        if let Some(solution) =
            inner_solver(new_bottles, new_moves_so_far, visited_states, observer)
        {
            return Some(solution);
        }

        observer.render(&bottles, None);
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
