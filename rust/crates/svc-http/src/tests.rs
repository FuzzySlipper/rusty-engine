use super::*;
use std::{
    io::{BufRead, BufReader},
    net::{TcpListener, TcpStream},
};

/// A one-route-per-path HTTP/1.1 server answering each connection once.
fn serve(route: fn(&str, &mut TcpStream)) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut request = String::new();
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap() <= 2 {
                        break;
                    }
                    request.push_str(&header.to_ascii_lowercase());
                }
                let route_path = if path == "/echo" {
                    format!("/echo\n{request}")
                } else {
                    path
                };
                route(&route_path, &mut stream);
            });
        }
    });
    format!("http://{address}")
}

fn respond(stream: &mut TcpStream, status: &str, headers: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n{headers}\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    let _ = stream.write_all(body);
}

fn routes(path: &str, stream: &mut TcpStream) {
    match path {
        "/index.json" => respond(
            stream,
            "200 OK",
            "etag: \"v1\"\r\nx-ratelimit-remaining: 59\r\n",
            b"{\"releases\":[]}",
        ),
        "/moved" => respond(stream, "302 Found", "location: /asset.bin\r\n", b""),
        "/asset.bin" => respond(stream, "200 OK", "", &[7u8; 300_000]),
        "/cached" => respond(stream, "304 Not Modified", "etag: \"v1\"\r\n", b""),
        "/missing" => respond(stream, "404 Not Found", "", b"no such asset"),
        "/truncated" => {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-length: 1000\r\nconnection: close\r\n\r\nonly a little",
            );
        }
        "/slow" => {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-length: 100000000\r\nconnection: close\r\n\r\n",
            );
            for _ in 0..600 {
                if stream.write_all(&[1u8; 1024]).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        echo if echo.starts_with("/echo\n") => {
            let body = echo["/echo\n".len()..].to_owned();
            respond(stream, "200 OK", "", body.as_bytes());
        }
        _ => respond(stream, "500 Internal Server Error", "", b""),
    }
}

