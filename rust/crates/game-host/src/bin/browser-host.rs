use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_ids::EntityId;
use game_host::{DoorState, EncounterState, EnemyState, GameEvent, GameRuntime, MotionFact};
use serde::Serialize;

const PROJECT: &str = include_str!("../../../../../content/generated/encounter-gate.project.json");
const DEFAULT_ADDRESS: &str = "127.0.0.1:37881";
const ACTOR: EntityId = EntityId::new(1);
const ENCOUNTER: EntityId = EntityId::new(2);
const EXIT: EntityId = EntityId::new(3);
const FIRST_ENEMY: u64 = 4;
const SECOND_ENEMY: u64 = 5;
const MOTION_PROBE: EntityId = EntityId::new(10);
const PRODUCT_MOTION_PHASES: usize = 120;
const PRODUCT_MOTION_DELTA_SECONDS: f32 = 1.0 / 60.0;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserProjectionNode {
    id: u64,
    name: String,
    asset: String,
    translation: Option<[f32; 3]>,
    visible: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserEnemyState {
    id: u64,
    name: String,
    state: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserState {
    tick: u64,
    entity_revision: u64,
    projection: Vec<BrowserProjectionNode>,
    door_state: &'static str,
    encounter_state: &'static str,
    motion_state: &'static str,
    enemies: Vec<BrowserEnemyState>,
    last_events: Vec<String>,
}

fn main() {
    let (address, dist) = arguments();
    let dist = dist.canonicalize().unwrap_or_else(|error| {
        panic!(
            "browser shell dist {} is unavailable: {error}",
            dist.display()
        )
    });
    assert!(
        dist.join("index.html").is_file(),
        "browser shell is not built"
    );

    let runtime = Arc::new(Mutex::new(
        GameRuntime::from_project_content(PROJECT).expect("admit browser project"),
    ));
    let listener = TcpListener::bind(&address)
        .unwrap_or_else(|error| panic!("cannot bind browser host at {address}: {error}"));
    println!("browser-host listening at http://{address}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let runtime = Arc::clone(&runtime);
                let dist = dist.clone();
                std::thread::spawn(move || handle_connection(stream, &runtime, &dist));
            }
            Err(error) => eprintln!("browser-host accept error: {error}"),
        }
    }
}

fn arguments() -> (String, PathBuf) {
    let default_dist =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../ts/packages/browser-shell/dist");
    let mut address = DEFAULT_ADDRESS.to_owned();
    let mut dist = default_dist;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--addr" => address = args.next().expect("--addr needs a value"),
            "--dist" => dist = PathBuf::from(args.next().expect("--dist needs a value")),
            _ => panic!("unknown browser-host argument {argument}"),
        }
    }
    (address, dist)
}

fn handle_connection(mut stream: TcpStream, runtime: &Arc<Mutex<GameRuntime>>, dist: &Path) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let request = match read_request_head(&mut stream) {
        Ok(request) => request,
        Err(message) => {
            let _ = write_response(
                &mut stream,
                400,
                "text/plain; charset=utf-8",
                message.into(),
            );
            return;
        }
    };
    let mut parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    let method = parts.next().unwrap_or_default();
    let raw_path = parts.next().unwrap_or_default();
    let path = raw_path.split('?').next().unwrap_or(raw_path);
    let response = route(method, path, runtime, dist);
    let _ = write_response(&mut stream, response.0, response.1, response.2);
}

fn read_request_head(stream: &mut TcpStream) -> Result<String, String> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 2_048];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > 16_384 {
            return Err("request headers are too large".to_owned());
        }
    }
    String::from_utf8(request).map_err(|_| "request headers are not UTF-8".to_owned())
}

