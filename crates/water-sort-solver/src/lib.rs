use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use water_sort_core::{
    bottles::{Bottle, BottleLayout},
    constants::BottleColor,
};
use water_sort_device::click_at_position;

pub mod discovery;

/// Indicates the move to perform: pour from bottle at index 0 to bottle at index 1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move(usize, usize);

pub mod visualization;

const FULL_BOTTLE_COUNT: usize = 4;

type CanonicalStateKey = Vec<Vec<BottleColor>>;

#[derive(Debug, Clone, Eq, PartialEq)]
struct SearchRecord {
    state: Vec<Bottle>,
    cost: usize,
    parent: Option<usize>,
    via_move: Option<Move>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct QueueEntry {
    estimated_total_cost: usize,
    cost: usize,
    record_index: usize,
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimated_total_cost
            .cmp(&self.estimated_total_cost)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.record_index.cmp(&self.record_index))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn canonical_state_key(bottles: &[Bottle]) -> CanonicalStateKey {
    let mut key = bottles
        .iter()
        .map(|bottle| bottle.get_fills().clone())
        .collect::<Vec<_>>();
    key.sort();
    key
}

fn bottle_run_count(bottle: &Bottle) -> usize {
    let fills = bottle.get_fills();

    if fills.is_empty() {
        return 0;
    }

    1 + fills.windows(2).filter(|pair| pair[0] != pair[1]).count()
}

fn total_run_count(bottles: &[Bottle]) -> usize {
    bottles.iter().map(bottle_run_count).sum()
}

fn target_solved_bottle_count(bottles: &[Bottle]) -> usize {
    bottles.iter().map(Bottle::get_fill_count).sum::<usize>() / FULL_BOTTLE_COUNT
}

fn estimate_remaining_moves(bottles: &[Bottle], target_solved_bottle_count: usize) -> usize {
    total_run_count(bottles).saturating_sub(target_solved_bottle_count)
}

fn reconstruct_moves(records: &[SearchRecord], mut record_index: usize) -> Vec<Move> {
    let mut moves = Vec::new();

    while let Some(parent_index) = records[record_index].parent {
        moves.push(records[record_index].via_move.expect("record should contain a move"));
        record_index = parent_index;
    }

    moves.reverse();
    moves
}

fn is_single_color_bottle(bottle: &Bottle) -> bool {
    let fills = bottle.get_fills();
    let hash_set = std::collections::HashSet::<&BottleColor>::from_iter(fills.iter());
    hash_set.len() == 1 && hash_set.iter().next() != Some(&&BottleColor::Mystery)
}

fn generate_possible_moves(bottles: &[Bottle]) -> Vec<(Move, Vec<Bottle>)> {
    let mut possible_moves = Vec::new();

    for source_idx in 0..bottles.len() {
        for destination_idx in 0..bottles.len() {
            if source_idx == destination_idx {
                continue;
            }

            let source_bottle = &bottles[source_idx];
            let destination_bottle = &bottles[destination_idx];

            if source_bottle.is_solved() || source_bottle.is_empty() || destination_bottle.is_solved() {
                continue;
            }

            if is_single_color_bottle(source_bottle) && destination_bottle.is_empty() {
                continue;
            }

            if !destination_bottle.can_fill_from(source_bottle) {
                continue;
            }

            let mut new_bottles = bottles.to_owned();
            let move_to_try = Move(source_idx, destination_idx);
            move_to_try.perform_move_on_bottles(&mut new_bottles);

            possible_moves.push((move_to_try, new_bottles));
        }
    }

    possible_moves
}

pub(crate) fn find_shortest_move_sequence<GoalFn>(
    bottles: Vec<Bottle>,
    mut is_goal: GoalFn,
) -> Option<Vec<Move>>
where
    GoalFn: FnMut(&[Bottle], usize) -> bool,
{
    let target_solved_bottle_count = target_solved_bottle_count(&bottles);
    let mut open_set = BinaryHeap::new();
    let mut records = Vec::new();
    let mut best_costs: HashMap<CanonicalStateKey, usize> = HashMap::new();

    let initial_cost = 0;
    let initial_estimate = estimate_remaining_moves(&bottles, target_solved_bottle_count);
    records.push(SearchRecord {
        state: bottles,
        cost: initial_cost,
        parent: None,
        via_move: None,
    });
    open_set.push(QueueEntry {
        estimated_total_cost: initial_estimate,
        cost: initial_cost,
        record_index: 0,
    });
    best_costs.insert(canonical_state_key(&records[0].state), initial_cost);

    while let Some(queue_entry) = open_set.pop() {
        let record_index = queue_entry.record_index;
        let record_cost = records[record_index].cost;
        let current_key = canonical_state_key(&records[record_index].state);

        if best_costs.get(&current_key).is_some_and(|best_cost| record_cost > *best_cost) {
            continue;
        }

        if is_goal(&records[record_index].state, record_cost) {
            return Some(reconstruct_moves(&records, record_index));
        }

        let mut possible_moves = generate_possible_moves(&records[record_index].state);
        sort_moves_by_heuristic(&mut possible_moves);

        for (move_to_try, next_state) in possible_moves {
            let next_cost = record_cost + 1;
            let next_key = canonical_state_key(&next_state);

            if best_costs.get(&next_key).is_some_and(|best_cost| next_cost >= *best_cost) {
                continue;
            }

            best_costs.insert(next_key, next_cost);

            let next_record_index = records.len();
            records.push(SearchRecord {
                state: next_state,
                cost: next_cost,
                parent: Some(record_index),
                via_move: Some(move_to_try),
            });

            let estimated_total_cost = next_cost
                + estimate_remaining_moves(
                    &records[next_record_index].state,
                    target_solved_bottle_count,
                );

            open_set.push(QueueEntry {
                estimated_total_cost,
                cost: next_cost,
                record_index: next_record_index,
            });
        }
    }

    None
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
    pub fn source_index(&self) -> usize {
        self.0
    }

    pub fn destination_index(&self) -> usize {
        self.1
    }

    pub fn perform_move_on_device(&self, layout: &BottleLayout) {
        click_at_position(water_sort_core::position::get_bottle_position(layout, self.0));
        click_at_position(water_sort_core::position::get_bottle_position(layout, self.1));
    }

    pub fn can_perform_on_bottles(&self, bottles: &[Bottle]) -> bool {
        if self.0 == self.1 {
            return false;
        }

        let Some(source_bottle) = bottles.get(self.0) else {
            return false;
        };

        let Some(destination_bottle) = bottles.get(self.1) else {
            return false;
        };

        destination_bottle.can_fill_from(source_bottle)
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
pub fn run_solver(bottles: &[Bottle]) -> Option<Vec<Move>> {
    println!("Solving puzzle with initial state: {:?}", bottles);

    find_shortest_move_sequence(bottles.to_vec(), |state, _move_count| {
        state.iter().all(|b| b.is_solved() || b.is_empty())
    })
}

pub fn get_possible_moves(
    bottles: &[Bottle],
    visited_states: &mut HashSet<Vec<Bottle>>,
) -> Vec<(Move, Vec<Bottle>)> {
    generate_possible_moves(bottles)
        .into_iter()
        .filter(|(_, new_bottles)| !visited_states.contains(new_bottles))
        .collect()
}

pub fn sort_moves_by_heuristic(possible_moves: &mut [(Move, Vec<Bottle>)]) {
    possible_moves.sort_by(|(_, a), (_, b)| {
        let a_solved = a.iter().filter(|x| x.is_solved()).count();
        let b_solved = b.iter().filter(|x| x.is_solved()).count();
        let a_runs = total_run_count(a);
        let b_runs = total_run_count(b);
        let a_non_empty = a.iter().filter(|bottle| !bottle.is_empty()).count();
        let b_non_empty = b.iter().filter(|bottle| !bottle.is_empty()).count();

        b_solved
            .cmp(&a_solved)
            .then_with(|| a_runs.cmp(&b_runs))
            .then_with(|| a_non_empty.cmp(&b_non_empty))
    })
}