fn wait(transfer: &Transfer) -> TransferSnapshot {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let snapshot = transfer.snapshot();
        if snapshot.state != TransferState::Running || Instant::now() > deadline {
            return snapshot;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn header<'a>(snapshot: &'a TransferSnapshot, name: &str) -> Option<&'a str> {
    snapshot
        .headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn a_get_completes_with_its_body_status_and_headers() {
    let server = serve(routes);
    let client = HttpClient::default();
    let done = wait(
        &client
            .start(
                &format!("{server}/index.json"),
                Headers::default(),
                Destination::Memory,
            )
            .unwrap(),
    );
    assert_eq!(done.state, TransferState::Completed);
    assert_eq!(done.status, 200);
    assert_eq!(&*done.body, b"{\"releases\":[]}");
    assert_eq!(header(&done, "etag"), Some("\"v1\""));
    assert_eq!(header(&done, "x-ratelimit-remaining"), Some("59"));

    let cached = wait(
        &client
            .start(
                &format!("{server}/cached"),
                Headers::default(),
                Destination::Memory,
            )
            .unwrap(),
    );
    assert_eq!(cached.state, TransferState::Completed, "a 304 completes");
    assert_eq!(cached.status, 304);
}

#[test]
fn product_headers_are_sent_and_empty_ones_omitted() {
    let server = serve(routes);
    let done = wait(
        &HttpClient::default()
            .start(
                &format!("{server}/echo"),
                Headers {
                    user_agent: "goldbox/1".to_owned(),
                    accept: "application/vnd.github+json".to_owned(),
                    bearer_token: "secret".to_owned(),
                    if_none_match: String::new(),
                },
                Destination::Memory,
            )
            .unwrap(),
    );
    let echoed = String::from_utf8(done.body.to_vec()).unwrap();
    assert!(echoed.contains("user-agent: goldbox/1"), "{echoed}");
    assert!(echoed.contains("accept: application/vnd.github+json"));
    assert!(echoed.contains("authorization: bearer secret"));
    assert!(!echoed.contains("if-none-match"));
}

#[test]
fn a_download_follows_redirects_and_appears_only_when_complete() {
    let server = serve(routes);
    let library = tempfile::tempdir().unwrap();
    let transfer = HttpClient::default()
        .start(
            &format!("{server}/moved"),
            Headers::default(),
            Destination::File {
                directory: library.path().to_owned(),
                file_name: "module.rpak".to_owned(),
            },
        )
        .unwrap();
    let done = wait(&transfer);
    assert_eq!(done.state, TransferState::Completed, "{}", done.diagnostic);
    assert_eq!(done.received_bytes, 300_000);
    assert_eq!(done.expected_bytes, 300_000);
    assert_eq!(
        library_files(library.path()).unwrap(),
        [("module.rpak".to_owned(), 300_000)]
    );
    assert_eq!(fs::read_dir(library.path()).unwrap().count(), 1);
}

#[test]
fn failed_downloads_leave_no_file() {
    let server = serve(routes);
    let library = tempfile::tempdir().unwrap();
    let client = HttpClient::default();
    let download = |path: &str| {
        wait(
            &client
                .start(
                    &format!("{server}{path}"),
                    Headers::default(),
                    Destination::File {
                        directory: library.path().to_owned(),
                        file_name: "module.rpak".to_owned(),
                    },
                )
                .unwrap(),
        )
    };
    let missing = download("/missing");
    assert_eq!(missing.state, TransferState::Failed(Failure::Status));
    assert_eq!(missing.status, 404);
    let truncated = download("/truncated");
    assert_eq!(
        truncated.state,
        TransferState::Failed(Failure::Interrupted),
        "{}",
        truncated.diagnostic
    );
    // Partial files are removed as their workers finish.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(fs::read_dir(library.path()).unwrap().count(), 0);
}

#[test]
fn a_cancel_is_final_and_removes_the_partial_file() {
    let server = serve(routes);
    let library = tempfile::tempdir().unwrap();
    let transfer = HttpClient::default()
        .start(
            &format!("{server}/slow"),
            Headers::default(),
            Destination::File {
                directory: library.path().to_owned(),
                file_name: "big.rpak".to_owned(),
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while transfer.snapshot().received_bytes == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        transfer.snapshot().received_bytes > 0,
        "progress is reported"
    );
    assert!(library_files(library.path()).unwrap().is_empty());
    transfer.cancel();
    assert_eq!(transfer.snapshot().state, TransferState::Cancelled);
    let deadline = Instant::now() + Duration::from_secs(10);
    while fs::read_dir(library.path()).unwrap().count() != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fs::read_dir(library.path()).unwrap().count(), 0);
    assert_eq!(transfer.snapshot().state, TransferState::Cancelled);
}

#[test]
fn connection_failures_are_reported() {
    // Bind and drop a listener so the port refuses connections.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let done = wait(
        &HttpClient::default()
            .start(
                &format!("http://127.0.0.1:{port}/index.json"),
                Headers::default(),
                Destination::Memory,
            )
            .unwrap(),
    );
    assert_eq!(done.state, TransferState::Failed(Failure::Connect));
    assert!(!done.diagnostic.is_empty());
}

#[test]
fn requests_name_a_url_and_a_plain_file_name() {
    let client = HttpClient::default();
    assert!(client
        .start("not a url", Headers::default(), Destination::Memory)
        .is_err());
    assert!(client
        .start(
            "ftp://example.com/x",
            Headers::default(),
            Destination::Memory
        )
        .is_err());
    for name in ["", "..", "a/b", "a\\b", ".x.partial"] {
        assert!(check_file_name(name).is_err(), "{name}");
    }
    assert!(check_file_name("module-1.2.rpak").is_ok());
}

#[test]
fn a_new_download_removes_leftover_partials_of_its_name() {
    let library = tempfile::tempdir().unwrap();
    fs::write(library.path().join(".module.rpak.99-1.partial"), b"stale").unwrap();
    fs::write(library.path().join(".other.rpak.99-1.partial"), b"other").unwrap();
    fs::write(library.path().join("module.rpak"), b"installed").unwrap();
    let partial = Partial::claim(library.path(), "module.rpak");
    assert!(!library.path().join(".module.rpak.99-1.partial").exists());
    assert!(library.path().join(".other.rpak.99-1.partial").exists());
    assert_eq!(
        library_files(library.path()).unwrap(),
        [("module.rpak".to_owned(), 9)]
    );
    partial.release(false);
    assert!(remove_library_file(library.path(), "module.rpak").unwrap());
    assert!(!remove_library_file(library.path(), "module.rpak").unwrap());
}
