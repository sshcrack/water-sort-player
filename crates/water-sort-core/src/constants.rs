use std::fmt::Display;

use colored::Colorize;
use lazy_static::lazy_static;
use opencv::core::{Scalar, Vec3b};
use serde::Serialize;

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
pub const COLOR_DISTANCE_THRESHOLD_SQ: u32 = 50 * 50;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BottleColor {
    Empty,
    Mystery,
    /// The order is b,g,r to match the OpenCV BGR format
    Fill((u8, u8, u8)),
}

impl BottleColor {
    pub fn from_hex(hex: &str) -> Self {
        let vec = vec3_from_hex(hex);
        BottleColor::Fill((vec[0], vec[1], vec[2]))
    }
    pub fn to_hex(&self) -> String {
        match self {
            BottleColor::Empty => "E".into(),
            BottleColor::Mystery => "?".into(),
            BottleColor::Fill((b, g, r)) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        }
    }

    pub fn to_pixel_value(&self) -> Vec3b {
        match self {
            BottleColor::Empty => FAILED_LEVEL_EMPTY_COLOR.clone(),
            BottleColor::Mystery => Vec3b::from([0, 0, 0]),
            BottleColor::Fill((b, g, r)) => Vec3b::from([*b, *g, *r]),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, BottleColor::Empty)
    }
}

impl BottleColor {
    pub fn yellow() -> Self {
        Self::from_hex("#fbdf20")
    }

    pub fn red() -> Self {
        Self::from_hex("#df1a24")
    }

    pub fn green() -> Self {
        Self::from_hex("#46de1e")
    }

    pub fn light_green() -> Self {
        Self::from_hex("#a3ed33")
    }

    pub fn light_blue() -> Self {
        Self::from_hex("#2cf8fe")
    }

    pub fn medium_blue() -> Self {
        Self::from_hex("#52b7fb")
    }

    pub fn blue() -> Self {
        Self::from_hex("#194af9")
    }

    pub fn purple() -> Self {
        Self::from_hex("#8c00d9")
    }

    pub fn pink() -> Self {
        Self::from_hex("#d212cc")
    }

    pub fn orange() -> Self {
        Self::from_hex("#f37c1c")
    }

    pub fn values() -> Vec<Self> {
        vec![
            Self::yellow(),
            Self::red(),
            Self::green(),
            Self::light_green(),
            Self::light_blue(),
            Self::medium_blue(),
            Self::blue(),
            Self::purple(),
            Self::pink(),
            Self::orange(),
        ]
    }
}

impl Display for BottleColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BottleColor::Empty => write!(f, "E"),
            BottleColor::Mystery => write!(f, "?"),
            BottleColor::Fill((b, g, r)) => {
                write!(f, "{}", "█".on_truecolor(*r, *g, *b))
            }
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
    pub static ref FAILED_LEVEL_EMPTY_COLOR: Vec3b = vec3_from_hex("#331c14");

    /// Used for legacy to new system migration, maps the chars to the hex values and vice versa
    pub static ref COLOR_VALUES: Vec<(char, Vec3b)> = vec![
        ('Y', vec3_from_hex("#fbdf20")),
        ('R', vec3_from_hex("#df1a24")),
        ('G', vec3_from_hex("#46de1e")),
        ('g', vec3_from_hex("#a3ed33")),
        ('L', vec3_from_hex("#2cf8fe")),
        ('M', vec3_from_hex("#52b7fb")),
        ('B', vec3_from_hex("#194af9")),
        ('D', vec3_from_hex("#434A8E")),
        ('P', vec3_from_hex("#8c00d9")),
        ('W', vec3_from_hex("#d212cc")),
        ('O', vec3_from_hex("#f37c1c"))
    ];
}
