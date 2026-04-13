mod capture;

use std::collections::HashMap;

pub use capture::*;
use water_sort_core::{Bottle, BottleColor};

pub fn is_level_valid(initial: &[Bottle], resolved: &[Bottle]) -> bool {
    if initial.len() != resolved.len() {
        return false;
    }

    for (initial, resolved) in initial.iter().zip(resolved.iter()) {
        if !initial.is_hidden_and_empty() && !resolved.is_hidden_and_empty() {
            for (initial_color, resolved_color) in
                initial.get_fills().iter().zip(resolved.get_fills().iter())
            {
                if initial_color != &BottleColor::Mystery && initial_color != resolved_color {
                    log::debug!(
                        "Invalid level: initial color {:?} does not match resolved color {:?}",
                        initial_color,
                        resolved_color
                    );

                    return false;
                }
            }
        }

        if initial.hidden_requirement() != resolved.hidden_requirement() {
            log::debug!(
                "Invalid level: hidden requirements do not match (initial: {:?}, resolved: {:?})",
                initial.hidden_requirement(),
                resolved.hidden_requirement()
            );

            return false;
        }

        if initial.hidden_requirement().is_some() && !initial.is_empty() {
            log::debug!(
                "Invalid level: bottle has hidden requirement but is not empty (initial: {:?})",
                initial
            );

            return false;
        }

        if initial.is_hidden_and_empty() && resolved.is_hidden_and_empty() {
            log::debug!(
                "Invalid level: bottle is hidden and empty in both initial and resolved states (initial: {:?}, resolved: {:?})",
                initial,
                resolved
            );

            return false;
        }
    }

    let mut resolved_color_count: HashMap<BottleColor, usize> = HashMap::new();
    resolved.iter().for_each(|bottle| {
        bottle.get_fills().iter().for_each(|color| {
            if *color != BottleColor::Mystery {
                *resolved_color_count.entry(*color).or_insert(0) += 1;
            }
        });
    });

    let res = resolved_color_count.values().all(|&count| count % 4 == 0);
    if !res {
        log::debug!(
            "Invalid level: resolved state has colors that are not in multiples of 4 (resolved color count: {:?})",
            resolved_color_count
        );
    }

    res
}
