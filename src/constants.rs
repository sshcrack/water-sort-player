use lazy_static::lazy_static;
use opencv::core::Vec3b;

use crate::position::Pos;

pub const VIRTUAL_CAM: &str = "/dev/video10";

pub const START_BUTTON_POS: Pos = Pos(186, 605);

pub const FIRST_ROW_START_POS: Pos = Pos(41, 223);
pub const SECOND_ROW_OFFSET: Pos = Pos(0, 217);
pub const BOTTLE_SPACING: Pos = Pos(69, 0);

pub const NEXT_LEVEL_BUTTON_POS: Pos = Pos(184, 604);

pub const NO_THANK_YOU_REWARDS_POS: Pos = Pos(187, 737);
lazy_static! {
    pub static ref NEXT_LEVEL_BUTTON_COLOR: Vec3b = vec3_from_hex("#eff6e2");
    pub static ref NO_THANK_YOU_REWARDS_COLOR: Vec3b = vec3_from_hex("#fbdcb1");
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BottleColor {
    Yellow,
    Red,
    Green,
    LightBlue,
    MediumBlue,
    Blue,
    Purple,
    Pink,
    Orange,
    Mystery,
}

pub fn is_color_within_tolerance(pixel: &Vec3b, target: &Vec3b, tolerance: u8) -> bool {
    let b_diff = (pixel[0] as i16 - target[0] as i16).unsigned_abs() as u8;
    let g_diff = (pixel[1] as i16 - target[1] as i16).unsigned_abs() as u8;
    let r_diff = (pixel[2] as i16 - target[2] as i16).unsigned_abs() as u8;

    b_diff <= tolerance && g_diff <= tolerance && r_diff <= tolerance
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

lazy_static! {
    pub static ref COLOR_VALUES: Vec<(BottleColor, Vec3b)> = vec![
        (BottleColor::Yellow, vec3_from_hex("#fbdf20")),
        (BottleColor::Red, vec3_from_hex("#df1a24")),
        (BottleColor::Green, vec3_from_hex("#46de1e")),
        (BottleColor::LightBlue, vec3_from_hex("#2cf8fe")),
        (BottleColor::MediumBlue, vec3_from_hex("#52b7fb")),
        (BottleColor::Blue, vec3_from_hex("#194af9")),
        (BottleColor::Purple, vec3_from_hex("#8c00d9")),
        (BottleColor::Pink, vec3_from_hex("#d212cc")),
        (BottleColor::Orange, vec3_from_hex("#f37c1c")),
        (BottleColor::Mystery, vec3_from_hex("#363636")),
    ];
    pub static ref EMPTY_COLOR: Vec3b = vec3_from_hex("#713d2c");
}

impl BottleColor {
    pub fn from_pixel_value(pixel: Vec3b) -> Option<Self> {
        if Self::is_empty_pixel(&pixel) {
            return None;
        }

        const COLOR_DISTANCE_THRESHOLD_SQ: u32 = 50 * 50;

        let mut best_match = None;
        for (color, target_pixel) in COLOR_VALUES.iter() {
            let dist = color_distance_sq(&pixel, target_pixel);
            if best_match.is_none_or(|(_, best_dist)| dist < best_dist) {
                best_match = Some((*color, dist));
            }
        }

        match best_match {
            Some((color, dist)) if dist <= COLOR_DISTANCE_THRESHOLD_SQ => Some(color),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn values() -> Vec<BottleColor> {
        COLOR_VALUES.iter().map(|(color, _)| *color).collect()
    }

    pub fn is_empty_pixel(pixel: &Vec3b) -> bool {
        is_color_within_tolerance(pixel, &EMPTY_COLOR, 30)
    }

    pub fn to_pixel_value(self) -> Vec3b {
        COLOR_VALUES
            .iter()
            .find(|(color, _)| *color == self)
            .map(|(_, pixel)| *pixel)
            .unwrap()
    }
}
