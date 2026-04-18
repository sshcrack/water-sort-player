use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Display;

use anyhow::Result;
#[cfg(feature = "discovery-debugging")]
use log::debug;
use log::info;
use serde::Serialize;
use water_sort_core::Pos;
use water_sort_core::{
    bottles::{Bottle, HiddenRequirement},
    constants::BottleColor,
};
use water_sort_device::CaptureDeviceBackend;

pub mod discovery;

/// Indicates the move to perform: pour from bottle at index 0 to bottle at index 1
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Move {
    source_idx: usize,
    source_clickable_pos: Option<Pos>,

    destination_idx: usize,
    destination_clickable_pos: Option<Pos>,
}

impl Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}->{}", self.source_idx, self.destination_idx)
    }
}

#[cfg(feature = "solver-visualization")]
pub struct SolverProgressSnapshot<'a> {
    pub state: &'a [Bottle],
    pub explored_states: usize,
    pub queue_len: usize,
    pub depth: usize,
    pub is_goal: bool,
}

#[cfg(feature = "solver-visualization")]
type SolverProgressCallback<'a> = &'a mut dyn FnMut(SolverProgressSnapshot<'_>);
/// Arguments are old state and new state
type IsStateValidFn<'a> = &'a dyn Fn(&[Bottle], &[Bottle]) -> bool;

pub mod visualization;

const FULL_BOTTLE_COUNT: usize = 4;

type CanonicalStateKey = Vec<(HiddenRequirement, Vec<BottleColor>)>;

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
        .map(|bottle| {
            (
                bottle.hidden_requirement_state(),
                bottle.get_fills().clone(),
            )
        })
        .collect::<Vec<_>>();
    key.sort();
    key
}

fn bottle_run_count(bottle: &Bottle) -> usize {
    let fills = bottle
        .get_fills()
        .into_iter()
        .filter(|color| *color != BottleColor::Empty)
        .collect::<Vec<_>>();

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
        log::debug!(
            "Reconstructing move {} for record {} with state: {}",
            records[record_index]
                .via_move
                .as_ref()
                .expect("record should contain a move"),
            record_index,
            records[record_index]
                .state
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );

        moves.push(
            records[record_index]
                .via_move
                .clone()
                .expect("record should contain a move"),
        );
        record_index = parent_index;
    }

    moves.reverse();
    moves
}

fn is_single_color_bottle(bottle: &Bottle) -> bool {
    let fills = bottle
        .get_fills()
        .into_iter()
        .filter(|color| *color != BottleColor::Empty)
        .collect::<Vec<_>>();

    if fills.is_empty() {
        return false;
    }

    let hash_set = std::collections::HashSet::<BottleColor>::from_iter(fills.into_iter());
    hash_set.len() == 1 && !hash_set.contains(&BottleColor::Mystery)
}

fn unlock_hidden_bottles_with_solved_colors(bottles: &mut [Bottle]) {
    let solved_colors = bottles
        .iter()
        .filter_map(Bottle::solved_color)
        .collect::<HashSet<_>>();

    if solved_colors.is_empty() {
        return;
    }

    for bottle in bottles.iter_mut() {
        if bottle
            .get_locked_hidden_requirement()
            .is_some_and(|required_color| solved_colors.contains(&required_color))
        {
            bottle.unlock_hidden_requirement();
        }
    }
}

