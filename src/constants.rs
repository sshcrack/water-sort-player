use lazy_static::lazy_static;
use opencv::core::Vec3b;

use crate::position::Pos;

pub const VIRTUAL_CAM: &str = "/dev/video10";

pub const START_BUTTON_POS: Pos = Pos(186, 605);

pub const FIRST_ROW_START_POS: Pos = Pos(41, 223);
pub const SECOND_ROW_OFFSET: Pos = Pos(0, 217);
pub const BOTTLE_SPACING: Pos = Pos(69, 0);
pub const COLOR_CHECK_OFFSET: Pos = Pos(0, 35);

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BottleColor {
    Yellow,
    Red,
    Green,
    LightBlue,
    Blue,
    Purple,
    Pink,
    Orange,
}

fn is_color_within_tolerance(pixel: Vec3b, target: Vec3b, tolerance: u8) -> bool {
    let b_diff = (pixel[0] as i16 - target[0] as i16).abs() as u8;
    let g_diff = (pixel[1] as i16 - target[1] as i16).abs() as u8;
    let r_diff = (pixel[2] as i16 - target[2] as i16).abs() as u8;

    b_diff <= tolerance && g_diff <= tolerance && r_diff <= tolerance
}

fn vec3_from_hex(hex: &str) -> Vec3b {
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
        (BottleColor::LightBlue, vec3_from_hex("#52b7fb")),
        (BottleColor::Blue, vec3_from_hex("#194af9")),
        (BottleColor::Purple, vec3_from_hex("#8c00d9")),
        (BottleColor::Pink, vec3_from_hex("#d212cc")),
        (BottleColor::Orange, vec3_from_hex("#f37c1c")),
    ];
    pub static ref EMPTY_COLOR: Vec3b = vec3_from_hex("#713d2c");
}

impl BottleColor {
    pub fn from_pixel_value(pixel: Vec3b) -> Option<Self> {
        for (color, target_pixel) in COLOR_VALUES.iter() {
            if is_color_within_tolerance(pixel, *target_pixel, 30) {
                return Some(*color);
            }
        }

        None
    }

    #[allow(dead_code)]
    pub fn values() -> Vec<BottleColor> {
        COLOR_VALUES.iter().map(|(color, _)| *color).collect()
    }

    pub fn is_empty_pixel(pixel: Vec3b) -> bool {
        is_color_within_tolerance(pixel, *EMPTY_COLOR, 30)
    }

    pub fn to_pixel_value(self) -> Vec3b {
        COLOR_VALUES
            .iter()
            .find(|(color, _)| *color == self)
            .map(|(_, pixel)| *pixel)
            .unwrap()
    }
}
