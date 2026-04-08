use opencv::core::MatTraitConst;

use crate::bottles::{
    BottleLayout, detect_and_draw_bottles, test_utils::TestUtils,
};

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

#[test]
fn test_mystery_detection() {
    let image = TestUtils::load_test_image("detection/mystery-detection-1.png")
        .expect("Failed to load mystery detection image");

    let expected_bottles = "ERRR LLL? EYYY OO?? G??? EMMM ER?? EPP? GG?? EEBB EWWW";
    let expected_bottles = TestUtils::parse_bottles_sequence(expected_bottles);

    let mut out_mat = image.try_clone().unwrap();
    let detected_bottles = detect_and_draw_bottles(&image, &mut out_mat)
        .expect("Failed to detect bottles with layout");

    // Write out mat to file
    opencv::imgcodecs::imwrite(
        "target/mystery_detection_result.png",
        &out_mat,
        &opencv::core::Vector::new(),
    ).unwrap();
    assert_eq!(
        detected_bottles.len(),
        expected_bottles.len(),
        "Detected bottle count does not match expected"
    );

    for (idx, (detected, expected)) in detected_bottles
        .iter()
        .zip(expected_bottles.iter())
        .enumerate()
    {
        assert_eq!(
            detected.get_fills(),
            expected.get_fills(),
            "Bottle {} does not match expected. Detected: {:?}, Expected: {:?}",
            idx,
            detected.get_fills(),
            expected.get_fills()
        );
    }
}
