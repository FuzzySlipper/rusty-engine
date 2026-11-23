//! Sound through the world, measured in what playback mixes: a looping noise
//! emitter behind a wall, down a bending passage, and in a cave against open
//! ground. Each case runs the runtime's acoustics over a real voxel scene and
//! measures the mixed PCM's level (RMS) and brightness (spectral centroid).
//! `cargo test -p csharp-product-runtime sound_world -- --nocapture` prints
//! each measurement as one `sound-world {json}` line.

use std::sync::Arc;

use csharp_engine_services::acoustics::{query, ArrivalPath, Scene};
use render_audio::{
    AcousticKey, AudioRealizer, NoEntityPositions, OfflineBackend, OfflineBackendSettings,
};
use render_presentation::{
    AudioAcoustics, AudioBus, AudioClipRef, AudioEmitter, AudioHandle, AudioProjectionOp,
    AudioRolloff, AudioSourceDescriptor, PresentationOp, PresentationOpMeta,
};

use super::Acoustics;

const SAMPLE_RATE: u32 = 48_000;
const NOISE: &str = "sha256:noise";
const VOICE: u64 = 1;

/// Rock everywhere in `0..size` but where `open`, as 1 m voxels.
fn scene(size: [i64; 3], open: impl Fn(i64, i64, i64) -> bool) -> Arc<Scene> {
    let mut solids = Vec::new();
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                if !open(x, y, z) {
                    solids.push([x, y, z]);
                }
            }
        }
    }
    Arc::new(Scene::from_solid_voxels(1.0, 8, solids).unwrap())
}

/// Two seconds of looping white noise, mono 16-bit at 48 kHz.
fn noise_wav() -> Arc<[u8]> {
    let frames = SAMPLE_RATE as usize * 2;
    let mut bytes = Vec::with_capacity(44 + frames * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + frames as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(frames as u32 * 2).to_le_bytes());
    let mut state = 0x2545_f491_u32;
    for _ in 0..frames {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let sample = ((state as f32 / u32::MAX as f32) * 2.0 - 1.0) * 0.5;
        bytes.extend_from_slice(&((sample * 32767.0) as i16).to_le_bytes());
    }
    Arc::from(bytes)
}

struct Case {
    name: &'static str,
    scene: Arc<Scene>,
    emitter: [f32; 3],
    listener: [f32; 3],
    range: f32,
    reverb_level: f32,
}

struct Measured {
    path: Option<ArrivalPath>,
    /// What the query answers for the listener point.
    query_gain: f32,
    transmission: f32,
    low_pass_hz: Option<f32>,
    heard_at: Option<[f32; 3]>,
    reverb_send: f32,
    rms_db: f32,
    centroid_hz: f32,
    /// Level after the source stops: the space's reverb tail.
    tail_db: f32,
}

fn descriptor(case: &Case) -> AudioSourceDescriptor {
    AudioSourceDescriptor {
        clip: AudioClipRef {
            asset: "audio/noise".to_owned(),
            content_hash: NOISE.to_owned(),
            duration_seconds: Some(2.0),
        },
        bus: AudioBus::Sfx,
        volume: 1.0,
        pitch: 1.0,
        looping: true,
        // Unpanned, so the level measured does not turn with the way a sound
        // arrives; range and acoustics apply whatever the blend.
        spatial_blend: 0.0,
        max_distance: case.range,
        rolloff: AudioRolloff::LinearDecibels,
        pan: 0.0,
        emitter: AudioEmitter::World3d {
            position: case.emitter,
        },
        acoustics: AudioAcoustics::World,
    }
}

fn audio_op(sequence: u32, op: AudioProjectionOp) -> PresentationOp {
    PresentationOp::Audio {
        meta: PresentationOpMeta::new(sequence),
        op,
    }
}

