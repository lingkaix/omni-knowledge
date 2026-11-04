//! Loopback server for the WebView document and fixture previews.
//!
//! The listener binds `127.0.0.1` only. Paths are an allow-list compiled into
//! the binary. `contentUrl` values point here so image and PDF reads do not
//! put bytes in `body`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// Bind an ephemeral loopback port and serve the compiled assets.
///
/// Returns the port. The thread ends when the process ends.
pub fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind spike loopback");
    let port = listener.local_addr().expect("loopback port").port();
    thread::Builder::new()
        .name("spike-http".to_owned())
        .spawn(move || accept_loop(listener))
        .expect("spawn spike http");
    port
}

fn accept_loop(listener: TcpListener) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let _ = handle(stream);
    }
}

fn handle(mut stream: TcpStream) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let mut buffer = [0_u8; 2048];
    let count = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..count]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let path = path.split('?').next().unwrap_or("/");
    let found = ASSETS.iter().find(|(route, _, _)| *route == path);
    let (status, mime, body): (&str, &str, &[u8]) = match found {
        Some((_, bytes, mime)) => ("200 OK", mime, bytes),
        None => ("404 Not Found", "text/plain; charset=utf-8", b"not found"),
    };
    let header = format!(
        "HTTP/1.0 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}
