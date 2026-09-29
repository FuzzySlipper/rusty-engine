//! Video playback over a composed room: frames on the Engine timeline,
//! letterboxed on black, and the facts that end a playback. The clip is
//! `render-video`'s synthetic fixture (ffmpeg `testsrc2`, 320x200, 10 fps,
//! 1.5 s, VP9 with an Opus tone).

mod common;

use common::*;
use render_presentation::{video_frame, VideoClipRef, VideoPlaybackHandle, VideoProjectionOp};
use render_wgpu::{RendererOptions, VideoFact, VideoFailure};

const CLIP: &[u8] = include_bytes!("../../render-video/tests/fixtures/testsrc.webm");
const CLIP_RESOURCE: &str = "content/video/testsrc.webm";
const HANDLE: VideoPlaybackHandle = VideoPlaybackHandle::new(7);

fn clip(asset: &str) -> VideoClipRef {
    VideoClipRef {
        asset: asset.to_owned(),
        content_hash: "sha256:fixture".to_owned(),
        media_type: "video/webm".to_owned(),
    }
}

fn room_harness() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    harness
        .resources
        .0
        .insert(CLIP_RESOURCE.to_owned(), CLIP.to_vec());
    harness.apply(room());
    harness.renderer.set_view_composition(
        &composition(
            vec![camera("main", [0.0, 0.6, 0.0], 0.0, -8.0)],
            vec![primary_view(
                "main",
                "main",
                viewport(0.0, 0.0, 1.0, 1.0),
                0,
            )],
        ),
        0.0,
    );
    harness
}

fn apply(harness: &mut Harness, op: VideoProjectionOp) {
    let issues =
        harness
            .renderer
            .apply_presentation(&video_frame([op]), &harness.resources, &|_| None);
    assert!(issues.is_empty(), "{issues:?}");
}

fn play(harness: &mut Harness, asset: &str) {
    apply(
        harness,
        VideoProjectionOp::Play {
            handle: HANDLE,
            clip: clip(asset),
        },
    );
}

fn render_at(harness: &mut Harness, seconds: f64) -> Vec<u8> {
    harness.renderer.set_animation_time(seconds);
    harness.composition(seconds).1
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * WIDTH + x) * 4) as usize;
    rgba[at..at + 4].try_into().unwrap()
}

#[test]
fn a_playing_clip_covers_the_world_letterboxed_on_the_engine_timeline() {
    let mut harness = room_harness();
    let world = render_at(&mut harness, 10.0);
    play(&mut harness, CLIP_RESOURCE);
    // The clip starts at the first render after its play op.
    let first = render_at(&mut harness, 10.0);
    assert_screenshot("video-first-frame", WIDTH, HEIGHT, &first);
    // 320x200 in a 320x180 target: 288x180, with 16-pixel bars at the sides.
    assert_eq!(pixel(&first, 4, 90), [0, 0, 0, 255], "left bar");
    assert_eq!(pixel(&first, WIDTH - 4, 90), [0, 0, 0, 255], "right bar");
    assert_ne!(
        pixel(&first, 4, 90),
        pixel(&world, 4, 90),
        "the world is covered"
    );
    let later = render_at(&mut harness, 10.55);
    assert_screenshot("video-frame-5", WIDTH, HEIGHT, &later);
    assert_ne!(first, later);
    // A held timeline holds the picture.
    assert_eq!(render_at(&mut harness, 10.55), later);
    assert!(harness.renderer.take_video_facts().is_empty());
    assert_eq!(harness.renderer.active_video(), Some(HANDLE));

    // The end of the clip completes the playback and uncovers the world.
    let after = render_at(&mut harness, 11.6);
    assert_eq!(
        harness.renderer.take_video_facts(),
        [VideoFact::Completed { handle: HANDLE }]
    );
    assert_eq!(harness.renderer.active_video(), None);
    assert_eq!(pixel(&after, 4, 90), pixel(&world, 4, 90));
}

#[test]
fn a_fresh_renderer_plays_an_active_clip_from_its_start() {
    // A new attachment's baseline carries the play op; the runtime applies it
    // before moving the renderer to the Engine's current time.
    let mut harness = room_harness();
    play(&mut harness, CLIP_RESOURCE);
    let restarted = render_at(&mut harness, 300.0);
    assert_screenshot("video-first-frame", WIDTH, HEIGHT, &restarted);
    assert!(harness.renderer.take_video_facts().is_empty());
}

#[test]
fn skips_stops_and_unreadable_clips_end_as_the_browser_host_ended_them() {
    let mut harness = room_harness();
    play(&mut harness, CLIP_RESOURCE);
    render_at(&mut harness, 1.0);
    apply(&mut harness, VideoProjectionOp::Skip { handle: HANDLE });
    assert_eq!(
        harness.renderer.take_video_facts(),
        [VideoFact::Skipped { handle: HANDLE }]
    );

    play(&mut harness, CLIP_RESOURCE);
    apply(&mut harness, VideoProjectionOp::Stop { handle: HANDLE });
    assert!(
        harness.renderer.take_video_facts().is_empty(),
        "stop is silent"
    );
    assert_eq!(harness.renderer.active_video(), None);

    play(&mut harness, "content/video/missing.webm");
    assert_eq!(
        harness.renderer.take_video_facts(),
        [VideoFact::Failed {
            handle: HANDLE,
            failure: VideoFailure::HostFailure
        }]
    );

    harness
        .resources
        .0
        .insert("content/video/broken.webm".to_owned(), b"not webm".to_vec());
    play(&mut harness, "content/video/broken.webm");
    assert_eq!(
        harness.renderer.take_video_facts(),
        [VideoFact::Failed {
            handle: HANDLE,
            failure: VideoFailure::DecodeFailed
        }]
    );
    // A skip with nothing playing is still reported, as the browser did.
    apply(&mut harness, VideoProjectionOp::Skip { handle: HANDLE });
    assert_eq!(
        harness.renderer.take_video_facts(),
        [VideoFact::Skipped { handle: HANDLE }]
    );
}
