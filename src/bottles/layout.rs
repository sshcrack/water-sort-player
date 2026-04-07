use crate::constants::BottleColor;
use crate::position::Pos;
use opencv::core::{Mat, MatTraitConst, Vec3b};

const Y_MEASURE_OFFSET: i32 = -5;
const X_MEASURE_OFFSET: i32 = 6;

/// Macro to create bottle layouts from a declarative specification.
///
/// # Parameters
/// - `$name`: Layout name (as string literal)
/// - `$layer_spacing`: Vertical pixel spacing between color layers within a bottle
/// - `$layer_count`: Number of color layers per bottle (typically 4)
/// - `$(row_specs)+`: One or more row specifications in the format: `(start_pos, spacing, count)`
///   - `start_pos`: `Pos(x, y)` - top-left position of the first bottle in the row
///   - `spacing`: `Pos(dx, dy)` - offset between consecutive bottles in the row
///   - `count`: number of bottles in this row
///
/// # Example
/// ```ignore
/// bottle_layout!(
///     "my-layout",
///     35,  // layer_spacing
///     4,   // layer_count
///     (Pos(41, 223), Pos(69, 0), 5),   // First row: 5 bottles, 69px apart
///     (Pos(41, 440), Pos(69, 0), 5),   // Second row: 5 bottles, 69px apart
/// )
/// ```
macro_rules! bottle_layout {
    ($name:expr, $layer_spacing:expr, $layer_count:expr, $(($start:expr, $spacing:expr, $count:expr)),+ $(,)?) => {{
        let mut positions = Vec::new();
        $(
            for col in 0..$count {
                let base_pos = Pos(
                    $start.0 + col * $spacing.0 + X_MEASURE_OFFSET,
                    $start.1 + col * $spacing.1 + Y_MEASURE_OFFSET,
                );
                positions.push(BottlePosition::vertical(base_pos, $layer_spacing, $layer_count));
            }
        )+
        BottleLayout::new($name.to_string(), positions)
    }};
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct BottleLayout {
    pub name: String,
    pub positions: Vec<BottlePosition>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BottlePosition {
    /// Starting position for bottle detection (top layer)
    pub base_pos: Pos,
    /// Offsets for each color layer (from top to bottom)
    pub layer_offsets: Vec<Pos>,
}

impl BottleLayout {
    /// Create a new layout with the given name and positions
    pub fn new(name: String, positions: Vec<BottlePosition>) -> Self {
        Self { name, positions }
    }

    /// Get the number of bottles in this layout
    pub fn bottle_count(&self) -> usize {
        self.positions.len()
    }

    /// Get the position for sampling a specific color layer in a bottle
    pub fn get_sample_position(&self, bottle_index: usize, layer_index: usize) -> Option<Pos> {
        let bottle_pos = self.positions.get(bottle_index)?;
        let layer_offset = bottle_pos.layer_offsets.get(layer_index)?;

        Some(Pos(
            bottle_pos.base_pos.0 + layer_offset.0,
            bottle_pos.base_pos.1 + layer_offset.1,
        ))
    }

    /// Get a safe click position within a bottle (use middle layer by default)
    pub fn get_click_position(&self, bottle_index: usize) -> Option<Pos> {
        let bottle_pos = self.positions.get(bottle_index)?;
        if bottle_pos.layer_offsets.is_empty() {
            return Some(bottle_pos.base_pos);
        }

        let mid_index = bottle_pos.layer_offsets.len() / 2;
        let layer_offset = bottle_pos.layer_offsets.get(mid_index)?;
        Some(Pos(
            bottle_pos.base_pos.0 + layer_offset.0,
            bottle_pos.base_pos.1 + layer_offset.1,
        ))
    }


    /// Attempt to automatically detect the best layout for an image
    pub fn detect_layout(image: &Mat) -> anyhow::Result<Self> {
        let layouts = Self::get_layouts();

        let mut best_layout = layouts[0].clone();
        let mut best_score = 0;

        for layout in layouts {
            let score = Self::score_layout_fit(image, &layout)?;
            println!("Layout '{}' fit score: {}", layout.name, score);
            if score > best_score {
                best_score = score;
                best_layout = layout;
            }
        }

        Ok(best_layout)
    }

    /// Score how well a layout fits an image based on detected bottles
    fn score_layout_fit(image: &Mat, layout: &BottleLayout) -> anyhow::Result<usize> {
        let mut score = 0;

        for bottle_idx in 0..layout.bottle_count() {
            let mut bottle_has_content = false;

            // Check if any layer in this bottle has color content
            for layer_idx in 0..4 {
                if let Some(sample_pos) = layout.get_sample_position(bottle_idx, layer_idx) {
                    let x = sample_pos.0;
                    let y = sample_pos.1;

                    // Check if coordinates are within image bounds
                    if y >= 0 && y < image.rows() && x >= 0 && x < image.cols() {
                        let pixel = image.at_2d::<Vec3b>(y, x)?;

                        // Check if pixel looks like bottle content (not empty background)
                        if BottleColor::from_pixel_value(*pixel).is_some() {
                            bottle_has_content = true;
                            break;
                        }
                    }
                }
            }

            if bottle_has_content {
                score += 1;
            }
        }

        Ok(score)
    }
}

impl BottlePosition {
    /// Create a new bottle position
    pub fn new(base_pos: Pos, layer_offsets: Vec<Pos>) -> Self {
        Self {
            base_pos,
            layer_offsets,
        }
    }

    /// Create a standard vertical bottle with equally spaced layers
    pub fn vertical(base_pos: Pos, layer_spacing: i32, layer_count: usize) -> Self {
        let layer_offsets = (0..layer_count)
            .map(|i| Pos(0, i as i32 * layer_spacing))
            .collect();

        Self::new(base_pos, layer_offsets)
    }
}

/// Predefined layouts
impl BottleLayout {
    pub fn get_layouts() -> Vec<Self> {
        vec![Self::ten_bottle_layout(), Self::eleven_bottle_layout(), Self::twelve_bottle_layout()]
    }

    /// Create the standard 10-bottle layout (2x5 grid)
    pub fn ten_bottle_layout() -> Self {
        bottle_layout!(
            "10-bottles", // Layout name
            35,           // Layer spacing (pixels between color layers)
            4,            // Layer count (4 colors per bottle)
            // Row 1: 5 bottles at y=223, starting at x=41, spaced 69px apart
            (Pos(41, 223), Pos(69, 0), 5),
            // Row 2: 5 bottles at y=440, starting at x=41, spaced 69px apart
            (Pos(41, 440), Pos(69, 0), 5),
        )
    }

    /// Create the 11-bottle layout (6 + 5 arrangement)
    pub fn eleven_bottle_layout() -> Self {
        bottle_layout!(
            "11-bottles", // Layout name
            31,           // Layer spacing (pixels between color layers)
            4,            // Layer count (4 colors per bottle)
            // Row 1: 6 bottles at y=244, starting at x=34, spaced 58px apart
            (Pos(34, 244), Pos(58, 0), 6),
            // Row 2: 5 bottles at y=436, starting at x=56, spaced 58px apart
            (Pos(56, 436), Pos(58, 0), 5),
        )
    }

    pub fn twelve_bottle_layout() -> BottleLayout {
        bottle_layout!(
            "12-bottles",
            31,
            4,
            (Pos(34, 244), Pos(58, 0), 6),
            (Pos(34, 436), Pos(58, 0), 6),
        )
    }
}