fn measure(case: &Case) -> Measured {
    let clip = noise_wav();
    let clips = move |hash: &str| (hash == NOISE).then(|| Arc::clone(&clip));
    let mut realizer =
        AudioRealizer::<OfflineBackend>::with_backend_settings(OfflineBackendSettings {
            sample_rate: SAMPLE_RATE,
        })
        .unwrap();
    realizer.set_listener_pose(case.listener, [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]);
    realizer.apply(
        &[audio_op(
            1,
            AudioProjectionOp::Create {
                handle: AudioHandle::new(VOICE),
                descriptor: descriptor(case),
            },
        )],
        &clips,
        &NoEntityPositions,
    );
    let mut acoustics = Acoustics::new();
    let space = Some((Arc::clone(&case.scene), case.reverb_level));
    acoustics.follow(
        &mut realizer,
        space.clone(),
        Some(case.listener),
        &NoEntityPositions,
    );
    // The listener's open-air search runs on its worker; the next refresh
    // hears by it.
    while acoustics.listener.searching() {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    acoustics.refreshed = None;
    acoustics.follow(
        &mut realizer,
        space,
        Some(case.listener),
        &NoEntityPositions,
    );
    let key = AcousticKey::Voice(AudioHandle::new(VOICE));
    let applied = realizer.acoustics(key).expect("acoustics applied");
    let widen = |value: [f32; 3]| value.map(f64::from);
    let answered = query(
        &case.scene,
        widen(case.emitter),
        case.range,
        AudioRolloff::LinearDecibels,
        &[widen(case.listener)],
    );
    let (arrival, _) = answered.arrivals[0];
    let backend = realizer.backend_mut();
    // Let tweens and the reverb fill settle, then measure a second.
    backend.render(SAMPLE_RATE as usize / 2);
    let mixed = mono(&backend.render(SAMPLE_RATE as usize));
    realizer.apply(
        &[audio_op(
            2,
            AudioProjectionOp::Destroy {
                handle: AudioHandle::new(VOICE),
            },
        )],
        &clips,
        &NoEntityPositions,
    );
    let backend = realizer.backend_mut();
    // The dry sound stops within a block; what rings on is the reverb.
    backend.render(SAMPLE_RATE as usize / 50);
    let tail = mono(&backend.render(SAMPLE_RATE as usize / 2));
    let measured = Measured {
        path: Some(arrival.path),
        query_gain: arrival.gain,
        transmission: applied.transmission,
        low_pass_hz: applied.low_pass_hz,
        heard_at: applied.heard_at,
        reverb_send: realizer.space_reverb().map_or(0.0, |space| space.send),
        rms_db: decibels(rms(&mixed)),
        centroid_hz: centroid(&mixed),
        tail_db: decibels(rms(&tail)),
    };
    println!(
        "sound-world {{\"case\":\"{}\",\"path\":\"{:?}\",\"query_gain_db\":{:.2},\"transmission_db\":{:.2},\"low_pass_hz\":{},\"heard_at\":{:?},\"reverb_send\":{:.3},\"rms_db\":{:.2},\"centroid_hz\":{:.0},\"tail_db\":{:.2}}}",
        case.name,
        arrival.path,
        decibels(measured.query_gain),
        decibels(measured.transmission),
        measured
            .low_pass_hz
            .map_or("null".to_owned(), |cutoff| format!("{cutoff:.0}")),
        measured.heard_at,
        measured.reverb_send,
        measured.rms_db,
        measured.centroid_hz,
        measured.tail_db,
    );
    measured
}

fn mono(interleaved: &[f32]) -> Vec<f32> {
    let (frames, _) = interleaved.as_chunks::<2>();
    frames
        .iter()
        .map(|[left, right]| (left + right) * 0.5)
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

fn decibels(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1e-9).log10()
}

/// The magnitude-weighted mean frequency over Hann windows of 2048 samples.
fn centroid(samples: &[f32]) -> f32 {
    const WINDOW: usize = 2048;
    let mut weighted = 0.0_f64;
    let mut total = 0.0_f64;
    let (windows, _) = samples.as_chunks::<WINDOW>();
    for window in windows.iter().step_by(4).take(4) {
        let shaped: Vec<f64> = window
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let hann =
                    0.5 - 0.5 * (std::f64::consts::TAU * index as f64 / (WINDOW - 1) as f64).cos();
                f64::from(*sample) * hann
            })
            .collect();
        for bin in 1..WINDOW / 2 {
            let step = std::f64::consts::TAU * bin as f64 / WINDOW as f64;
            let (mut re, mut im) = (0.0, 0.0);
            for (index, sample) in shaped.iter().enumerate() {
                let angle = step * index as f64;
                re += sample * angle.cos();
                im -= sample * angle.sin();
            }
            let magnitude = (re * re + im * im).sqrt();
            weighted += magnitude * bin as f64 * f64::from(SAMPLE_RATE) / WINDOW as f64;
            total += magnitude;
        }
    }
    (weighted / total.max(1e-12)) as f32
}

/// A hall 24 × 8 × 24 m split by a one-metre wall at x = 12, with an
/// optional doorway two metres wide and three high starting at `door_z`.
fn walled_hall(door_z: Option<i64>) -> Arc<Scene> {
    scene([26, 10, 26], |x, y, z| {
        let inside = (1..25).contains(&x) && (1..9).contains(&y) && (1..25).contains(&z);
        let wall = x == 12 && !door_z.is_some_and(|door| (door..door + 2).contains(&z) && y < 4);
        inside && !wall
    })
}

