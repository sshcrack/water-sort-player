use crate::position::Pos;

pub const VIRTUAL_CAM: &str = "/dev/video10";

pub const START_BUTTON_POS: Pos = Pos(186, 605);

pub const FIRST_ROW_START_POS: Pos = Pos(42, 224);
pub const SECOND_ROW_OFFSET: Pos = Pos(0, 217);
pub const BOTTLE_SPACING: Pos = Pos(62, 0);

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Color {
    Yellow,
    Red,
    Green,
    LightBlue,
    Blue,
    Purple,
    Pink,
    Orange,
}

#[allow(dead_code)]
impl Color {
    pub fn from_pixel_value(value: u32) -> Option<Self> {
        match value {
            0x00FFFF00 => Some(Color::Yellow),
            0x00FF0000 => Some(Color::Red),
            0x0000FF00 => Some(Color::Green),
            0x0000FFFF => Some(Color::LightBlue),
            0x000000FF => Some(Color::Blue),
            0x00FF00FF => Some(Color::Purple),
            0x00FFC0CB => Some(Color::Pink),
            0x00FFA500 => Some(Color::Orange),
            _ => None,
        }
    }
}