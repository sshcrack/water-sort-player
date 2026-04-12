use lazy_static::lazy_static;
use opencv::core::{Scalar, Vec3b};

use crate::position::Pos;

pub const SCRCPY_SERVER_VERSION: &str = "3.3.4";
pub const SCRCPY_CONTROL_PORT: u16 = 27183;
pub const SCRCPY_DEVICE_SOCKET_NAME: &str = "scrcpy";
pub const SCRCPY_MAX_SIZE: u32 = 800;
pub const SCRCPY_MAX_FPS: u32 = 15;
pub const SCRCPY_VIDEO_BIT_RATE: u32 = 2_000_000;

pub const START_BUTTON_POS: Pos = Pos(186, 605);

pub const RETRY_BUTTON_POS: Pos = Pos(324, 57);

pub const FAILED_LEVEL_TEXT: Pos = Pos(170, 647);
pub const NEW_FEATURES_COLOR_POS: Pos = Pos(200, 88);

pub const Y_MEASURE_OFFSET: i32 = 0;
pub const X_MEASURE_OFFSET: i32 = 0;
lazy_static! {
    pub static ref NEXT_LEVEL_BUTTON_POSITIONS: Vec<Pos> = vec![Pos(184, 604), Pos(184, 675),];
    pub static ref NO_THANK_YOU_POSITIONS: Vec<Pos> = vec![Pos(177, 738), Pos(187, 737)];
    pub static ref FAILED_LEVEL_COLOR: Vec3b = vec3_from_hex("#f8d224");
    pub static ref NEXT_LEVEL_BUTTON_COLOR: Vec3b = vec3_from_hex("#B2CE39");
    pub static ref NO_THANK_YOU_REWARDS_COLOR: Vec3b = vec3_from_hex("#fbdcb1");
    pub static ref NEW_FEATURES_COLOR: Vec3b = vec3_from_hex("#F6BE33");
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BottleColor {
    Yellow,
    Red,
    Green,
    Lime,
    LightBlue,
    MediumBlue,
    Blue,
    Purple,
    Pink,
    Orange,
    Mystery,
}

impl BottleColor {
    pub fn to_char(self) -> char {
        match self {
            BottleColor::Yellow => 'Y',
            BottleColor::Red => 'R',
            BottleColor::Green => 'G',
            BottleColor::Lime => 'g',
            BottleColor::LightBlue => 'L',
            BottleColor::MediumBlue => 'M',
            BottleColor::Blue => 'B',
            BottleColor::Purple => 'P',
            BottleColor::Pink => 'W',
            BottleColor::Orange => 'O',
            BottleColor::Mystery => '?',
        }
    }

    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'Y' => Some(BottleColor::Yellow),
            'R' => Some(BottleColor::Red),
            'G' => Some(BottleColor::Green),
            'g' => Some(BottleColor::Lime),
            'L' => Some(BottleColor::LightBlue),
            'M' => Some(BottleColor::MediumBlue),
            'B' => Some(BottleColor::Blue),
            'P' => Some(BottleColor::Purple),
            'O' => Some(BottleColor::Orange),
            'W' => Some(BottleColor::Pink),
            '?' => Some(BottleColor::Mystery),
            _ => None,
        }
    }
}

pub fn color_distance_sq(pixel: &Vec3b, target: &Vec3b) -> u32 {
    let b_diff = pixel[0] as i32 - target[0] as i32;
    let g_diff = pixel[1] as i32 - target[1] as i32;
    let r_diff = pixel[2] as i32 - target[2] as i32;

    (b_diff * b_diff + g_diff * g_diff + r_diff * r_diff) as u32
}

pub fn vec3_from_hex(hex: &str) -> Vec3b {
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap();
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap();
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap();
    Vec3b::from([b, g, r])
}

pub fn scalar_from_hex(hex: &str) -> Scalar {
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap();
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap();
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap();
    Scalar::from([b as f64, g as f64, r as f64, 0.0])
}

lazy_static! {
    pub static ref COLOR_VALUES: Vec<(BottleColor, Vec3b)> = vec![
        (BottleColor::Yellow, vec3_from_hex("#fbdf20")),
        (BottleColor::Red, vec3_from_hex("#df1a24")),
        (BottleColor::Green, vec3_from_hex("#46de1e")),
        (BottleColor::Lime, vec3_from_hex("#a3ed33")),
        (BottleColor::LightBlue, vec3_from_hex("#2cf8fe")),
        (BottleColor::MediumBlue, vec3_from_hex("#52b7fb")),
        (BottleColor::Blue, vec3_from_hex("#194af9")),
        (BottleColor::Purple, vec3_from_hex("#8c00d9")),
        (BottleColor::Pink, vec3_from_hex("#d212cc")),
        (BottleColor::Orange, vec3_from_hex("#f37c1c"))
    ];
    pub static ref FAILED_LEVEL_EMPTY_COLOR: Vec3b = vec3_from_hex("#331c14");
    pub static ref EMPTY_COLORS: Vec<Vec3b> = vec![
        vec3_from_hex("#371D16"),
        vec3_from_hex("#713d2c"),
        vec3_from_hex("#a9674e")
    ];
}

pub const COLOR_DISTANCE_THRESHOLD_SQ: u32 = 50 * 50;
impl BottleColor {
    pub fn from_pixel_value(pixel: Vec3b, has_failed_level: bool) -> Option<Self> {
        if Self::is_empty_pixel(&pixel, has_failed_level) {
            return None;
        }

        if Self::is_mystery_pixel(&pixel) {
            return Some(BottleColor::Mystery);
        }

        let mut best_match = None;
        for (color, target_pixel) in COLOR_VALUES.iter() {
            let dist = color_distance_sq(&pixel, target_pixel);
            if best_match.is_none_or(|(_, best_dist)| dist < best_dist) {
                best_match = Some((*color, dist));
            }
        }

        match best_match {
            Some((color, dist)) if dist <= COLOR_DISTANCE_THRESHOLD_SQ || has_failed_level => {
                Some(color)
            }
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn values() -> Vec<BottleColor> {
        COLOR_VALUES.iter().map(|(color, _)| *color).collect()
    }

    pub fn is_empty_pixel(pixel: &Vec3b, has_failed_level: bool) -> bool {
        for color in EMPTY_COLORS.iter() {
            if color_distance_sq(pixel, color) <= COLOR_DISTANCE_THRESHOLD_SQ {
                return true;
            }
        }
        if has_failed_level
            && color_distance_sq(pixel, &FAILED_LEVEL_EMPTY_COLOR) <= COLOR_DISTANCE_THRESHOLD_SQ
        {
            return true;
        }

        false
    }

    fn is_mystery_pixel(pixel: &Vec3b) -> bool {
        let b = pixel[0] as i32;
        let g = pixel[1] as i32;
        let r = pixel[2] as i32;

        let max_channel = b.max(g).max(r);
        let min_channel = b.min(g).min(r);
        let channel_spread = max_channel - min_channel;
        let average_brightness = (b + g + r) / 3;

        // Mystery liquid appears as medium/bright neutral gray in screenshots.
        channel_spread <= 20 && average_brightness >= 85
    }

    pub fn to_pixel_value(self) -> Vec3b {
        if self == BottleColor::Mystery {
            // Mystery color is a medium gray - not an actual color in the game, but useful for testing
            return vec3_from_hex("#4f4f4f");
        }

        COLOR_VALUES
            .iter()
            .find(|(color, _)| *color == self)
            .map(|(_, pixel)| *pixel)
            .unwrap()
    }
}
