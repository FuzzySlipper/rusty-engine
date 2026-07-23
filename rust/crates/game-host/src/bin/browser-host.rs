use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_ids::EntityId;
use game_host::{
    CombatFact, CombatMissReason, DoorState, EncounterState, EnemyState, GameEvent, GameRuntime,
    MotionFact, NavigationFact, NavigationState, PlayerControlFact, ResolvedAttackAction,
    ResolvedPlayerAction,
};
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
const PRODUCT_ACTION_TICKS: u64 = 1;

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
    position: [f32; 3],
    current_health: u32,
    max_health: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserPlayerBindings {
    move_forward: String,
    move_backward: String,
    move_left: String,
    move_right: String,
    mouse_look: String,
    primary_fire: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserPlayerState {
    id: u64,
    position: [f32; 3],
    yaw_degrees: f32,
    pitch_degrees: f32,
    move_step_seconds: f32,
    look_degrees_per_unit: f32,
    bindings: BrowserPlayerBindings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserWeaponState {
    damage: u32,
    ammo_remaining: u32,
    ammo_capacity: u32,
    ready_at_tick: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVoxelMeshGroup {
    material_slot: u16,
    start: u32,
    count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVoxelMeshChunk {
    chunk: [i64; 3],
    content_hash: String,
    translation: [f32; 3],
    positions: Vec<f32>,
    normals: Vec<f32>,
    indices: Vec<u32>,
    groups: Vec<BrowserVoxelMeshGroup>,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserGeneratedEnvironment {
    seed: u64,
    output_hash: String,
    solid_voxels: usize,
    mesh_vertices: u32,
    mesh_quads: u32,
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
    navigation_state: &'static str,
    player_motion_state: &'static str,
    combat_state: &'static str,
    player: BrowserPlayerState,
    weapon: BrowserWeaponState,
    voxel_meshes: Vec<BrowserVoxelMeshChunk>,
    generated_environment: Option<BrowserGeneratedEnvironment>,
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
    let request = match read_request(&mut stream) {
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
    let path = request.path.split('?').next().unwrap_or(&request.path);
    let response = route(&request.method, path, &request.body, runtime, dist);
    let _ = write_response(&mut stream, response.0, response.1, response.2);
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 2_048];
    let header_end = loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("request ended before its headers".to_owned());
        }
        request.extend_from_slice(&buffer[..read]);
        if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
        if request.len() > 16_384 {
            return Err("request headers are too large".to_owned());
        }
    };
    let head = String::from_utf8(request[..header_end].to_vec())
        .map_err(|_| "request headers are not UTF-8".to_owned())?;
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>())
        })
        .transpose()
        .map_err(|_| "content-length must be an integer".to_owned())?
        .unwrap_or(0);
    if content_length > 16_384 {
        return Err("request body is too large".to_owned());
    }
    while request.len() < header_end + content_length {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("request ended before its declared body".to_owned());
        }
        request.extend_from_slice(&buffer[..read]);
    }
    let mut parts = head.lines().next().unwrap_or_default().split_whitespace();
    let method = parts.next().ok_or("request method is missing")?.to_owned();
    let path = parts.next().ok_or("request path is missing")?.to_owned();
    Ok(HttpRequest {
        method,
        path,
        body: request[header_end..header_end + content_length].to_vec(),
    })
}

