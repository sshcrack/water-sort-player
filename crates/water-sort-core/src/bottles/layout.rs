use anyhow::{Result, anyhow};
use opencv::{
    core::{Mat, MatTraitConst, Point, Rect, Vector},
    imgproc,
};

#[cfg(target_os = "windows")]
use opencv::core::AlgorithmHint;
use serde::Serialize;

use crate::Pos;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LayoutRow {
    pub start_position: Pos,
    pub spacing: Pos,
    pub bottle_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BottleLayout {
    friendly_name: &'static str,
    layer_spacing: i32,
    layer_count: usize,
    rows: Vec<LayoutRow>,
}

impl BottleLayout {
    fn new(
        friendly_name: &'static str,
        layer_spacing: i32,
        layer_count: usize,
        rows: Vec<LayoutRow>,
    ) -> Self {
        Self {
            friendly_name,
            layer_spacing,
            layer_count,
            rows,
        }
    }

    pub fn friendly_name(&self) -> &str {
        self.friendly_name
    }

    pub fn layer_spacing(&self) -> i32 {
        self.layer_spacing
    }

    pub fn layer_count(&self) -> usize {
        self.layer_count
    }

    pub fn rows(&self) -> &[LayoutRow] {
        &self.rows
    }

    pub fn bottle_count(&self) -> usize {
        self.rows.iter().map(|row| row.bottle_count).sum()
    }

    pub fn get_layouts() -> Vec<Self> {
        vec![
            Self::new(
                "ten_bottle_layout",
                35,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(41, 223),
                        spacing: Pos(69, 0),
                        bottle_count: 5,
                    },
                    LayoutRow {
                        start_position: Pos(41, 440),
                        spacing: Pos(69, 0),
                        bottle_count: 5,
                    },
                ],
            ),
            Self::new(
                "eleven_bottle_layout",
                31,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(34, 244),
                        spacing: Pos(58, 0),
                        bottle_count: 6,
                    },
                    LayoutRow {
                        start_position: Pos(56, 436),
                        spacing: Pos(61, 0),
                        bottle_count: 5,
                    },
                ],
            ),
            Self::new(
                "twelve_bottle_layout",
                31,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(34, 244),
                        spacing: Pos(58, 0),
                        bottle_count: 6,
                    },
                    LayoutRow {
                        start_position: Pos(34, 436),
                        spacing: Pos(58, 0),
                        bottle_count: 6,
                    },
                ],
            ),
            Self::new(
                "five_bottle_layout",
                35,
                4,
                vec![LayoutRow {
                    start_position: Pos(39, 336),
                    spacing: Pos(70, 0),
                    bottle_count: 5,
                }],
            ),
            Self::new(
                "six_bottle_layout",
                31,
                4,
                vec![LayoutRow {
                    start_position: Pos(32, 338),
                    spacing: Pos(59, 0),
                    bottle_count: 6,
                }],
            ),
            Self::new(
                "seven_bottle_layout",
                38,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(50, 207),
                        spacing: Pos(86, 0),
                        bottle_count: 4,
                    },
                    LayoutRow {
                        start_position: Pos(72, 444),
                        spacing: Pos(106, 0),
                        bottle_count: 3,
                    },
                ],
            ),
            Self::new(
                "twelve_bottle_layout_alternative",
                28,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(61, 156),
                        spacing: Pos(76, 0),
                        bottle_count: 4,
                    },
                    LayoutRow {
                        start_position: Pos(60, 340),
                        spacing: Pos(76, 0),
                        bottle_count: 4,
                    },
                    LayoutRow {
                        start_position: Pos(61, 528),
                        spacing: Pos(76, 0),
                        bottle_count: 4,
                    },
                ],
            ),
            Self::new(
                "eight_bottle_layout",
                35,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(49, 208),
                        spacing: Pos(84, 0),
                        bottle_count: 4,
                    },
                    LayoutRow {
                        start_position: Pos(51, 446),
                        spacing: Pos(84, 0),
                        bottle_count: 4,
                    },
                ],
            ),
            Self::new(
                "nine_bottle_layout",
                32,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(38, 225),
                        spacing: Pos(70, 0),
                        bottle_count: 5,
                    },
                    LayoutRow {
                        start_position: Pos(61, 440),
                        spacing: Pos(77, 0),
                        bottle_count: 4,
                    },
                ],
            ),
            Self::new(
                "nine_bottle_layout_alternative",
                33,
                4,
                vec![
                    LayoutRow {
                        start_position: Pos(63, 225),
                        spacing: Pos(77, 0),
                        bottle_count: 4,
                    },
                    LayoutRow {
                        start_position: Pos(39, 444),
                        spacing: Pos(70, 0),
                        bottle_count: 5,
                    },
                ],
            ),
        ]
    }

    pub fn get_click_position(&self, bottle_index: usize) -> Option<Pos> {
        let mut index = bottle_index;

        for row in &self.rows {
            if index < row.bottle_count {
                return Some(Pos(
                    row.start_position.0 + row.spacing.0 * index as i32,
                    row.start_position.1 + row.spacing.1 * index as i32,
                ));
            }
            index -= row.bottle_count;
        }

        None
    }

    pub fn get_sample_position(&self, bottle_index: usize, layer_index: usize) -> Option<Pos> {
        let click_position = self.get_click_position(bottle_index)?;
        let layer_offset = (self.layer_count.saturating_sub(1)).saturating_sub(layer_index) as i32;

        Some(Pos(
            click_position.0,
            click_position.1 - (self.layer_spacing * layer_offset),
        ))
    }

    pub fn bottle_positions(&self) -> Vec<Pos> {
        (0..self.bottle_count())
            .filter_map(|index| self.get_click_position(index))
            .collect()
    }

    pub fn detect_layout(frame_raw: &Mat) -> Result<Self> {
        let candidate_layouts = Self::get_layouts();
        let observed_pattern = detect_row_pattern(frame_raw)?;

        if let Some(layout) = candidate_layouts
            .iter()
            .find(|layout| layout.row_pattern() == observed_pattern)
        {
            return Ok(layout.clone());
        }

        candidate_layouts
            .into_iter()
            .min_by_key(|layout| {
                let expected = layout.bottle_count() as isize;
                let observed = observed_pattern.iter().sum::<usize>() as isize;
                (expected - observed).abs()
            })
            .ok_or_else(|| anyhow!("No bottle layouts are available"))
    }

    fn row_pattern(&self) -> Vec<usize> {
        self.rows.iter().map(|row| row.bottle_count).collect()
    }
}

