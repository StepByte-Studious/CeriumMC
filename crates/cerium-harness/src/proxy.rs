//! A transparent TCP proxy that records both directions of one connection.

use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

/// Everything that passed through the proxy.
pub struct Capture {
    pub to_server: Vec<u8>,
    pub to_client: Vec<u8>,
}

/// A proxy that relays and records exactly one connection.
pub struct Proxy {
    address: SocketAddr,
    handle: JoinHandle<io::Result<Capture>>,
}

impl Proxy {
    /// Listens on a free local port and relays the first connection to
    /// `upstream`.
    pub fn start(upstream: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = thread::spawn(move || relay(&listener, upstream));
        Ok(Self { address, handle })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Waits for the connection to close and returns what was recorded.
    /// A client must have connected first, or this waits forever.
    pub fn finish(self) -> io::Result<Capture> {
        match self.handle.join() {
            Ok(result) => result,
            Err(_) => Err(io::Error::other("the proxy thread panicked")),
        }
    }
}

fn relay(listener: &TcpListener, upstream: SocketAddr) -> io::Result<Capture> {
    let (client, _) = listener.accept()?;
    let server = TcpStream::connect_timeout(&upstream, TIMEOUT)?;
    client.set_read_timeout(Some(TIMEOUT))?;
    server.set_read_timeout(Some(TIMEOUT))?;
    let to_server = pump(client.try_clone()?, server.try_clone()?);
    let to_client = pump(server, client);
    Ok(Capture {
        to_server: join(to_server)?,
        to_client: join(to_client)?,
    })
}

/// Copies `from` to `to` on a new thread, keeping a copy of every byte.
fn pump(mut from: TcpStream, mut to: TcpStream) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut seen = Vec::new();
        let mut buffer = [0u8; 8192];
        while let Ok(count) = from.read(&mut buffer) {
            if count == 0 {
                break;
            }
            seen.extend_from_slice(&buffer[..count]);
            if to.write_all(&buffer[..count]).is_err() {
                break;
            }
        }
        let _ = to.shutdown(Shutdown::Write);
        seen
    })
}

fn join(handle: JoinHandle<Vec<u8>>) -> io::Result<Vec<u8>> {
    match handle.join() {
        Ok(bytes) => Ok(bytes),
        Err(_) => Err(io::Error::other("a proxy relay thread panicked")),
    }
}
