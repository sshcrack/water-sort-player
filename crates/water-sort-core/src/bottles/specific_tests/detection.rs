use crate::bottles::test_utils::TestUtils;

macro_rules! detect_bottles_with_saved_frame {
    ($image:expr, $debug_prefix:expr) => {{ TestUtils::detect_bottles_from_image(&$image, $debug_prefix) }};
}

macro_rules! create_bottle_detection_test {
    ($test_name:ident, $image_filename:expr, $expected_bottles:expr) => {
        paste::paste! {
            #[test_log::test]
            fn $test_name() {
                let image = TestUtils::load_test_image($image_filename)
                    .expect(concat!("Failed to load ", $image_filename));

                let expected_bottles = TestUtils::parse_bottles_sequence($expected_bottles);
                let detected_bottles = detect_bottles_with_saved_frame!(
                    image,
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
        }
    };
}

create_bottle_detection_test!(
    test_mystery_detection,
    "detection/mystery-detection-1.png",
    "ERRR LLL? EYYY OO?? G??? EMMM ER?? EPP? GG?? EEBB EWWW"
);

create_bottle_detection_test!(
    test_empty_detection,
    "detection/empty-detection.png",
    "EOOO Y??? EEEE ggg? BB?? EB?? MMM? LLLL W??? RRR? EGGG EPPP"
);

create_bottle_detection_test!(
    test_failed_level_detection,
    "detection/failed-level.png",
    "RRRR LLL? YYYY EEL? G??? EMMM EOOO EPP? GG?? EEBB EWWW"
);

create_bottle_detection_test!(
    test_five_bottle_detection,
    "detection/five-bottle-detection.png",
    "BGBO GGOO BGBO EEEE EEEE"
);

create_bottle_detection_test!(
    test_six_bottle_detection,
    "detection/six-bottle-detection.png",
    "PGBP GGOO EEOO EEEE EEBB GPBP"
);

create_bottle_detection_test!(
    test_seven_bottle_detection,
    "detection/seven-bottle-detection.png",
    "OGGP OOBB OGBB RPGP RPRR EEEE EEEE"
);

create_bottle_detection_test!(
    test_twelve_alternative_detection,
    "detection/twelve-alternative-detection.png",
    "P??? Y??? O??? G??? B??? B??? O??? B??? M??? P??? EEEE EEEE"
);

create_bottle_detection_test!(
    test_eight_bottle_detection,
    "detection/eight-bottle-detection.png",
    "B??O Y??? B??? G??R RPOO BBYP EEEE EEEE"
);

create_bottle_detection_test!(
    test_nine_bottle_detection,
    "detection/nine-bottle-detection.png",
    "W??W G??? O??? G??? Y??P B??B EEEE EEEE R??O"
);

create_bottle_detection_test!(
    test_special_level_detection,
    "detection/special-level.png",
    "EGOB EPRG EBPO EGGR EBPB EEOP EEROR"
);

create_bottle_detection_test!(
    test_hidden_bottle_detection,
    "detection/hidden-detection.png",
    "OORR OOBB EEEE !O !B !B"
);

create_bottle_detection_test!(
    test_hidden_bottle_detection_2,
    "detection/hidden-detection-2.png",
    "!O !O !B !B BBOY OYBB EEEE EEEE"
);

create_bottle_detection_test!(
    test_hidden_bottle_detection_3,
    "detection/hidden-detection-3.png",
    "!O !O !B !R BOYB OBYB EEEE EEEE"
);

create_bottle_detection_test!(
    test_failed_level_with_hidden,
    "detection/failed-level-with-hidden.png",
    "!R !R EEBB O??G R?YR RY?R EEEP EEEY"
);

create_bottle_detection_test!(
    test_detection_with_nine_layout_alternative,
    "detection/nine-bottle-alternative-detection.png",
    "EEW? EOOO EEYY EPP? PWOY EEWP RRRY GGGG GGGG"
);
create_bottle_detection_test!(
    test_detection_with_nine_layout_alternative_2,
    "detection/nine-bottle-alternative-detection-2.png",
    "O?W? G?Y? G?B? G?P? !B !G !G EEEE EEEE"
);
create_bottle_detection_test!(
    test_detection_with_dark_blue_bottle,
    "detection/dark-blue-bottle.png",
    "EB?L EP?? EEWL EELD EB?? EP?P !D !L !D !L !D"
);