/// The same hall with no wall.
fn open_hall() -> Arc<Scene> {
    scene([26, 10, 26], |x, y, z| {
        (1..25).contains(&x) && (1..9).contains(&y) && (1..25).contains(&z)
    })
}

#[test]
fn a_wall_muffles_a_sound_and_opening_it_restores_it() {
    let case = |name, scene| Case {
        name,
        scene,
        emitter: [17.0, 2.5, 13.0],
        listener: [7.0, 2.5, 13.0],
        range: 40.0,
        reverb_level: 0.0,
    };
    let open = measure(&case("wall:none", open_hall()));
    let closed = measure(&case("wall:closed", walled_hall(None)));
    let side = measure(&case("wall:door-aside", walled_hall(Some(16))));
    let through = measure(&case("wall:door-on-line", walled_hall(Some(12))));
    assert_eq!(closed.path, Some(ArrivalPath::Occluded));
    assert!(
        closed.rms_db < open.rms_db - 12.0,
        "quieter behind the wall"
    );
    assert!(
        closed.centroid_hz < open.centroid_hz * 0.5,
        "duller behind the wall"
    );
    assert!(closed.low_pass_hz.is_some());
    assert_eq!(side.path, Some(ArrivalPath::Around));
    assert!(
        side.rms_db > closed.rms_db + 3.0,
        "a doorway aside lets more through"
    );
    assert!(side.centroid_hz > closed.centroid_hz * 1.5, "and brighter");
    assert_eq!(through.path, Some(ArrivalPath::Open));
    assert!(
        (through.rms_db - open.rms_db).abs() < 1.0,
        "a doorway on the line is open"
    );
    // Playback plays the level the query answers. A low-passed sound also
    // loses the noise's energy above its cutoff, which `Gain` leaves out.
    for measured in [&open, &closed, &side, &through] {
        let modelled = decibels(measured.query_gain) - decibels(open.query_gain);
        let played = measured.rms_db - open.rms_db;
        if measured.low_pass_hz.is_none() {
            assert!((modelled - played).abs() < 1.5, "{modelled} vs {played}");
        } else {
            assert!(played < modelled, "{modelled} vs {played}");
        }
    }
}

#[test]
fn a_bending_passage_carries_a_sound_further_than_rock_does() {
    // Rock 24 × 6 × 24 m with a passage two metres square along +x, turning
    // along +z; or the same passage sealed by rock at its middle.
    let passage = |sealed: bool| {
        scene([24, 6, 24], move |x, y, z| {
            (1..3).contains(&y)
                && (((2..20).contains(&x) && (2..4).contains(&z))
                    || ((18..20).contains(&x) && (2..20).contains(&z)))
                && !(sealed && (10..12).contains(&x))
        })
    };
    let case = |name, scene| Case {
        name,
        scene,
        emitter: [19.0, 2.0, 19.0],
        listener: [3.0, 2.0, 3.0],
        range: 48.0,
        reverb_level: 0.0,
    };
    let carried = measure(&case("passage:open", passage(false)));
    let sealed = measure(&case("passage:sealed", passage(true)));
    assert_eq!(carried.path, Some(ArrivalPath::Around));
    assert!(carried.heard_at.is_some());
    assert_eq!(sealed.path, Some(ArrivalPath::Occluded));
    // Straight-line falloff through 16 m of rock leaves nothing audible.
    assert!(carried.rms_db > sealed.rms_db + 20.0);
    assert!(carried.rms_db > -60.0);
}

#[test]
fn a_cave_rings_and_open_ground_does_not() {
    let cave = scene([24, 12, 24], |x, y, z| {
        (2..22).contains(&x) && (1..10).contains(&y) && (2..22).contains(&z)
    });
    let ground = scene([24, 1, 24], |_, _, _| false);
    let case = |name, scene, base: f32| Case {
        name,
        scene,
        emitter: [8.0, base + 1.5, 12.0],
        listener: [14.0, base + 1.5, 12.0],
        range: 40.0,
        reverb_level: 1.0,
    };
    let in_cave = measure(&case("space:cave", cave, 1.0));
    let in_open = measure(&case("space:open", ground, 1.0));
    assert_eq!(in_cave.path, Some(ArrivalPath::Open));
    assert_eq!(in_open.path, Some(ArrivalPath::Open));
    assert!(in_cave.reverb_send > 0.9);
    assert_eq!(in_open.reverb_send, 0.0);
    assert!(
        in_cave.tail_db > in_open.tail_db + 30.0,
        "the cave rings on"
    );
    assert!(
        in_cave.rms_db > in_open.rms_db + 1.0,
        "its reverb adds level"
    );
}
