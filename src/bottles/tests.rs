use crate::{bottles::Bottle, constants::BottleColor};

#[cfg(test)]
use crate::bottles::{
    BottleLayout,
    test_utils::{ExpectedBottles, TestUtils},
};

fn parse_bottle_string(bottle_str: &str) -> Vec<BottleColor> {
    let mut fills: Vec<BottleColor> = bottle_str
        .chars()
        .filter_map(|c| match c {
            'Y' => Some(BottleColor::Yellow),
            'R' => Some(BottleColor::Red),
            'G' => Some(BottleColor::Green),
            'L' => Some(BottleColor::LightBlue),
            'M' => Some(BottleColor::MediumBlue),
            'B' => Some(BottleColor::Blue),
            'P' => Some(BottleColor::Purple),
            'O' => Some(BottleColor::Orange),
            'W' => Some(BottleColor::Pink),
            'E' => None,
            _ => panic!("Invalid character in bottle string: {}", c),
        })
        .collect();

    // Strings are provided top->bottom; bottle fills are stored bottom->top.
    fills.reverse();
    fills
}

fn parse_bottles_sequence(sequence: &str) -> Vec<Vec<BottleColor>> {
    sequence
        .split_whitespace()
        .map(parse_bottle_string)
        .collect()
}

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

    let empty_bottle = Bottle { fills: vec![] };
    assert!(!empty_bottle.is_solved());

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
    assert_eq!(
        destination.fills,
        vec![
            BottleColor::Red,
            BottleColor::Red,
            BottleColor::Red,
            BottleColor::Red
        ]
    );
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

// New comprehensive detection tests

#[test]
fn test_10_bottle_detection() {
    // Test bottle detection on the 10-bottle layout
    let image = match TestUtils::load_test_image("10-bottles.png") {
        Ok(img) => img,
        Err(_) => {
            println!("Warning: Could not load 10-bottles.png, skipping test");
            return;
        }
    };

    let layout = BottleLayout::ten_bottle_layout();
    let bottles =
        TestUtils::detect_bottles_from_image(&image, &layout).expect("Failed to detect bottles");

    let result = TestUtils::validate_detection(&bottles);
    result.print_summary();

    let expected = ExpectedBottles::for_10_bottle_layout();
    assert!(
        expected.validate(&result),
        "10-bottle detection failed validation"
    );

    // Additional checks
    assert_eq!(bottles.len(), 10, "Should detect exactly 10 bottles");

    // Print detailed bottle contents for manual verification
    println!("Detected bottle contents:");
    for (i, bottle) in bottles.iter().enumerate() {
        println!("  Bottle {}: {:?}", i, bottle.get_fills());
    }
}

#[test]
fn test_12_bottle_detection() {
    // Test bottle detection on the 12-bottle layout
    let image = match TestUtils::load_test_image("12-bottles.png") {
        Ok(img) => img,
        Err(_) => {
            println!("Warning: Could not load 12-bottles.png, skipping test");
            return;
        }
    };

    let layout = BottleLayout::twelve_bottle_layout();
    let bottles =
        TestUtils::detect_bottles_from_image(&image, &layout).expect("Failed to detect bottles");

    let result = TestUtils::validate_detection(&bottles);
    result.print_summary();

    let expected = ExpectedBottles::for_12_bottle_layout();
    assert!(
        expected.validate(&result),
        "12-bottle detection failed validation"
    );

    // Additional checks
    assert_eq!(bottles.len(), 11, "Should detect exactly 11 bottles");

    // Expected bottles (top->bottom notation, left->right by row)
    let expected_sequence = "POGR LMOR GYPO GYGB WBRL MLRY WMLP POMW BWBY";
    let expected_bottles = parse_bottles_sequence(expected_sequence);
    assert_eq!(expected_bottles.len(), 9);

    // Top row (6 bottles) + bottom row first 3 bottles
    for (i, expected_fills) in expected_bottles.iter().enumerate() {
        assert_eq!(
            bottles[i].get_fills(),
            expected_fills,
            "Bottle {} mismatch",
            i
        );
    }

    // Last two bottles should be empty
    assert!(bottles[9].is_empty(), "Bottle 9 should be empty");
    assert!(bottles[10].is_empty(), "Bottle 10 should be empty");
}

#[test]
fn test_detect_layout_10_bottles() {
    let image = match TestUtils::load_test_image("10-bottles.png") {
        Ok(img) => img,
        Err(_) => {
            println!("Warning: Could not load 10-bottles.png, skipping test");
            return;
        }
    };

    let layout = BottleLayout::detect_layout(&image).expect("Failed to detect layout");
    assert_eq!(layout.name, "10-bottles");
    assert_eq!(layout.bottle_count(), 10);
}

#[test]
fn test_detect_layout_12_bottles() {
    let image = match TestUtils::load_test_image("12-bottles.png") {
        Ok(img) => img,
        Err(_) => {
            println!("Warning: Could not load 12-bottles.png, skipping test");
            return;
        }
    };

    let layout = BottleLayout::detect_layout(&image).expect("Failed to detect layout");
    assert_eq!(layout, BottleLayout::twelve_bottle_layout());
}

#[test]
fn test_layout_comparison() {
    // Compare different layouts to ensure they produce different results
    let layout_10 = BottleLayout::ten_bottle_layout();
    let layout_12 = BottleLayout::twelve_bottle_layout();

    assert_ne!(layout_10.bottle_count(), layout_12.bottle_count());
    assert_ne!(layout_10.positions, layout_12.positions);

    println!("10-bottle layout: {} bottles", layout_10.bottle_count());
    println!("12-bottle layout: {} bottles", layout_12.bottle_count());
}
