use crate::bottles::{BottleLayout, test_utils::TestUtils};

macro_rules! detect_bottles_with_saved_frame {
    ($image:expr, $layout:expr, $debug_prefix:expr) => {{ TestUtils::detect_bottles_from_image(&$image, &$layout, $debug_prefix) }};
}

macro_rules! create_bottle_detection_test {
    ($test_name:ident, $image_filename:expr, $layout:expr, $expected_bottles:expr) => {
        paste::paste! {
            #[test_log::test]
            fn $test_name() {
                let image = TestUtils::load_test_image($image_filename)
                    .expect(concat!("Failed to load ", $image_filename));

                let expected_bottles = TestUtils::parse_bottles_sequence($expected_bottles);
                let layout = $layout;
                let detected_bottles = detect_bottles_with_saved_frame!(
                    image,
                    layout,
                    stringify!($test_name)
                )
                .expect("Failed to detect bottles with layout");

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


            #[test_log::test]
            fn [<$test_name _layout>]() {
                let image = TestUtils::load_test_image($image_filename)
                    .expect(concat!("Failed to load ", $image_filename));

                let detected_layout = BottleLayout::detect_layout(&image)
                    .expect("Failed to detect layout for test image");

                assert_eq!(
                    detected_layout,
                    $layout,
                    "Expected to detect layout '{}' but detected '{}'",
                    $layout.name,
                    detected_layout.name
                );
            }
        }
    };
}

#[test_log::test]
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

create_bottle_detection_test!(
    test_mystery_detection,
    "detection/mystery-detection-1.png",
    BottleLayout::eleven_bottle_layout(),
    "ERRR LLL? EYYY OO?? G??? EMMM ER?? EPP? GG?? EEBB EWWW"
);

create_bottle_detection_test!(
    test_empty_detection,
    "detection/empty-detection.png",
    BottleLayout::twelve_bottle_layout(),
    "EOOO Y??? EEEE ggg? BB?? EB?? MMM? LLLL W??? RRR? EGGG EPPP"
);

create_bottle_detection_test!(
    test_failed_level_detection,
    "detection/failed-level.png",
    BottleLayout::eleven_bottle_layout(),
    "RRRR LLL? YYYY EEL? G??? EMMM EOOO EPP? GG?? EEBB EWWW"
);

create_bottle_detection_test!(
    test_five_bottle_detection,
    "detection/five-bottle-detection.png",
    BottleLayout::five_bottle_layout(),
    "BGBO GGOO BGBO EEEE EEEE"
);

create_bottle_detection_test!(
    test_six_bottle_detection,
    "detection/six-bottle-detection.png",
    BottleLayout::six_bottle_layout(),
    "PGBP GGOO EEOO EEEE EEBB GPBP"
);

create_bottle_detection_test!(
    test_seven_bottle_detection,
    "detection/seven-bottle-detection.png",
    BottleLayout::seven_bottle_layout(),
    "OGGP OOBB OGBB RPGP RPRR EEEE EEEE"
);

create_bottle_detection_test!(
    test_twelve_alternative_detection,
    "detection/twelve-alternative-detection.png",
    BottleLayout::twelve_bottle_layout_alternative(),
    "P??? Y??? O??? G??? B??? B??? O??? B??? M??? P??? EEEE EEEE"
);

create_bottle_detection_test!(
    test_eight_bottle_detection,
    "detection/eight-bottle-detection.png",
    BottleLayout::eight_bottle_layout(),
    "B??O Y??? B??? G??R RPOO BBYP EEEE EEEE"
);

create_bottle_detection_test!(
    test_nine_bottle_detection,
    "detection/nine-bottle-detection.png",
    BottleLayout::nine_bottle_layout(),
    "W??W G??? O??? G??? Y??P B??B EEEE EEEE R??O"
);

create_bottle_detection_test!(
    test_special_level_detection,
    "detection/special-level.png",
    BottleLayout::seven_bottle_layout(),
    "EGOB EPRG EBPO EGGR EBPB EEOP EEROR"
);

create_bottle_detection_test!(
    test_hidden_bottle_detection,
    "detection/hidden-detection.png",
    BottleLayout::six_bottle_layout(),
    "OORR OOBB EEEE !O !B !B"
);

create_bottle_detection_test!(
    test_hidden_bottle_detection_2,
    "detection/hidden-detection-2.png",
    BottleLayout::eight_bottle_layout(),
    "!O !O !B !B BBOY OYBB EEEE EEEE"
);

create_bottle_detection_test!(
    test_hidden_bottle_detection_3,
    "detection/hidden-detection-3.png",
    BottleLayout::eight_bottle_layout(),
    "!O !O !B !R BOYB OBYB EEEE EEEE"
);

create_bottle_detection_test!(
    test_failed_level_with_hidden,
    "detection/failed-level-with-hidden.png",
    BottleLayout::eight_bottle_layout(),
    "!R !R EEBB O??G R?YR RY?R EEEP EEEY"
);

create_bottle_detection_test!(
    test_detection_with_nine_layout_alternative,
    "detection/nine-bottle-alternative-detection.png",
    BottleLayout::nine_bottle_layout_alternative(),
    "EEW? EOOO EEYY EPP? PWOY EEWP RRRY GGGG GGGG"
);
create_bottle_detection_test!(
    test_detection_with_nine_layout_alternative_2,
    "detection/nine-bottle-alternative-detection-2.png",
    BottleLayout::nine_bottle_layout_alternative(),
    "O?W? G?Y? G?B? G?P? !B !G !G EEEE EEEE"
);
create_bottle_detection_test!(
    test_detection_with_dark_blue_bottle,
    "detection/dark-blue-bottle.png",
    BottleLayout::eleven_bottle_layout(),
    "EB?L EP?? EEWL EELD EB?? EP?P !D !L !D !L !D"
);

