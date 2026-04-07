use crate::{bottles::Bottle, constants::BottleColor};

#[test]
pub fn solver_level_1() {
    // Y = yellow, R = red, G = green, L = light blue, B = blue, P = purple, O = orange, W = pink, E = empty
    let bottles = "YRGL BPWO OBPG ROPL YRPW LWBG BGRY YLWO EEEE EEEE";
    let mut bottles_parsed: Vec<Bottle> = bottles
        .split_whitespace()
        .map(|bottle_str| {
            let fills = bottle_str
                .chars()
                .filter_map(|c| match c {
                    'Y' => Some(BottleColor::Yellow),
                    'R' => Some(BottleColor::Red),
                    'G' => Some(BottleColor::Green),
                    'L' => Some(BottleColor::LightBlue),
                    'B' => Some(BottleColor::Blue),
                    'P' => Some(BottleColor::Purple),
                    'O' => Some(BottleColor::Orange),
                    'W' => Some(BottleColor::Pink),
                    'E' => None,
                    _ => panic!("Invalid character in bottle string: {}", c),
                })
                .collect();

            Bottle::from_fills(fills)
        })
        .collect();

    let solution = crate::solver::run_solver(&bottles_parsed).expect("No solution found");

    for m in solution {
        println!("Move from bottle {} to bottle {}", m.0, m.1);
        m.perform_move_on_bottles(&mut bottles_parsed);
    }
}