fn generate_possible_moves(bottles: &[Bottle]) -> Vec<(Move, Vec<Bottle>)> {
    let mut possible_moves = Vec::new();

    let solved_bottles = bottles
        .iter()
        .filter_map(|b| b.solved_color())
        .collect::<Vec<_>>();

    for source_idx in 0..bottles.len() {
        for destination_idx in 0..bottles.len() {
            if source_idx == destination_idx {
                /* log::trace!("Skipping move from bottle {} to itself", source_idx); */
                continue;
            }

            let source_bottle = &bottles[source_idx];
            let destination_bottle = &bottles[destination_idx];

            if source_bottle.is_solved()
                || source_bottle.is_empty()
                || source_bottle.is_hidden_and_locked()
                || destination_bottle.is_solved()
                || destination_bottle.is_hidden_and_locked()
            {
                /* log::trace!(
                    "Skipping move from bottle {} to bottle {} because one of them is solved, empty, or hidden and empty",
                    source_idx,
                    destination_idx
                );
                log::trace!("Source bottle: {:?}", source_bottle);
                log::trace!("Destination bottle: {:?}", destination_bottle); */
                continue;
            }

            if let Some(source_req) = source_bottle.get_locked_hidden_requirement()
                && !solved_bottles.contains(&source_req)
            {
                /* log::trace!(
                    "Skipping move from bottle {} to bottle {} because source bottle has hidden requirement {:?} that is not yet solved",
                    source_idx,
                    destination_idx,
                    source_req
                ); */
                continue;
            }

            if let Some(destination_req) = destination_bottle.get_locked_hidden_requirement()
                && !solved_bottles.contains(&destination_req)
            {
                /* log::trace!(
                    "Skipping move from bottle {} to bottle {} because destination bottle has hidden requirement {:?} that is not yet solved",
                    source_idx,
                    destination_idx,
                    destination_req
                ); */
                continue;
            }

            if is_single_color_bottle(source_bottle) && destination_bottle.is_empty() {
                /* log::trace!(
                    "Skipping move from bottle {} to bottle {} because source is single-color and destination is empty",
                    source_idx,
                    destination_idx
                ); */

                continue;
            }

            if !destination_bottle.can_fill_from(source_bottle) {
                /* log::trace!(
                    "Skipping move from bottle {} to bottle {} because it cannot fill from the source",
                    source_idx,
                    destination_idx
                ); */
                continue;
            }

            let mut new_bottles = bottles.to_owned();
            let move_to_try = Move {
                source_idx,
                source_clickable_pos: *source_bottle.click_position(),
                destination_idx,
                destination_clickable_pos: *destination_bottle.click_position(),
            };
            if !move_to_try.can_perform_on_bottles(&new_bottles) {
                /* log::trace!(
                    "Skipping move from bottle {} to bottle {} because it cannot be performed on the current state",
                    source_idx,
                    destination_idx
                ); */
                continue;
            }

            move_to_try.perform_move_on_bottles(&mut new_bottles);
            possible_moves.push((move_to_try, new_bottles));
        }
    }

    possible_moves
}

