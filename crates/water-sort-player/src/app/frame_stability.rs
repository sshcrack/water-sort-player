use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use anyhow::Result;
use log::trace;
use opencv::core::{Mat, MatTraitConst, absdiff, mean, no_array};
use water_sort_core::bottles::detection::CROP_RECT;

const HISTORY_COVERAGE_TOLERANCE: Duration = Duration::from_millis(20);

pub enum MotionWindowState {
    Stable,
    WaitingForCoverage { missing_coverage: Duration },
    MovementDetected,
}

pub fn evaluate_motion_window(
    recent_frame_matches: &VecDeque<(Instant, bool)>,
    now: Instant,
    no_movement_window: Duration,
) -> MotionWindowState {
    let Some(cutoff) = now.checked_sub(no_movement_window) else {
        trace!("Current time is before no-movement window duration.");
        return MotionWindowState::WaitingForCoverage {
            missing_coverage: no_movement_window,
        };
    };

    let Some((oldest_timestamp, _)) = recent_frame_matches.front() else {
        trace!("No frame history available to determine stillness.");
        return MotionWindowState::WaitingForCoverage {
            missing_coverage: no_movement_window,
        };
    };

    // Require coverage for the full rolling window, allowing a small tolerance for frame timing jitter.
    if *oldest_timestamp > cutoff {
        let missing_coverage = oldest_timestamp.duration_since(cutoff);
        if missing_coverage > HISTORY_COVERAGE_TOLERANCE {
            /* trace!(
                "Not enough frame history to cover no-movement window. Oldest: {:?}, cutoff: {:?}, missing: {:?}.",
                oldest_timestamp,
                cutoff,
                missing_coverage
            ); */
            return MotionWindowState::WaitingForCoverage { missing_coverage };
        }
    }

    if recent_frame_matches.iter().any(|(_, is_match)| !*is_match) {
        return MotionWindowState::MovementDetected;
    }

    MotionWindowState::Stable
}

pub fn has_no_movement_in_window(
    recent_frame_matches: &VecDeque<(Instant, bool)>,
    now: Instant,
    no_movement_window: Duration,
) -> bool {
    matches!(
        evaluate_motion_window(recent_frame_matches, now, no_movement_window),
        MotionWindowState::Stable
    )
}

pub fn frames_are_identical(
    previous: &Mat,
    current: &Mat,
    mean_diff_threshold: f64,
) -> Result<bool> {
    if previous.size()? != current.size()? || previous.typ() != current.typ() {
        return Ok(false);
    }

    let previous = previous.roi(*CROP_RECT)?;
    let current = current.roi(*CROP_RECT)?;

    let mut diff = Mat::default();
    absdiff(&previous, &current, &mut diff)?;

    let channel_diffs = mean(&diff, &no_array())?;
    let channels = previous.channels();
    if channels <= 0 {
        return Ok(false);
    }

    let channels = channels.min(4) as usize;
    let mut total = 0.0;
    for idx in 0..channels {
        total += channel_diffs[idx];
    }
    let mean_abs_diff = total / channels as f64;

    Ok(mean_abs_diff <= mean_diff_threshold)
}
