use crate::constants::BottleColor;
use crate::position::Pos;
use opencv::core::{Mat, MatTraitConst, Vec3b};

#[derive(Debug, Clone, PartialEq)]
pub struct BottleLayout {
    pub name: String,
    pub positions: Vec<BottlePosition>,
}

#[derive(Debug, Clone, PartialEq)]
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

    /// Attempt to automatically detect the best layout for an image
    pub fn detect_layout(image: &Mat) -> anyhow::Result<Self> {
        let layouts = vec![
            BottleLayout::ten_bottle_layout(),
            BottleLayout::twelve_bottle_layout(),
        ];

        let mut best_layout = layouts[0].clone();
        let mut best_score = 0;

        for layout in layouts {
            let score = Self::score_layout_fit(image, &layout)?;
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
                        if BottleColor::from_pixel_value(*pixel).is_some()
                            || BottleColor::is_empty_pixel(pixel)
                        {
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
    /// Create the standard 10-bottle layout (2x5 grid)
    pub fn ten_bottle_layout() -> Self {
        let mut positions = Vec::new();

        // Constants from the original implementation
        let first_row_start = Pos(41, 223);
        let second_row_offset = Pos(0, 217);
        let bottle_spacing = Pos(69, 0);
        let layer_spacing = 35; // COLOR_CHECK_OFFSET.1

        // First row (5 bottles)
        for col in 0..5 {
            let base_pos = Pos(
                first_row_start.0 + col * bottle_spacing.0,
                first_row_start.1,
            );
            positions.push(BottlePosition::vertical(base_pos, layer_spacing, 4));
        }

        // Second row (5 bottles)
        for col in 0..5 {
            let base_pos = Pos(
                first_row_start.0 + col * bottle_spacing.0 + second_row_offset.0,
                first_row_start.1 + second_row_offset.1,
            );
            positions.push(BottlePosition::vertical(base_pos, layer_spacing, 4));
        }

        Self::new("10-bottles".to_string(), positions)
    }

    /// Create the 12-bottle layout (6 + 5 arrangement)
    pub fn twelve_bottle_layout() -> Self {
        let mut positions = Vec::new();

        // Measured from the 12-bottles screenshot.
        // Top row: 6 bottles
        let top_row_x = [72, 196, 321, 445, 569, 693];
        let top_row_y = 521;
        let top_layer_spacing = 58;
        for x in top_row_x {
            positions.push(BottlePosition::vertical(
                Pos(x, top_row_y),
                top_layer_spacing,
                4,
            ));
        }

        // Bottom row: 5 bottles
        let bottom_row_x = [119, 251, 383, 514, 646];
        let bottom_row_y = 936;
        let bottom_layer_offsets = vec![Pos(0, 0), Pos(0, 57), Pos(0, 114), Pos(0, 160)];
        for x in bottom_row_x {
            positions.push(BottlePosition::new(
                Pos(x, bottom_row_y),
                bottom_layer_offsets.clone(),
            ));
        }

        Self::new("12-bottles".to_string(), positions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bottle_position_vertical() {
        let base_pos = Pos(100, 200);
        let bottle_pos = BottlePosition::vertical(base_pos, 35, 4);

        assert_eq!(bottle_pos.base_pos, base_pos);
        assert_eq!(bottle_pos.layer_offsets.len(), 4);
        assert_eq!(bottle_pos.layer_offsets[0], Pos(0, 0)); // Top layer
        assert_eq!(bottle_pos.layer_offsets[1], Pos(0, 35)); // Second layer
        assert_eq!(bottle_pos.layer_offsets[2], Pos(0, 70)); // Third layer
        assert_eq!(bottle_pos.layer_offsets[3], Pos(0, 105)); // Bottom layer
    }

    #[test]
    fn test_layout_sample_position() {
        let positions = vec![
            BottlePosition::vertical(Pos(50, 200), 30, 3),
            BottlePosition::vertical(Pos(100, 200), 30, 3),
        ];
        let layout = BottleLayout::new("test".to_string(), positions);

        // First bottle, top layer
        assert_eq!(layout.get_sample_position(0, 0), Some(Pos(50, 200)));
        // First bottle, bottom layer
        assert_eq!(layout.get_sample_position(0, 2), Some(Pos(50, 260)));
        // Second bottle, middle layer
        assert_eq!(layout.get_sample_position(1, 1), Some(Pos(100, 230)));
        // Invalid indices
        assert_eq!(layout.get_sample_position(2, 0), None);
        assert_eq!(layout.get_sample_position(0, 3), None);
    }

    #[test]
    fn test_ten_bottle_layout() {
        let layout = BottleLayout::ten_bottle_layout();
        assert_eq!(layout.bottle_count(), 10);
        assert_eq!(layout.name, "10-bottles");

        // Test first bottle position
        let first_pos = layout.get_sample_position(0, 0).unwrap();
        assert_eq!(first_pos, Pos(41, 223));

        // Test last bottle position (second row, last column)
        let last_pos = layout.get_sample_position(9, 0).unwrap();
        assert_eq!(last_pos, Pos(41 + 4 * 69, 223 + 217));
    }

    #[test]
    fn test_twelve_bottle_layout() {
        let layout = BottleLayout::twelve_bottle_layout();
        assert_eq!(layout.bottle_count(), 11); // Actually 11 bottles visible in the image
        assert_eq!(layout.name, "12-bottles");
    }
}