pub(crate) fn find_shortest_move_sequence<GoalFn>(
    mut bottles: Vec<Bottle>,
    is_state_valid: Option<IsStateValidFn>,
    mut is_goal: GoalFn,
    #[cfg(feature = "solver-visualization")] mut on_progress: Option<SolverProgressCallback<'_>>,
) -> Option<Vec<Move>>
where
    GoalFn: FnMut(&[Bottle], usize) -> bool,
{
    let clone_before = bottles.clone();
    unlock_hidden_bottles_with_solved_colors(&mut bottles);
    if let Some(is_state_valid) = is_state_valid.as_ref()
        && !is_state_valid(&clone_before, &bottles)
    {
        return None;
    }

    let target_solved_bottle_count = target_solved_bottle_count(&bottles);
    let mut open_set = BinaryHeap::new();
    let mut records = Vec::new();
    let mut best_costs: HashMap<CanonicalStateKey, usize> = HashMap::new();
    #[cfg(feature = "solver-visualization")]
    let mut explored_states = 0usize;

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

        if best_costs
            .get(&current_key)
            .is_some_and(|best_cost| record_cost > *best_cost)
        {
            continue;
        }

        #[cfg(feature = "solver-visualization")]
        {
            explored_states += 1;
        }

        let goal_reached = is_goal(&records[record_index].state, record_cost);

        #[cfg(feature = "solver-visualization")]
        if let Some(progress_callback) = on_progress.as_mut() {
            progress_callback(SolverProgressSnapshot {
                state: &records[record_index].state,
                explored_states,
                queue_len: open_set.len(),
                depth: record_cost,
                is_goal: goal_reached,
            });
        }

        if goal_reached {
            return Some(reconstruct_moves(&records, record_index));
        }

        let mut possible_moves = generate_possible_moves(&records[record_index].state);

        sort_moves_by_heuristic(&mut possible_moves);

        for (move_to_try, mut next_state) in possible_moves {
            unlock_hidden_bottles_with_solved_colors(&mut next_state);
            if let Some(is_state_valid) = is_state_valid.as_ref()
                && !is_state_valid(&records[record_index].state, &next_state)
            {
                continue;
            }

            let next_cost = record_cost + 1;
            let next_key = canonical_state_key(&next_state);

            if best_costs
                .get(&next_key)
                .is_some_and(|best_cost| next_cost >= *best_cost)
            {
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
        self.source_idx
    }

    pub fn destination_index(&self) -> usize {
        self.destination_idx
    }

    pub fn source_clickable_pos(&self) -> Option<Pos> {
        self.source_clickable_pos
    }

    pub fn destination_clickable_pos(&self) -> Option<Pos> {
        self.destination_clickable_pos
    }

    pub fn perform_move_on_device<B: CaptureDeviceBackend>(&self, device: &B) -> Result<()> {
        device.click_at_position(
            self.source_clickable_pos
                .expect("Source clickable pos must be set to perform move on device"),
        )?;
        device.click_at_position(
            self.destination_clickable_pos
                .expect("Destination clickable pos must be set to perform move on device"),
        )?;
        Ok(())
    }

    pub fn can_perform_on_bottles(&self, bottles: &[Bottle]) -> bool {
        if self.source_idx == self.destination_idx {
            return false;
        }

        let Some(source_bottle) = bottles.get(self.source_idx) else {
            return false;
        };

        let Some(destination_bottle) = bottles.get(self.destination_idx) else {
            return false;
        };

        destination_bottle.can_fill_from(source_bottle)
    }

    pub fn perform_move_on_bottles(&self, bottles: &mut [Bottle]) {
        let (source_bottle, destination_bottle) =
            get_two_mut_from_vec(bottles, self.source_idx, self.destination_idx);
        if !destination_bottle.can_fill_from(source_bottle) {
            #[cfg(feature = "discovery-debugging")]
            {
                debug!(
                    "Invalid move: cannot pour from bottle {} to bottle {}",
                    self.source_idx, self.destination_idx
                );
                debug!("Source bottle: {:?}", source_bottle);
                debug!("Destination bottle: {:?}", destination_bottle);
                std::io::stdin().read_line(&mut String::new()).unwrap();
            }
            panic!(
                "Invalid move: cannot pour from bottle {} to bottle {}",
                self.source_idx, self.destination_idx
            );
        }

        destination_bottle.fill_from(source_bottle);
    }
}

pub fn build_solver_initial_bottle_state(
    max_revealed_bottles: &[Bottle],
    initial_bottles: &[Bottle],
) -> Vec<Bottle> {
    let mut bottles = Vec::with_capacity(max_revealed_bottles.len());

    max_revealed_bottles
        .iter()
        .zip(initial_bottles.iter())
        .for_each(|(max_revealed_bottle, initial_bottle)| {
            let mut new_bottle = Bottle::from_fills_with_initial(
                max_revealed_bottle.get_fills().clone(),
                initial_bottle.get_fills().clone(),
                *initial_bottle.click_position(),
            );

            new_bottle.set_hidden_requirement(max_revealed_bottle.hidden_requirement_state());
            new_bottle.lock_hidden_requirement();
            bottles.push(new_bottle);
        });

    bottles
}

#[allow(dead_code)]
pub fn run_solver(
    max_revealed_bottle_state: &[Bottle],
    initial_state: &[Bottle],
) -> Option<Vec<Move>> {
    let bottles = build_solver_initial_bottle_state(max_revealed_bottle_state, initial_state);

    info!(
        "Solving puzzle with build bottle state: {}",
        bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );

    find_shortest_move_sequence(
        bottles.to_vec(),
        None,
        |state, _move_count| {
            state
                .iter()
                .all(|b| b.is_solved() || (b.is_empty() && !b.is_hidden_and_locked()))
        },
        #[cfg(feature = "solver-visualization")]
        None,
    )
}

#[cfg(feature = "solver-visualization")]
pub fn run_solver_with_progress<ProgressFn>(
    max_revealed_bottle_state: &[Bottle],
    initial_state: &[Bottle],
    mut on_progress: ProgressFn,
) -> Option<Vec<Move>>
where
    ProgressFn: FnMut(SolverProgressSnapshot<'_>),
{
    log::debug!("Solver run:");
    log::debug!(
        "Initial state for solver: {}",
        initial_state
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    log::debug!(
        "Max revealed bottle state for solver: {}",
        max_revealed_bottle_state
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let bottles = build_solver_initial_bottle_state(max_revealed_bottle_state, initial_state);

    info!(
        "Solving puzzle with build bottle state: {}",
        bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );

    find_shortest_move_sequence(
        bottles.to_vec(),
        None,
        |state, _move_count| {
            state
                .iter()
                .all(|b| b.is_solved() || (b.is_empty() && !b.is_hidden_and_locked()))
        },
        Some(&mut on_progress),
    )
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

#[cfg(test)]
mod tests {
    use crate::{
        Move,
        discovery::{find_best_hidden_unlock_moves, improve_best_revealed_state, improve_current_and_initial_bottles_with_revealed_state},
    };

    use super::{find_shortest_move_sequence, unlock_hidden_bottles_with_solved_colors};
    use water_sort_core::{
        bottles::{HiddenRequirement, test_utils::TestUtils},
        constants::BottleColor,
    };

    #[test]
    fn clears_hidden_requirement_when_required_color_is_solved() {
        let mut bottles = TestUtils::parse_bottles_sequence("!R RRRR EEEE EEEE");

        unlock_hidden_bottles_with_solved_colors(&mut bottles);

        assert!(!bottles[0].is_hidden_and_locked());
        assert_eq!(
            bottles[0].hidden_requirement_state(),
            HiddenRequirement::Unlocked(BottleColor::red())
        );
    }

    #[test]
    fn solver_can_finish_when_unlocking_hidden_empty_bottle() {
        let bottles = TestUtils::parse_bottles_sequence("!R RRRR G GGG EEEE EEEE EEEE EEEE");

        let solution = find_shortest_move_sequence(
            bottles,
            None,
            |state, _move_count| {
                state
                    .iter()
                    .all(|b| b.is_solved() || (b.is_empty() && !b.is_hidden_and_locked()))
            },
            #[cfg(feature = "solver-visualization")]
            None,
        );

        assert!(solution.is_some());
        assert_eq!(solution.expect("solver should return a solution").len(), 1);
    }

    #[test_log::test]
    fn solve_tester() {
        let (mut initial_state, mut max_revealed_bottle_state) = TestUtils::load_bottles_from_state(
            "C:\\Users\\hendr\\Documents\\Coding\\rust\\water-sort-player\\target\\release\\save-states\\1776447073776\\level-0001\\0028-HiddenDiscoverBottles.json",
        );
        let mut current_bottles = initial_state.clone();

        println!("Initial: {}", initial_state.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(" "));
        println!("Max revealed: {}", max_revealed_bottle_state.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(" "));
        improve_current_and_initial_bottles_with_revealed_state(
            &mut current_bottles,
            &mut initial_state,
            &max_revealed_bottle_state,
        );
        improve_best_revealed_state(&mut max_revealed_bottle_state, &initial_state, &current_bottles);

        let x = find_best_hidden_unlock_moves(&current_bottles);
        match x {
            crate::discovery::DiscoverResult::NoMove => panic!("NoMove"),
            crate::discovery::DiscoverResult::MoveToDiscover(items) => {
                for m in items {
                    println!("{}", m);
                }
            }
            crate::discovery::DiscoverResult::AlreadySolved => println!("Already solved"),
        }
    }
}
