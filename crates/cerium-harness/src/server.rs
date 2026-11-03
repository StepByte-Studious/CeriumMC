//! Launches a server under test and watches its console.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(250);

/// A server implementation the harness can run.
pub enum Target {
    /// The pinned vanilla jar, run with `java -jar`.
    Vanilla { jar: PathBuf },
    /// The CeriumMC executable.
    Cerium { exe: PathBuf },
}

impl Target {
    pub fn name(&self) -> &'static str {
        match self {
            Target::Vanilla { .. } => "vanilla",
            Target::Cerium { .. } => "cerium",
        }
    }

    pub fn is_vanilla(&self) -> bool {
        matches!(self, Target::Vanilla { .. })
    }

    fn command(&self) -> Command {
        match self {
            Target::Vanilla { jar } => {
                let mut command = Command::new("java");
                command.arg("-jar").arg(jar).arg("--nogui");
                command
            }
            Target::Cerium { exe } => {
                let mut command = Command::new(exe);
                command.arg("--nogui");
                command
            }
        }
    }
}

/// A running server. It is killed when dropped.
pub struct ServerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    console: Receiver<String>,
    log: Vec<String>,
}

impl ServerProcess {
    /// Starts `target` with `dir` as its working directory.
    pub fn start(target: &Target, dir: &Path) -> io::Result<Self> {
        let mut command = target.command();
        command
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        let (sender, console) = mpsc::channel();
        if let Some(stdout) = child.stdout.take() {
            forward(stdout, sender.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            forward(stderr, sender);
        }
        let stdin = child.stdin.take();
        Ok(Self {
            child,
            stdin,
            console,
            log: Vec::new(),
        })
    }

    /// Waits until `address` accepts a TCP connection. Returns `false` if
    /// the server exits or `timeout` passes first.
    pub fn wait_until_reachable(&mut self, address: SocketAddr, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if TcpStream::connect_timeout(&address, POLL).is_ok() {
                return true;
            }
            if !matches!(self.child.try_wait(), Ok(None)) {
                return false;
            }
            thread::sleep(POLL);
        }
        false
    }

    /// Waits for the server to exit on its own.
    pub fn wait_for_exit(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status);
            }
            thread::sleep(POLL);
        }
        None
    }

    /// Asks the server to stop with the `stop` console command, and kills it
    /// if it is still running after `timeout`. Returns the console output.
    pub fn stop(&mut self, timeout: Duration) -> io::Result<Vec<String>> {
        if let Some(mut stdin) = self.stdin.take() {
            // The server may already have exited and closed its input.
            let _ = stdin.write_all(b"stop\n");
        }
        if self.wait_for_exit(timeout).is_none() {
            self.child.kill()?;
            self.child.wait()?;
        }
        while let Ok(line) = self.console.recv_timeout(Duration::from_secs(2)) {
            self.log.push(line);
        }
        Ok(std::mem::take(&mut self.log))
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Sends every console line to `sender` until the output closes. The
/// output is always drained, so the server never blocks on a full pipe.
fn forward(output: impl Read + Send + 'static, sender: Sender<String>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(output);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&buffer);
            let _ = sender.send(line.trim_end().to_string());
        }
    });
}
