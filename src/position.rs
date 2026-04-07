use crate::bottles::BottleLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos(pub i32, pub i32);

pub fn get_bottle_position(layout: &BottleLayout, index: usize) -> Pos {
    layout
        .get_click_position(index)
        .unwrap_or_else(|| panic!("Invalid bottle index: {}", index))
}