fn crop_game_board(frame_raw: &Mat) -> Result<Mat> {
    let crop_rect = Rect::new(0, 143, 360, 544);
    let region = frame_raw.roi(crop_rect)?;
    Ok(region.try_clone()?)
}

fn detect_row_pattern(frame_raw: &Mat) -> Result<Vec<usize>> {
    let cropped = crop_game_board(frame_raw)?;
    let mut gray = Mat::default();
    cvt_color(&cropped, &mut gray, imgproc::COLOR_BGR2GRAY)?;

    let mut thresh = Mat::default();
    imgproc::threshold(&gray, &mut thresh, 127.0, 255.0, imgproc::THRESH_BINARY)?;

    let mut contours = Vector::<Vector<Point>>::new();
    imgproc::find_contours(
        &thresh,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),
    )?;

    let mut bounds = Vec::new();
    for contour in contours {
        bounds.push(imgproc::bounding_rect(&contour)?);
    }

    if bounds.is_empty() {
        return Ok(Vec::new());
    }

    let mut adjacency = vec![Vec::new(); bounds.len()];
    for i in 0..bounds.len() {
        for j in (i + 1)..bounds.len() {
            if boxes_overlap_vertically(bounds[i], bounds[j]) {
                adjacency[i].push(j);
                adjacency[j].push(i);
            }
        }
    }

    let mut visited = vec![false; bounds.len()];
    let mut row_counts = Vec::new();

    for index in 0..bounds.len() {
        if visited[index] {
            continue;
        }

        let mut stack = vec![index];
        visited[index] = true;
        let mut component = Vec::new();

        while let Some(current) = stack.pop() {
            component.push(current);
            for neighbor in &adjacency[current] {
                if !visited[*neighbor] {
                    visited[*neighbor] = true;
                    stack.push(*neighbor);
                }
            }
        }

        let min_y = component
            .iter()
            .map(|index| bounds[*index].y)
            .min()
            .unwrap_or_default();
        row_counts.push((min_y, component.len()));
    }

    row_counts.sort_by_key(|(min_y, _)| *min_y);
    Ok(row_counts.into_iter().map(|(_, count)| count).collect())
}

fn boxes_overlap_vertically(left: Rect, right: Rect) -> bool {
    let left_top = left.y;
    let left_bottom = left.y + left.height;
    let right_top = right.y;
    let right_bottom = right.y + right.height;

    !(left_bottom < right_top || right_bottom < left_top)
}

fn cvt_color(src: &Mat, dst: &mut Mat, code: i32) -> opencv::Result<()> {
    #[cfg(target_os = "windows")]
    {
        imgproc::cvt_color(src, dst, code, 0, AlgorithmHint::ALGO_HINT_DEFAULT)
    }

    #[cfg(not(target_os = "windows"))]
    {
        imgproc::cvt_color(src, dst, code, 0)
    }
}