fn route(
    method: &str,
    path: &str,
    body: &[u8],
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
        ("POST", "/api/attack") => {
            let action: ResolvedAttackAction = match serde_json::from_slice(body) {
                Ok(action) => action,
                Err(error) => return error_json(400, &format!("invalid attack action: {error}")),
            };
            let mut runtime = runtime.lock().expect("runtime lock");
            let mut facts = match advance_product_action(&mut runtime) {
                Ok(events) => events,
                Err(error) => return error_json(409, &format!("{error}")),
            };
            match runtime.attack(ACTOR, action) {
                Ok(receipt) => {
                    facts.extend(
                        receipt
                            .facts
                            .iter()
                            .map(combat_fact_name)
                            .map(str::to_owned),
                    );
                    facts.extend(receipt.events.iter().map(event_name).map(str::to_owned));
                    json_response(200, browser_state(&runtime, facts))
                }
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
        ("POST", "/api/navigation-step") => {
            let mut runtime = runtime.lock().expect("runtime lock");
            match runtime.run_navigation_phase(PRODUCT_MOTION_DELTA_SECONDS) {
                Ok(receipt) => {
                    let facts = receipt
                        .facts
                        .iter()
                        .map(navigation_fact_name)
                        .map(str::to_owned)
                        .collect();
                    json_response(200, browser_state(&runtime, facts))
                }
                Err(error) => error_json(409, &format!("{error}")),
            }
        }
        ("POST", "/api/navigation-phase") => {
            let mut runtime = runtime.lock().expect("runtime lock");
            let mut advanced = false;
            let mut arrived = false;
            let mut blocked = false;
            let mut unreachable = false;
            for _ in 0..240 {
                match runtime.run_navigation_phase(PRODUCT_MOTION_DELTA_SECONDS) {
                    Ok(receipt) => {
                        for fact in receipt.facts {
                            match fact {
                                NavigationFact::Advanced { .. } => advanced = true,
                                NavigationFact::Arrived { .. } => arrived = true,
                                NavigationFact::Blocked { .. } => blocked = true,
                                NavigationFact::Unreachable { .. } => unreachable = true,
                            }
                        }
                        if runtime
                            .session()
                            .navigation(EntityId::new(FIRST_ENEMY))
                            .is_some_and(|view| view.state != NavigationState::Following)
                        {
                            break;
                        }
                    }
                    Err(error) => return error_json(409, &format!("{error}")),
                }
            }
            let mut facts = Vec::new();
            if advanced {
                facts.push("NavigationAdvanced".to_owned());
            }
            if arrived {
                facts.push("NavigationArrived".to_owned());
            }
            if blocked {
                facts.push("NavigationBlocked".to_owned());
            }
            if unreachable {
                facts.push("NavigationUnreachable".to_owned());
            }
            json_response(200, browser_state(&runtime, facts))
        }
        ("POST", "/api/player-action") => {
            let action: ResolvedPlayerAction = match serde_json::from_slice(body) {
                Ok(action) => action,
                Err(error) => return error_json(400, &format!("invalid resolved action: {error}")),
            };
            let mut runtime = runtime.lock().expect("runtime lock");
            let mut facts = match advance_product_action(&mut runtime) {
                Ok(events) => events,
                Err(error) => return error_json(409, &format!("{error}")),
            };
            match runtime.apply_player_action(ACTOR, action) {
                Ok(receipt) => {
                    facts.extend(
                        receipt
                            .facts
                            .iter()
                            .map(player_fact_name)
                            .map(str::to_owned),
                    );
                    json_response(200, browser_state(&runtime, facts))
                }
                Err(error) => error_json(409, &format!("{error}")),
            }
        }
        ("GET", _) | ("HEAD", _) => serve_static(method, path, dist),
        _ => error_json(405, "method not allowed"),
    }
}

fn advance_product_action(
    runtime: &mut GameRuntime,
) -> Result<Vec<String>, game_host::RuntimeError> {
    let receipt = runtime.advance_by(PRODUCT_ACTION_TICKS)?;
    Ok(receipt
        .events
        .iter()
        .map(event_name)
        .map(str::to_owned)
        .collect())
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
                position: view
                    .entity_view
                    .transform
                    .expect("browser enemy transform")
                    .translation
                    .to_array(),
                current_health: runtime
                    .session()
                    .health(EntityId::new(raw))
                    .expect("browser enemy health")
                    .current,
                max_health: runtime
                    .session()
                    .health(EntityId::new(raw))
                    .expect("browser enemy health")
                    .config
                    .max,
            }
        })
        .collect();
    let player = runtime
        .session()
        .player_controller(ACTOR)
        .expect("browser player controller");
    let bindings = &player.config.bindings;
    let player_state = BrowserPlayerState {
        id: ACTOR.raw(),
        position: player
            .entity_view
            .transform
            .expect("browser player transform")
            .translation
            .to_array(),
        yaw_degrees: player.state.yaw_degrees,
        pitch_degrees: player.state.pitch_degrees,
        move_step_seconds: player.config.move_step_seconds,
        look_degrees_per_unit: player.config.look_degrees_per_unit,
        bindings: BrowserPlayerBindings {
            move_forward: bindings.move_forward.clone(),
            move_backward: bindings.move_backward.clone(),
            move_left: bindings.move_left.clone(),
            move_right: bindings.move_right.clone(),
            mouse_look: bindings.mouse_look.clone(),
            primary_fire: bindings.primary_fire.clone(),
        },
    };
    let weapon = runtime
        .session()
        .weapon(ACTOR)
        .expect("browser player weapon");
    let weapon_state = BrowserWeaponState {
        damage: weapon.config.damage,
        ammo_remaining: weapon.state.ammo_remaining,
        ammo_capacity: weapon.config.ammo_capacity,
        ready_at_tick: weapon.state.ready_at_tick.raw(),
    };
    let player_motion_state = if last_events.iter().any(|event| event == "PlayerBlocked") {
        "blocked"
    } else if last_events.iter().any(|event| event == "PlayerMoved") {
        "moved"
    } else {
        "idle"
    };
    let combat_state = if last_events.iter().any(|event| event == "CombatHit") {
        "hit"
    } else if last_events
        .iter()
        .any(|event| event.starts_with("CombatMissed"))
    {
        "missed"
    } else {
        "ready"
    };
    let scene = runtime
        .collision_scene()
        .expect("browser project collision scene");
    let voxel_meshes = scene
        .mesh_chunks()
        .iter()
        .map(|mesh| BrowserVoxelMeshChunk {
            chunk: mesh.chunk,
            content_hash: format!("{:016x}", mesh.content_hash),
            translation: mesh.translation,
            positions: mesh.positions.clone(),
            normals: mesh.normals.clone(),
            indices: mesh.indices.clone(),
            groups: mesh
                .groups
                .iter()
                .map(|group| BrowserVoxelMeshGroup {
                    material_slot: group.material_slot,
                    start: group.start,
                    count: group.count,
                })
                .collect(),
            bounds_min: mesh.bounds_min,
            bounds_max: mesh.bounds_max,
        })
        .collect();
    let generated_environment = scene.generated_room().map(|(config, record)| {
        let mesh_vertices = scene.mesh_chunks().iter().map(|mesh| mesh.vertices).sum();
        let mesh_quads = scene.mesh_chunks().iter().map(|mesh| mesh.quads).sum();
        BrowserGeneratedEnvironment {
            seed: config.seed,
            output_hash: format!("{:016x}", record.output_hash),
            solid_voxels: record.solid_voxel_count,
            mesh_vertices,
            mesh_quads,
        }
    });
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
        navigation_state: match runtime
            .session()
            .navigation(EntityId::new(FIRST_ENEMY))
            .expect("browser navigator")
            .state
        {
            NavigationState::Following => "following",
            NavigationState::Arrived => "arrived",
            NavigationState::Blocked => "blocked",
            NavigationState::Unreachable => "unreachable",
        },
        player_motion_state,
        combat_state,
        player: player_state,
        weapon: weapon_state,
        voxel_meshes,
        generated_environment,
        enemies,
        last_events,
    }
}

