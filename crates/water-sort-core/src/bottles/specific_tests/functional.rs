use crate::{bottles::Bottle, constants::BottleColor};

#[test]
fn test_bottle_is_full() {
    let fills = vec![
        BottleColor::Red,
        BottleColor::Green,
        BottleColor::Red,
        BottleColor::Blue,
    ];

    let mut bottle = Bottle::from_fills(fills);

    assert!(bottle.is_full());

    bottle.fills.pop();
    assert!(!bottle.is_full());
}

#[test]
fn test_bottle_solved() {
    BottleColor::values().iter().for_each(|&color| {
        if color == BottleColor::Mystery {
            return;
        }

        let fills = vec![color; 4];
        let bottle = Bottle::from_fills(fills);
        assert!(bottle.is_solved());
    });

    let empty_bottle = Bottle::from_fills(vec![]);
    assert!(!empty_bottle.is_solved());

    let unsolved_bottle = Bottle::from_fills(vec![
        BottleColor::Red,
        BottleColor::Green,
        BottleColor::Red,
        BottleColor::Blue,
    ]);

    assert!(!unsolved_bottle.is_solved());
}

#[test]
fn test_bottle_can_fill_from() {
    let source = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Red],
        mystery_origin_flags: vec![false, false],
    };

    let mut destination = Bottle::from_fills(vec![]);
    assert!(destination.can_fill_from(&source));

    destination.fills.push(BottleColor::Green);
    destination.get_mystery_origin_flags_mut().push(false);
    assert!(!destination.can_fill_from(&source));

    destination.fills.pop();
    destination.get_mystery_origin_flags_mut().pop();
    destination.fills.push(BottleColor::Red);
    destination.get_mystery_origin_flags_mut().push(false);
    assert!(destination.can_fill_from(&source));

    destination.fills.push(BottleColor::Red);
    destination.fills.push(BottleColor::Red);
    destination.get_mystery_origin_flags_mut().push(false);
    destination.get_mystery_origin_flags_mut().push(false);
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
        mystery_origin_flags: vec![false, false, false, false],
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
        mystery_origin_flags: vec![false, false, false, false],
    };

    let (amount, color) = bottle.get_top_fill().unwrap();
    assert_eq!(amount, 4);
    assert_eq!(color, BottleColor::Red);
}

#[test]
fn test_bottle_fill_from_simple() {
    let mut destination = Bottle::from_fills(vec![]);
    let mut source = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Red],
        mystery_origin_flags: vec![false, false],
    };

    destination.fill_from(&mut source);

    assert_eq!(destination.fills, vec![BottleColor::Red, BottleColor::Red]);
    assert!(source.is_empty());
}

#[test]
fn test_bottle_fill_from_partial() {
    let mut destination = Bottle::from_fills(vec![BottleColor::Red]);
    let mut source = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Red, BottleColor::Red],
        mystery_origin_flags: vec![false, false, false],
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
    let empty = Bottle::from_fills(vec![]);
    assert!(empty.is_empty());

    let not_empty = Bottle::from_fills(vec![BottleColor::Red]);
    assert!(!not_empty.is_empty());
}

#[test]
fn test_bottle_fill_count() {
    let bottle = Bottle {
        fills: vec![BottleColor::Red, BottleColor::Green, BottleColor::Blue],
        mystery_origin_flags: vec![false, false, false],
    };
    assert_eq!(bottle.get_fill_count(), 3);
}
