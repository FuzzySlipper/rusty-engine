//! Select independently usable mechanics and resolution capabilities.
//!
//! Run with:
//!
//! `cargo run -p gameplay-standard --example select_capabilities`

use gameplay_standard::modules::{mechanics, resolution};

fn selected_capability_ids() -> [&'static str; 2] {
    [
        mechanics::READOUT.identity().as_str(),
        resolution::READOUT.identity().as_str(),
    ]
}

fn main() {
    let selected = selected_capability_ids();
    assert_eq!(selected, ["mechanics", "resolution"]);

    let stat = mechanics::StatId::parse("health").expect("example stat identity is valid");
    let resolution = resolution::ResolutionId::new(1).expect("example resolution ID is positive");
    println!(
        "selected {selected:?}; {stat}; resolution={}",
        resolution.get()
    );
}
