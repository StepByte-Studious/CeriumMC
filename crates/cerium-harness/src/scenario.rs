//! Parity scenarios. Each one runs a server in a fresh directory and
//! records named observations, which are compared between vanilla and
//! CeriumMC.

use std::fs;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

use crate::proxy::Proxy;
use crate::server::{ServerProcess, Target};
use crate::wire;

const EXIT_TIMEOUT: Duration = Duration::from_secs(120);
const BOOT_TIMEOUT: Duration = Duration::from_secs(180);
const STOP_TIMEOUT: Duration = Duration::from_secs(60);

/// Directories the vanilla bundler unpacks next to the server. They are a
/// detail of how the Java server is packaged, not server behaviour.
const IGNORED_FILES: &[&str] = &["libraries", "versions"];

/// Everything a scenario needs to run one target.
pub struct Context<'a> {
    pub target: &'a Target,
    pub dir: &'a Path,
    pub protocol: u32,
}

/// One named fact a scenario recorded.
pub struct Observation {
    pub name: String,
    pub text: String,
}

fn observe(name: &str, text: String) -> Observation {
    Observation {
        name: name.to_string(),
        text,
    }
}

type Run = fn(&Context<'_>) -> io::Result<Vec<Observation>>;

pub struct Scenario {
    pub name: &'static str,
    pub run: Run,
}

pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "first-run",
        run: first_run,
    },
    Scenario {
        name: "status-ping",
        run: status_ping,
    },
];

/// A brand-new server in an empty directory, without an accepted EULA.
/// Vanilla writes its default files and exits.
fn first_run(cx: &Context<'_>) -> io::Result<Vec<Observation>> {
    let mut server = ServerProcess::start(cx.target, cx.dir)?;
    let exit = server.wait_for_exit(EXIT_TIMEOUT);
    server.stop(STOP_TIMEOUT)?;
    if exit.is_none() && cx.target.is_vanilla() {
        return Err(io::Error::other("vanilla did not exit on its first run"));
    }

    let mut observations = vec![
        observe("exit", describe_exit(exit)),
        observe("files", list_files(cx.dir)?),
    ];
    for file in ["eula.txt", "server.properties"] {
        observations.push(observe(file, read_normalized(&cx.dir.join(file))));
    }
    Ok(observations)
}

/// A server-list ping through the recording proxy.
fn status_ping(cx: &Context<'_>) -> io::Result<Vec<Observation>> {
    let port = free_port()?;
    fs::write(cx.dir.join("eula.txt"), "eula=true\n")?;
    let properties = format!("server-port={port}\nonline-mode=false\nlevel-seed=1\n");
    fs::write(cx.dir.join("server.properties"), properties)?;

    let mut server = ServerProcess::start(cx.target, cx.dir)?;
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    if !server.wait_until_reachable(address, BOOT_TIMEOUT) {
        server.stop(STOP_TIMEOUT)?;
        if cx.target.is_vanilla() {
            return Err(io::Error::other("vanilla never accepted connections"));
        }
        let text = "the server never accepted connections".to_string();
        return Ok(vec![observe("status", text)]);
    }

    let proxy = Proxy::start(address)?;
    let reply = wire::status_ping(proxy.address(), cx.protocol);
    let capture = proxy.finish()?;
    server.stop(STOP_TIMEOUT)?;

    let mut observations = match reply {
        Ok(reply) => {
            let (id, body) = reply.pong;
            let pong = format!("packet 0x{id:02x}, body {}", hex(&body));
            vec![observe("status", reply.json), observe("pong", pong)]
        }
        Err(err) => vec![observe("status", format!("ping failed: {err}"))],
    };
    let mut frames = vec!["client -> server:".to_string()];
    frames.extend(wire::describe_frames(&capture.to_server));
    frames.push("server -> client:".to_string());
    frames.extend(wire::describe_frames(&capture.to_client));
    observations.push(observe("frames", frames.join("\n")));
    Ok(observations)
}

/// Finds a free local TCP port.
fn free_port() -> io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn describe_exit(exit: Option<ExitStatus>) -> String {
    match exit.map(|status| status.code()) {
        Some(Some(code)) => format!("exited with code {code}"),
        Some(None) => "exited without a code (killed by a signal)".to_string(),
        None => "still running after the timeout".to_string(),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Lists the top-level files and directories, sorted. Directories end
/// with `/`.
fn list_files(dir: &Path) -> io::Result<String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if IGNORED_FILES.contains(&name.as_str()) {
            continue;
        }
        let is_dir = entry.file_type()?.is_dir();
        let suffix = if is_dir { "/" } else { "" };
        names.push(format!("{name}{suffix}"));
    }
    names.sort();
    Ok(names.join("\n"))
}

/// Reads a text file with timestamps masked, or describes why it can't.
fn read_normalized(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(text) => normalize(&text),
        Err(err) if err.kind() == io::ErrorKind::NotFound => "<missing>".to_string(),
        Err(err) => format!("<unreadable: {err}>"),
    }
}

/// Masks the date comments Java writes into `.properties`-style files.
fn normalize(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(mask_timestamp).collect();
    lines.join("\n")
}

fn mask_timestamp(line: &str) -> &str {
    if is_timestamp_comment(line) {
        "#<timestamp>"
    } else {
        line
    }
}

/// Matches comments such as `#Tue Nov 03 15:36:31 UTC 2026`.
fn is_timestamp_comment(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('#') else {
        return false;
    };
    let words: Vec<&str> = rest.split_whitespace().collect();
    words.len() == 6
        && words[3].matches(':').count() == 2
        && words[5].len() == 4
        && words[5].bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{hex, is_timestamp_comment, normalize};

    #[test]
    fn timestamp_comments_are_recognised() {
        assert!(is_timestamp_comment("#Tue Nov 03 15:36:31 GMT+06:00 2026"));
        assert!(is_timestamp_comment("#Tue Nov 03 09:36:31 UTC 2026"));
        assert!(!is_timestamp_comment("#Minecraft server properties"));
        assert!(!is_timestamp_comment("eula=false"));
    }

    #[test]
    fn normalize_masks_only_timestamps() {
        let text = "#Minecraft server properties\r\n#Tue Nov 03 09:36:31 UTC 2026\r\nmotd=x\r\n";
        let expected = "#Minecraft server properties\n#<timestamp>\nmotd=x";
        assert_eq!(normalize(text), expected);
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x01, 0xab]), "01ab");
    }
}