fn route(
    method: &str,
    path: &str,
    runtime: &Arc<Mutex<GameRuntime>>,
    dist: &Path,
) -> (u16, &'static str, Vec<u8>) {
    match (method, path) {
        ("GET", "/health") => (200, "text/plain; charset=utf-8", b"ok\n".to_vec()),
        ("GET", "/api/state") => {
            let runtime = runtime.lock().expect("runtime lock");
            json_response(200, browser_state(&runtime, Vec::new()))
        }
        ("POST", "/api/reset") => {
            let mut runtime = runtime.lock().expect("runtime lock");
            *runtime = GameRuntime::from_project_content(PROJECT).expect("reset browser project");
            json_response(200, browser_state(&runtime, Vec::new()))
        }
        ("POST", path) if path.starts_with("/api/defeat/") => {
            let raw = path.trim_start_matches("/api/defeat/");
            let Ok(enemy) = raw.parse::<u64>() else {
                return error_json(400, "enemy id must be an integer");
            };
            if enemy != FIRST_ENEMY && enemy != SECOND_ENEMY {
                return error_json(404, "enemy is not part of this encounter");
            }
            let mut runtime = runtime.lock().expect("runtime lock");
            match runtime.defeat_enemy(ACTOR, EntityId::new(enemy)) {
                Ok(receipt) => json_response(
                    200,
                    browser_state(
                        &runtime,
                        receipt
                            .events
                            .iter()
                            .map(event_name)
                            .map(str::to_owned)
                            .collect(),
                    ),
                ),
                Err(error) => error_json(409, &format!("{error}")),
            }
        }
        ("POST", "/api/motion-phase") => {
            let mut runtime = runtime.lock().expect("runtime lock");
            let mut moved = false;
            let mut blocked = false;
            for _ in 0..PRODUCT_MOTION_PHASES {
                match runtime.run_motion_phase(PRODUCT_MOTION_DELTA_SECONDS) {
                    Ok(receipt) => {
                        moved |= receipt
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, MotionFact::Moved { .. }));
                        blocked |= receipt
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, MotionFact::Blocked { .. }));
                    }
                    Err(error) => return error_json(409, &format!("{error}")),
                }
            }
            let mut facts = Vec::new();
            if moved {
                facts.push("KinematicMoved".to_owned());
            }
            if blocked {
                facts.push("KinematicBlocked".to_owned());
            }
            json_response(200, browser_state(&runtime, facts))
        }
        ("GET", _) | ("HEAD", _) => serve_static(method, path, dist),
        _ => error_json(405, "method not allowed"),
    }
}

fn browser_state(runtime: &GameRuntime, last_events: Vec<String>) -> BrowserState {
    let readout = runtime.readout();
    let projection = readout
        .projection
        .into_iter()
        .map(|node| BrowserProjectionNode {
            id: node.entity.raw(),
            name: node.name,
            asset: node.asset,
            translation: node.translation.map(|value| value.to_array()),
            visible: node.visible,
        })
        .collect();
    let enemies = [FIRST_ENEMY, SECOND_ENEMY]
        .into_iter()
        .map(|raw| {
            let view = runtime
                .session()
                .enemy(EntityId::new(raw))
                .expect("browser enemy");
            BrowserEnemyState {
                id: raw,
                name: view.entity_view.name,
                state: match view.state {
                    EnemyState::Alive => "alive",
                    EnemyState::Defeated => "defeated",
                },
            }
        })
        .collect();
    BrowserState {
        tick: readout.tick.raw(),
        entity_revision: readout.entity_revision,
        projection,
        door_state: match runtime.session().door(EXIT).expect("exit door").state {
            DoorState::Closed => "closed",
            DoorState::Open => "open",
        },
        encounter_state: match runtime
            .session()
            .encounter(ENCOUNTER)
            .expect("browser encounter")
            .state
        {
            EncounterState::Active => "active",
            EncounterState::Cleared => "cleared",
        },
        motion_state: if runtime
            .session()
            .entity(MOTION_PROBE)
            .expect("motion probe")
            .kinematic
            .expect("motion capability")
            .velocity
            .x
            == 0.0
        {
            "blocked"
        } else {
            "moving"
        },
        enemies,
        last_events,
    }
}

fn event_name(event: &GameEvent) -> &'static str {
    match event {
        GameEvent::SwitchActivated { .. } => "SwitchActivated",
        GameEvent::DoorOpened { .. } => "DoorOpened",
        GameEvent::DoorClosed { .. } => "DoorClosed",
        GameEvent::EnemyDefeated { .. } => "EnemyDefeated",
        GameEvent::EncounterCleared { .. } => "EncounterCleared",
    }
}

fn json_response(value_status: u16, value: impl Serialize) -> (u16, &'static str, Vec<u8>) {
    (
        value_status,
        "application/json; charset=utf-8",
        serde_json::to_vec(&value).expect("encode browser response"),
    )
}

fn error_json(status: u16, message: &str) -> (u16, &'static str, Vec<u8>) {
    json_response(status, serde_json::json!({ "error": message }))
}

fn serve_static(method: &str, path: &str, dist: &Path) -> (u16, &'static str, Vec<u8>) {
    let relative = if path == "/" {
        PathBuf::from("index.html")
    } else {
        PathBuf::from(path.trim_start_matches('/'))
    };
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return (403, "text/plain; charset=utf-8", b"forbidden\n".to_vec());
    }
    let file = dist.join(&relative);
    if !file.is_file() {
        return (404, "text/plain; charset=utf-8", b"not found\n".to_vec());
    }
    let content_type = content_type(&file);
    let body = if method == "HEAD" {
        Vec::new()
    } else {
        match fs::read(&file) {
            Ok(body) => body,
            Err(_) => return (500, "text/plain; charset=utf-8", b"read error\n".to_vec()),
        }
    };
    (200, content_type, body)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: Vec<u8>,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}
