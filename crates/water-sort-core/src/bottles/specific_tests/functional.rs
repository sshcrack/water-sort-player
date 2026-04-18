use crate::{bottles::Bottle, constants::BottleColor};

#[test_log::test]
fn test_bottle_is_full() {
    let fills = vec![
        BottleColor::red(),
        BottleColor::green(),
        BottleColor::red(),
        BottleColor::blue(),
    ];

    let mut bottle = Bottle::from_fills(fills, None);

    assert!(bottle.is_full());

    bottle.fills.pop();
    assert!(!bottle.is_full());
}

#[test_log::test]
fn test_bottle_solved() {
    BottleColor::values().iter().for_each(|&color| {
        if color == BottleColor::Mystery {
            return;
        }

        let fills = vec![color; 4];
        let bottle = Bottle::from_fills(fills, None);
        assert!(bottle.is_solved());
    });

    let empty_bottle = Bottle::empty();
    assert!(!empty_bottle.is_solved());

    let unsolved_bottle = Bottle::from_fills(
        vec![
            BottleColor::red(),
            BottleColor::green(),
            BottleColor::red(),
            BottleColor::blue(),
        ],
        None,
    );

    assert!(!unsolved_bottle.is_solved());
}

#[test_log::test]
fn test_bottle_can_fill_from() {
    let source = Bottle::from_fills(vec![BottleColor::red(), BottleColor::red()], None);

    let mut destination = Bottle::empty();
    assert!(destination.can_fill_from(&source));

    destination.fills.push((BottleColor::green(), false));
    assert!(!destination.can_fill_from(&source));

    destination.fills.pop();
    destination.fills.push((BottleColor::red(), false));
    assert!(destination.can_fill_from(&source));

    destination.fills.push((BottleColor::red(), false));
    destination.fills.push((BottleColor::red(), false));
    assert!(!destination.can_fill_from(&source));
}

#[test_log::test]
fn test_bottle_get_top_fill() {
    let bottle = Bottle::from_fills(
        vec![
            BottleColor::red(),
            BottleColor::red(),
            BottleColor::green(),
            BottleColor::green(),
        ],
        None,
    );

    let (amount, color) = bottle.get_top_fill().unwrap();
    assert_eq!(amount, 2);
    assert_eq!(color, BottleColor::green());
}

#[test_log::test]
fn test_bottle_get_top_fill_all_same() {
    let bottle = Bottle::from_fills(
        vec![
            BottleColor::red(),
            BottleColor::red(),
            BottleColor::red(),
            BottleColor::red(),
        ],
        None,
    );

    let (amount, color) = bottle.get_top_fill().unwrap();
    assert_eq!(amount, 4);
    assert_eq!(color, BottleColor::red());
}

#[test_log::test]
fn test_bottle_fill_from_simple() {
    let mut destination = Bottle::from_fills(vec![], None);
    let mut source = Bottle::from_fills(vec![BottleColor::red(), BottleColor::red()], None);

    destination.fill_from(&mut source);

    assert_eq!(
        destination.fills,
        vec![(BottleColor::red(), false), (BottleColor::red(), false)]
    );
    assert!(source.is_empty());
}

#[test_log::test]
fn test_bottle_fill_from_partial() {
    let mut destination = Bottle::from_fills(vec![BottleColor::red()], None);
    let mut source = Bottle::from_fills(
        vec![BottleColor::red(), BottleColor::red(), BottleColor::red()],
        None,
    );

    destination.fill_from(&mut source);

    // Source has [Red, Red, Red] with 3 Reds on top
    // Destination has [Red] with 3 spots free
    // All 3 reds from source should fill (1 + 3 = 4, which is full)
    assert_eq!(
        destination.fills,
        vec![
            (BottleColor::red(), false),
            (BottleColor::red(), false),
            (BottleColor::red(), false),
            (BottleColor::red(), false)
        ]
    );
    assert!(source.is_empty());
}

#[test_log::test]
fn test_bottle_is_empty() {
    let empty = Bottle::empty();
    assert!(empty.is_empty());

    let not_empty = Bottle::from_fills(vec![BottleColor::red()], None);
    assert!(!not_empty.is_empty());
}

#[test_log::test]
fn test_bottle_fill_count() {
    let bottle = Bottle::from_fills(
        vec![
            BottleColor::red(),
            BottleColor::green(),
            BottleColor::blue(),
        ],
        None,
    );
    assert_eq!(bottle.get_fill_count(), 3);
}