fn combat_fact_name(fact: &CombatFact) -> &'static str {
    match fact {
        CombatFact::AttackFired { .. } => "CombatFired",
        CombatFact::AttackHit { .. } => "CombatHit",
        CombatFact::AttackMissed {
            reason: CombatMissReason::NoTarget,
            ..
        } => "CombatMissedNoTarget",
        CombatFact::AttackMissed {
            reason: CombatMissReason::WorldBlocked,
            ..
        } => "CombatMissedWorldBlocked",
        CombatFact::DamageApplied { .. } => "DamageApplied",
        CombatFact::EnemyDefeated { .. } => "CombatEnemyDefeated",
    }
}

fn navigation_fact_name(fact: &NavigationFact) -> &'static str {
    match fact {
        NavigationFact::Advanced { .. } => "NavigationAdvanced",
        NavigationFact::Arrived { .. } => "NavigationArrived",
        NavigationFact::Blocked { .. } => "NavigationBlocked",
        NavigationFact::Unreachable { .. } => "NavigationUnreachable",
    }
}

fn player_fact_name(fact: &PlayerControlFact) -> &'static str {
    match fact {
        PlayerControlFact::Moved { .. } => "PlayerMoved",
        PlayerControlFact::Blocked { .. } => "PlayerBlocked",
        PlayerControlFact::LookChanged { .. } => "PlayerLookChanged",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_browser_actions_advance_cooldown_and_become_eligible_again() {
        let runtime = Arc::new(Mutex::new(
            GameRuntime::from_project_content(PROJECT).expect("admit browser project"),
        ));
        let attack = serde_json::to_vec(&ResolvedAttackAction::Attack).unwrap();
        let look = serde_json::to_vec(&ResolvedPlayerAction::Look {
            yaw_delta: 0.25,
            pitch_delta: 0.0,
        })
        .unwrap();

        assert_eq!(
            route("POST", "/api/attack", &attack, &runtime, Path::new(".")).0,
            200
        );
        assert_eq!(
            route("POST", "/api/attack", &attack, &runtime, Path::new(".")).0,
            409
        );
        assert_eq!(
            route(
                "POST",
                "/api/player-action",
                &look,
                &runtime,
                Path::new(".")
            )
            .0,
            200
        );
        assert_eq!(
            route("POST", "/api/attack", &attack, &runtime, Path::new(".")).0,
            200
        );

        let runtime = runtime.lock().expect("runtime lock");
        assert_eq!(runtime.tick().raw(), 4);
        assert_eq!(
            runtime
                .session()
                .weapon(ACTOR)
                .unwrap()
                .state
                .ammo_remaining,
            6
        );
    }
}
