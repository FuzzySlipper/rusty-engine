//! `fixtures/testsrc.webm` is ffmpeg's `testsrc2` pattern, 320x200 at
//! 10 fps for 1.5 s in VP9 profile 0, with a 440 Hz mono Opus tone:
//!
//! ```text
//! ffmpeg -f lavfi -i testsrc2=size=320x200:rate=10 -f lavfi -i sine=frequency=440:sample_rate=48000 \
//!   -t 1.5 -c:v libvpx-vp9 -pix_fmt yuv420p -b:v 200k -c:a libopus -ac 1 -b:a 32k -g 10 testsrc.webm
//! ```

use render_video::{VideoClip, VideoPlayback};

const FIXTURE: &[u8] = include_bytes!("fixtures/testsrc.webm");

/// FNV-1a over ffmpeg's decode of the fixture
/// (`ffmpeg -i testsrc.webm -map 0:v -f rawvideo -pix_fmt yuv420p -fps_mode passthrough`).
const FFMPEG_DECODE_FNV1A: u64 = 0x4107_af50_4579_f6c7;

fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[test]
fn the_fixture_opens_with_its_video_and_soundtrack() {
    let clip = VideoClip::open(FIXTURE).unwrap();
    assert_eq!((clip.width, clip.height), (320, 200));
    assert_eq!(clip.frame_count(), 15);
    assert!((clip.duration - 1.5).abs() < 0.02, "{}", clip.duration);
    let audio = clip.audio.as_ref().expect("an Opus track");
    assert_eq!((audio.channels, audio.sample_rate), (1, 48_000));
    assert_eq!(audio.pre_skip, 312);
    assert!(!audio.packets.is_empty());
}

#[test]
fn playback_decodes_every_frame_exactly_as_ffmpeg_does() {
    let clip = VideoClip::open(FIXTURE).unwrap();
    let mut playback = VideoPlayback::new(clip);
    let mut hash = 0xcbf2_9ce4_8422_2325;
    let mut shown = 0;
    // Step at the frame rate so each position shows one new frame.
    for step in 0..15 {
        playback.advance(f64::from(step) * 0.1 + 0.01).unwrap();
        let (frame, changed) = playback.take_frame().expect("a frame");
        assert!(changed, "frame {step}");
        shown += 1;
        for (plane, (bytes, stride)) in frame.planes.iter().zip(frame.strides).enumerate() {
            let (width, height) = if plane == 0 {
                (frame.width as usize, frame.height as usize)
            } else {
                (frame.width as usize / 2, frame.height as usize / 2)
            };
            for row in 0..height {
                hash = fnv1a(hash, &bytes[row * stride..row * stride + width]);
            }
        }
    }
    assert_eq!(shown, 15);
    assert_eq!(hash, FFMPEG_DECODE_FNV1A);
    assert!(playback.finished(1.51));
    assert!(!playback.finished(1.4));
}

#[test]
fn a_late_position_skips_to_the_frame_shown_then() {
    let mut playback = VideoPlayback::new(VideoClip::open(FIXTURE).unwrap());
    playback.advance(0.95).unwrap();
    let (frame, changed) = playback.take_frame().unwrap();
    assert!(changed);
    assert!((frame.at - 0.9).abs() < 1e-6, "{}", frame.at);
    playback.advance(0.97).unwrap();
    assert!(!playback.take_frame().unwrap().1, "no new frame yet");
}

#[test]
fn a_clip_without_vp9_does_not_open() {
    assert!(VideoClip::open(b"not a webm file").is_err());
}

/// Every clip in `RUSTY_VIDEO_CLIPS` (a directory of `.webm` files, such as
/// Dagger's `content/worldrpg/media/cinematics`) opens and decodes to its end.
#[test]
#[ignore = "needs RUSTY_VIDEO_CLIPS"]
fn product_clips_decode_to_their_end() {
    let dir = std::env::var("RUSTY_VIDEO_CLIPS").expect("RUSTY_VIDEO_CLIPS");
    let mut clips: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "webm")
        })
        .collect();
    clips.sort();
    assert!(!clips.is_empty());
    for path in clips {
        let started = std::time::Instant::now();
        let clip = VideoClip::open(&std::fs::read(&path).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let (frames, duration, audio) = (clip.frame_count(), clip.duration, clip.audio.is_some());
        let mut playback = VideoPlayback::new(clip);
        playback
            .advance(duration)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(playback.take_frame().is_some());
        println!(
            "{}: {frames} frames, {duration:.2} s, opus {audio}, decoded in {:.0} ms",
            path.file_name().unwrap().to_string_lossy(),
            started.elapsed().as_secs_f64() * 1e3
        );
    }
}
