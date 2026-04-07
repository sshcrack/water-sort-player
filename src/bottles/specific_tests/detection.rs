use crate::bottles::{
    BottleLayout,
    test_utils::{ExpectedBottles, TestUtils},
};

macro_rules! impl_test_bottle_detection {
    () => {
        
    };
}

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
fn test_11_bottle_detection() {
    // Test bottle detection on the 11-bottle layout
    let image = match TestUtils::load_test_image("11-bottles.png") {
        Ok(img) => img,
        Err(_) => {
            println!("Warning: Could not load 11-bottles.png, skipping test");
            return;
        }
    };

    let layout = BottleLayout::eleven_bottle_layout();
    let bottles =
        TestUtils::detect_bottles_from_image(&image, &layout).expect("Failed to detect bottles");

    let result = TestUtils::validate_detection(&bottles);
    result.print_summary();

    let expected = ExpectedBottles::for_11_bottle_layout();
    assert!(
        expected.validate(&result),
        "11-bottle detection failed validation"
    );

    // Expected bottles (top->bottom notation, left->right by row)
    let expected_sequence = "POGR LMOR GYPO GYGB WBRL MLRY WMLP POMW BWBY EEEE EEEE";
    let expected_bottles = TestUtils::parse_bottles_sequence(expected_sequence);
    assert_eq!(expected_bottles.len(), 11);

    // Top row (6 bottles) + bottom row first 3 bottles
    for (i, expected_fills) in expected_bottles.iter().enumerate() {
        assert_eq!(
            bottles[i].get_fills(),
            expected_fills,
            "Bottle {} mismatch",
            i
        );
    }
}

#[test]
fn test_detect_layout_11_bottles() {
    let image = match TestUtils::load_test_image("11-bottles.png") {
        Ok(img) => img,
        Err(_) => {
            println!("Warning: Could not load 11-bottles.png, skipping test");
            return;
        }
    };

    let layout = BottleLayout::detect_layout(&image).expect("Failed to detect layout");
    assert_eq!(layout, BottleLayout::eleven_bottle_layout());
}

#[test]
fn test_layout_comparison() {
    let layouts = BottleLayout::get_layouts();
    // Make sure that each layout is unique and has the expected number of bottles

    let mut seen_layouts = std::collections::HashSet::new();
    for layout in layouts {
        assert!(
            seen_layouts.insert(layout.clone()),
            "Duplicate layout detected: {}",
            layout.name
        );

        println!(
            "Layout '{}' has {} bottles",
            layout.name,
            layout.bottle_count()
        );
    }
}
