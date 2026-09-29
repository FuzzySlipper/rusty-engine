//! Size and compile baseline: loads the fixture and writes a blank PNG with no
//! renderer, so candidate measurements can subtract the shared cost.

use bootstrap_fixture::{op_census, write_png, Fixture, HEIGHT, WIDTH};

fn main() -> Result<(), String> {
    let fixture = Fixture::load(&Fixture::default_dir())?;
    let ops = fixture.baseline();
    println!("baseline ops: {:?}", op_census(&ops));
    let pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    write_png(
        std::path::Path::new("out/baseline.png"),
        WIDTH,
        HEIGHT,
        &pixels,
    )
}
