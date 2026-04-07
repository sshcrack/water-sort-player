use crate::{bottles::Bottle, constants::{BottleColor, EMPTY_COLOR}};

#[test]
fn test_bottle_is_full() {
    let fills = vec![
        BottleColor::Red,
        BottleColor::Green,
        BottleColor::Red,
        BottleColor::Blue,
    ];

    let mut bottle = Bottle { fills };

    assert!(bottle.is_full());

    bottle.fills.pop();
    assert!(!bottle.is_full());
}

#[test]
fn test_bottle_solved() {
    BottleColor::values().iter().for_each(|&color| {
        let fills = vec![color; 4];
        let bottle = Bottle { fills };
        assert!(bottle.is_solved());
    });

    let unsolved_bottle = Bottle {
        fills: vec![
            BottleColor::Red,
            BottleColor::Green,
            BottleColor::Red,
            BottleColor::Blue,
        ],
    };

    assert!(!unsolved_bottle.is_solved());
}

#[test]
fn test_bottle_can_fill_from() {
    let source = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Red],
    };

    let mut destination = Bottle { fills: vec![] };
    assert!(destination.can_fill_from(&source));

    destination.fills.push(BottleColor::Green);
    assert!(!destination.can_fill_from(&source));

    destination.fills.pop();
    destination.fills.push(BottleColor::Red);
    assert!(destination.can_fill_from(&source));

    destination.fills.push(BottleColor::Red);
    destination.fills.push(BottleColor::Red);
    assert!(!destination.can_fill_from(&source));
}

#[test]
fn test_bottle_get_top_fill() {
    let bottle = Bottle {
        fills: vec![
            BottleColor::Red,
            BottleColor::Red,
            BottleColor::Green,
            BottleColor::Green,
        ],
    };

    let (amount, color) = bottle.get_top_fill().unwrap();
    assert_eq!(amount, 2);
    assert_eq!(color, BottleColor::Green);
}

#[test]
fn test_bottle_get_top_fill_all_same() {
    let bottle = Bottle {
        fills: vec![
            BottleColor::Red,
            BottleColor::Red,
            BottleColor::Red,
            BottleColor::Red,
        ],
    };

    let (amount, color) = bottle.get_top_fill().unwrap();
    assert_eq!(amount, 4);
    assert_eq!(color, BottleColor::Red);
}

#[test]
fn test_bottle_fill_from_simple() {
    let mut destination = Bottle { fills: vec![] };
    let mut source = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Red],
    };

    destination.fill_from(&mut source);

    assert_eq!(destination.fills, vec![BottleColor::Red, BottleColor::Red]);
    assert!(source.is_empty());
}

#[test]
fn test_bottle_fill_from_partial() {
    let mut destination = Bottle {
        fills: vec![BottleColor::Red],
    };
    let mut source = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Red, BottleColor::Red],
    };

    destination.fill_from(&mut source);

    // Source has [Red, Red, Red] with 3 Reds on top
    // Destination has [Red] with 3 spots free
    // All 3 reds from source should fill (1 + 3 = 4, which is full)
    assert_eq!(destination.fills, vec![BottleColor::Red, BottleColor::Red, BottleColor::Red, BottleColor::Red]);
    assert!(source.is_empty());
}

#[test]
fn test_bottle_is_empty() {
    let empty = Bottle { fills: vec![] };
    assert!(empty.is_empty());

    let not_empty = Bottle {
        fills: vec![BottleColor::Red],
    };
    assert!(!not_empty.is_empty());
}

#[test]
fn test_bottle_fill_count() {
    let bottle = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Green, BottleColor::Blue],
    };
    assert_eq!(bottle.get_fill_count(), 3);
}

#[test]
fn test_validate_bottles_valid_config() {
    use crate::bottles::validate_detected_bottles;

    let bottles = vec![
        Bottle {
            fills: vec![BottleColor::Red, BottleColor::Red, BottleColor::Red, BottleColor::Red],
        },
        Bottle {
            fills: vec![BottleColor::Blue, BottleColor::Blue],
        },
        Bottle { fills: vec![] },
    ];

    assert!(validate_detected_bottles(&bottles));
}

#[test]
fn test_validate_bottles_invalid_overfull() {
    use crate::bottles::validate_detected_bottles;

    let bottles = vec![
        Bottle {
            fills: vec![
                BottleColor::Red,
                BottleColor::Red,
                BottleColor::Red,
                BottleColor::Red,
                BottleColor::Red, // 5 fills, should be max 4
            ],
        },
    ];

    assert!(!validate_detected_bottles(&bottles));
}

// New tests for color detection improvements
#[test]
fn test_color_within_tolerance() {
    use crate::constants::{is_color_within_tolerance, EMPTY_COLOR};
    use opencv::core::Vec3b;

    // Test exact match
    let pixel = *EMPTY_COLOR;
    assert!(is_color_within_tolerance(&pixel, &*EMPTY_COLOR, 0));

    // Test within tolerance
    let mut slightly_off = *EMPTY_COLOR;
    slightly_off[0] = slightly_off[0].saturating_add(5);
    assert!(is_color_within_tolerance(&slightly_off, &*EMPTY_COLOR, 10));

    // Test outside tolerance
    let way_off = Vec3b::from([255, 255, 255]);
    assert!(!is_color_within_tolerance(&way_off, &*EMPTY_COLOR, 10));
}

#[test]
fn test_bottle_color_detection() {
    use crate::constants::BottleColor;
    use opencv::core::Vec3b;

    let red_color = BottleColor::Red.to_pixel_value();
    assert_eq!(BottleColor::from_pixel_value(red_color), Some(BottleColor::Red));

    let green_color = BottleColor::Green.to_pixel_value();
    assert_eq!(
        BottleColor::from_pixel_value(green_color),
        Some(BottleColor::Green)
    );
}

#[test]
fn test_bottle_empty_detection() {
    use crate::constants::BottleColor;

    let empty_color = *EMPTY_COLOR;
    assert!(BottleColor::is_empty_pixel(&empty_color));

    let red_color = BottleColor::Red.to_pixel_value();
    assert!(!BottleColor::is_empty_pixel(&red_color));
}

#[test]
fn test_bottle_layout_standard10() {
    use crate::bottles::BottleLayout;

    let layout = BottleLayout::Standard10;
    assert_eq!(layout.rows(), 2);
    assert_eq!(layout.cols(), 5);
    assert_eq!(layout.total_bottles(), 10);
}

#[test]
fn test_bottle_layout_extended12() {
    use crate::bottles::BottleLayout;

    let layout = BottleLayout::Extended12;
    assert_eq!(layout.rows(), 3);
    assert_eq!(layout.cols(), 4);
    assert_eq!(layout.total_bottles(), 12);
}

#[test]
fn test_bottle_layout_default() {
    use crate::bottles::BottleLayout;

    let layout = BottleLayout::default();
    assert_eq!(layout, BottleLayout::Standard10);
}
