use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    bottles::{Bottle, BottleLayout},
    constants::BottleColor,
};

pub mod discovery;

/// Indicates the move to perform: pour from bottle at index 0 to bottle at index 1
#[derive(Debug, Clone, Copy)]
pub struct Move(usize, usize);

pub mod visualization;

#[derive(Debug)]
struct SearchNode {
    state: Vec<Bottle>,
    parent_index: Option<usize>,
    move_taken: Option<Move>,
    depth: usize,
}

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
    #[cfg(test)]
    pub fn source_index(&self) -> usize {
        self.0
    }

    #[cfg(test)]
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

#[allow(dead_code)]
pub type CallbackFn = dyn FnMut(&[Bottle], Option<Move>);

fn is_solved_state(bottles: &[Bottle]) -> bool {
    bottles.iter().all(|b| b.is_solved() || b.is_empty())
}

fn canonicalize_state(bottles: &[Bottle]) -> Vec<Vec<BottleColor>> {
    let mut normalized: Vec<Vec<BottleColor>> = bottles
        .iter()
        .map(|bottle| bottle.get_fills().clone())
        .collect();
    normalized.sort();
    normalized
}

fn reconstruct_moves(nodes: &[SearchNode], mut node_index: usize) -> Vec<Move> {
    let mut path = Vec::new();

    while let Some(parent_index) = nodes[node_index].parent_index {
        if let Some(m) = nodes[node_index].move_taken {
            path.push(m);
        }
        node_index = parent_index;
    }

    path.reverse();
    path
}

fn solve_shortest_path(bottles: &[Bottle]) -> Option<Vec<Move>> {
    if is_solved_state(bottles) {
        return Some(Vec::new());
    }

    let mut queue = VecDeque::new();
    let mut nodes = Vec::new();
    let mut best_depth_by_state: HashMap<Vec<Vec<BottleColor>>, usize> = HashMap::new();

    nodes.push(SearchNode {
        state: bottles.to_vec(),
        parent_index: None,
        move_taken: None,
        depth: 0,
    });

    queue.push_back(0);
    best_depth_by_state.insert(canonicalize_state(bottles), 0);

    while let Some(node_index) = queue.pop_front() {
        let depth = nodes[node_index].depth;
        let state = nodes[node_index].state.clone();

        let mut possible_moves = get_possible_moves_unfiltered(&state);
        sort_moves_by_heuristic(&mut possible_moves);

        for (m, new_state) in possible_moves {
            let next_depth = depth + 1;
            let state_key = canonicalize_state(&new_state);

            if best_depth_by_state
                .get(&state_key)
                .is_some_and(|best_depth| *best_depth <= next_depth)
            {
                continue;
            }

            best_depth_by_state.insert(state_key, next_depth);

            let next_index = nodes.len();
            nodes.push(SearchNode {
                state: new_state,
                parent_index: Some(node_index),
                move_taken: Some(m),
                depth: next_depth,
            });

            if is_solved_state(&nodes[next_index].state) {
                return Some(reconstruct_moves(&nodes, next_index));
            }

            queue.push_back(next_index);
        }
    }

    None
}

pub fn run_solver(bottles: &[Bottle]) -> Option<Vec<Move>> {
    println!("Solving puzzle with initial state: {:?}", bottles);
    solve_shortest_path(bottles)
}

pub fn get_possible_moves(
    bottles: &[Bottle],
    visited_states: &HashSet<Vec<Bottle>>,
) -> Vec<(Move, Vec<Bottle>)> {
    get_possible_moves_unfiltered(bottles)
        .into_iter()
        .filter(|(_, state)| !visited_states.contains(state))
        .collect()
}

pub fn get_possible_moves_unfiltered(bottles: &[Bottle]) -> Vec<(Move, Vec<Bottle>)> {
    let mut possible_moves = Vec::new();

    for source_idx in 0..bottles.len() {
        let source_bottle = &bottles[source_idx];
        if source_bottle.is_solved() || source_bottle.is_empty() {
            continue;
        }

        let source_is_one_color = {
            let fills = source_bottle.get_fills();
            let hash_set = std::collections::HashSet::<&BottleColor>::from_iter(fills.iter());
            hash_set.len() == 1
        };

        let mut saw_empty_destination = false;

        for destination_idx in 0..bottles.len() {
            if source_idx == destination_idx {
                continue;
            }

            let destination_bottle = &bottles[destination_idx];

            if destination_bottle.is_solved() {
                continue;
            }

            if destination_bottle.is_empty() {
                if saw_empty_destination {
                    continue;
                }
                saw_empty_destination = true;
            }

            if source_is_one_color && destination_bottle.is_empty() {
                continue;
            }

            if !destination_bottle.can_fill_from(source_bottle) {
                continue;
            }

            let mut new_bottles = bottles.to_vec();
            let move_to_try = Move(source_idx, destination_idx);
            move_to_try.perform_move_on_bottles(&mut new_bottles);

            possible_moves.push((move_to_try, new_bottles));
        }
    }

    possible_moves
}

pub fn sort_moves_by_heuristic(possible_moves: &mut [(Move, Vec<Bottle>)]) {
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
        .sum()
}
