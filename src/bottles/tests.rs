use crate::{bottles::Bottle, constants::BottleColor};

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