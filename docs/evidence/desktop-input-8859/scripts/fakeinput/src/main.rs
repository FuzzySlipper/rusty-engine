//! Drive KWin's org_kde_kwin_fake_input from the command line, for input
//! tests in a private headless KWin started with
//! KWIN_WAYLAND_NO_PERMISSION_CHECKS=1.
//!
//! fakeinput abs X Y | rel DX DY | button CODE 1|0 | key EVDEV 1|0 | sleep MS ...
use std::time::Duration;

use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input::OrgKdeKwinFakeInput;

struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<OrgKdeKwinFakeInput, ()> for State {
    fn event(_: &mut Self, _: &OrgKdeKwinFakeInput, _: <OrgKdeKwinFakeInput as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

fn main() {
    let connection = Connection::connect_to_env().expect("wayland connection");
    let (globals, mut queue) = registry_queue_init::<State>(&connection).expect("registry");
    let handle = queue.handle();
    let fake: OrgKdeKwinFakeInput = globals.bind(&handle, 4..=5, ()).expect("org_kde_kwin_fake_input");
    fake.authenticate("rusty-engine input test".into(), "desktop-shell input evidence (#8859)".into());
    let mut state = State;
    queue.roundtrip(&mut state).expect("roundtrip");
    // Commands come from the arguments, or line by line from stdin when there
    // are none, so one virtual device stays for a whole session (a device that
    // comes and goes changes the seat's capabilities).
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        for line in std::io::stdin().lines() {
            let line = line.expect("stdin");
            run(&fake, &mut queue, &mut state, line.split_whitespace().map(str::to_owned).collect());
            println!("ok");
        }
        return;
    }
    run(&fake, &mut queue, &mut state, std::mem::take(&mut args));
}

fn run(
    fake: &OrgKdeKwinFakeInput,
    queue: &mut wayland_client::EventQueue<State>,
    state: &mut State,
    args: Vec<String>,
) {
    let mut words = args.iter();
    let number = |value: Option<&String>| -> f64 { value.expect("argument").parse().expect("number") };
    while let Some(word) = words.next() {
        match word.as_str() {
            "abs" => fake.pointer_motion_absolute(number(words.next()), number(words.next())),
            "rel" => fake.pointer_motion(number(words.next()), number(words.next())),
            "button" => fake.button(number(words.next()) as u32, number(words.next()) as u32),
            "key" => fake.keyboard_key(number(words.next()) as u32, number(words.next()) as u32),
            "sleep" => {
                queue.roundtrip(state).expect("roundtrip");
                std::thread::sleep(Duration::from_millis(number(words.next()) as u64));
                continue;
            }
            other => panic!("unknown command {other}"),
        }
        queue.roundtrip(state).expect("roundtrip");
    }
}
