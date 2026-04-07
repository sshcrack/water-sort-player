use crate::constants::{BOTTLE_SPACING, FIRST_ROW_START_POS, SECOND_ROW_OFFSET};

#[derive(Debug, Clone, Copy)]
pub struct Pos(pub i32, pub i32);

pub fn get_bottle_position(index: usize) -> Pos {
    let row = index / 5;
    let col = index % 5;

    if row >= 2 || col >= 5 {
        panic!("Invalid bottle index: {}", index);
    }

    Pos(
        FIRST_ROW_START_POS.0 + col as i32 * BOTTLE_SPACING.0,
        FIRST_ROW_START_POS.1 + row as i32 * SECOND_ROW_OFFSET.1,
    )
}
