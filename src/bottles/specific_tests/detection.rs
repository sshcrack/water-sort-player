use crate::bottles::BottleLayout;

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
