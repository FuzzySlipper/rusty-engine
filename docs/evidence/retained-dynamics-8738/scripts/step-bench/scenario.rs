//! The scene both benches run: a voxel floor, four crate towers, loose balls
//! and one rope, stepped at 60 Hz. Mid-run it teleports, removes and adds
//! bodies, then digs the floor out from under one tower.

pub const STEPS: usize = 600;
pub const STEP_SECONDS: f32 = 1.0 / 60.0;
pub const GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];
pub const CRATE_HALF: f32 = 0.4;
pub const BALL_RADIUS: f32 = 0.3;
pub const TOWERS: usize = 4;
pub const TOWER_HEIGHT: usize = 6;
pub const BALLS: usize = 16;
pub const ROPE_ANCHOR: [f32; 3] = [16.0, 8.0, 4.0];
pub const ROPE_LENGTH: f32 = 3.0;
pub const EDIT_STEP: usize = 300;
pub const DIG_STEP: usize = 400;

pub fn floor() -> Vec<[i64; 3]> {
    (0..32)
        .flat_map(|x| (0..32).map(move |z| [x, -1, z]))
        .collect()
}

/// The floor voxels under the first tower, cleared by one voxel edit.
pub fn dug() -> Vec<[i64; 3]> {
    (4..9)
        .flat_map(|x| (14..19).map(move |z| [x, -1, z]))
        .collect()
}

/// Crate centres, bottom to top, tower by tower.
pub fn crates() -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    for tower in 0..TOWERS {
        for level in 0..TOWER_HEIGHT {
            out.push([
                6.5 + 6.0 * tower as f32,
                CRATE_HALF + 0.01 + (2.0 * CRATE_HALF + 0.01) * level as f32,
                16.5,
            ]);
        }
    }
    out
}

pub fn balls() -> Vec<[f32; 3]> {
    (0..BALLS)
        .map(|index| {
            [
                4.5 + 1.5 * (index % 8) as f32,
                3.0,
                26.5 + 1.5 * (index / 8) as f32,
            ]
        })
        .collect()
}

pub fn rope_ball() -> [f32; 3] {
    [ROPE_ANCHOR[0] + ROPE_LENGTH, ROPE_ANCHOR[1], ROPE_ANCHOR[2]]
}

pub struct Report {
    pub label: &'static str,
    pub step_us: Vec<f64>,
    pub sleeping_before_edit: usize,
    pub sleeping_at_end: usize,
    pub contacts_at_end: usize,
    pub bodies_at_end: usize,
    pub last_tower_top_y: f32,
    pub dug_tower_top_y: f32,
    pub teleported_y: f32,
    pub rope_distance: f64,
    pub rope_catches: usize,
}

impl Report {
    pub fn print(&mut self) {
        let mut sorted = self.step_us.clone();
        sorted.sort_by(f64::total_cmp);
        let percentile = |p: f64| sorted[((sorted.len() - 1) as f64 * p).round() as usize];
        let total: f64 = self.step_us.iter().sum();
        let window = |range: std::ops::Range<usize>| {
            let slice = &self.step_us[range];
            slice.iter().sum::<f64>() / slice.len() as f64
        };
        println!(
            "{{\"label\":\"{}\",\"steps\":{},\"totalMs\":{:.2},\"stepUsP50\":{:.1},\"stepUsP95\":{:.1},\"stepUsMax\":{:.1},\"meanUsSettling\":{:.1},\"meanUsSettled\":{:.1},\"meanUsAfterDig\":{:.1},\"sleepingBeforeEdit\":{},\"sleepingAtEnd\":{},\"contactsAtEnd\":{},\"bodiesAtEnd\":{},\"lastTowerTopY\":{:.3},\"dugTowerTopY\":{:.3},\"teleportedY\":{:.3},\"ropeDistance\":{:.4},\"ropeCatches\":{}}}",
            self.label,
            self.step_us.len(),
            total / 1000.0,
            percentile(0.5),
            percentile(0.95),
            percentile(1.0),
            window(0..120),
            window(200..EDIT_STEP),
            window(DIG_STEP + 60..STEPS),
            self.sleeping_before_edit,
            self.sleeping_at_end,
            self.contacts_at_end,
            self.bodies_at_end,
            self.last_tower_top_y,
            self.dug_tower_top_y,
            self.teleported_y,
            self.rope_distance,
            self.rope_catches,
        );
    }
}
