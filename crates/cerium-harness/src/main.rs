//! `cerium-parity` runs every parity scenario against vanilla and CeriumMC
//! and reports the differences (roadmap step 4).
//!
//! Vanilla's output is never stored in the repository: the harness runs the
//! vanilla jar live every time, and only the pass/fail expectations are
//! committed.

mod diff;
mod proxy;
mod scenario;
mod server;
mod wire;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use diff::Line;
use scenario::{Context, Observation, SCENARIOS, Scenario};
use server::Target;

const USAGE: &str = "\
usage: cerium-parity --vanilla <server.jar> --cerium <ceriummc executable>
                     [--expect <file>] [--pins <file>] [--out <dir>]
                     [--only <scenario>]

  --expect  pass/fail expectations (default: parity/expected.toml)
  --pins    pinned vanilla version (default: vanilla.toml)
  --out     where observations are written (default: target/parity)
  --only    run a single scenario";

const FLAGS: &[&str] = &[
    "--vanilla",
    "--cerium",
    "--expect",
    "--pins",
    "--out",
    "--only",
];

/// Most changed lines printed per observation, to keep reports readable.
const MAX_DIFF_LINES: usize = 60;

struct Options {
    vanilla: PathBuf,
    cerium: PathBuf,
    expect: PathBuf,
    pins: PathBuf,
    out: PathBuf,
    only: Option<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_string())
}

fn run() -> io::Result<bool> {
    let options = parse_args(std::env::args().skip(1).collect())?;
    let protocol = read_pin(&options.pins, "protocol")?;
    let Ok(protocol) = protocol.parse() else {
        return Err(invalid("the pinned protocol is not a number"));
    };
    let expected = read_expectations(&options.expect)?;
    for name in expected.keys() {
        if !SCENARIOS.iter().any(|scenario| scenario.name == name) {
            let message = format!("unknown scenario in expectations: {name}");
            return Err(invalid(&message));
        }
    }

    let vanilla = Target::Vanilla {
        jar: std::path::absolute(&options.vanilla)?,
    };
    let cerium = Target::Cerium {
        exe: std::path::absolute(&options.cerium)?,
    };

    let mut all_ok = true;
    for scenario in SCENARIOS {
        let selected = match &options.only {
            Some(only) => only == scenario.name,
            None => true,
        };
        if !selected {
            continue;
        }
        let Some(&expect_pass) = expected.get(scenario.name) else {
            let message = format!("add {} to the expectations file", scenario.name);
            return Err(invalid(&message));
        };
        println!("== {}", scenario.name);
        let passed = run_scenario(scenario, &vanilla, &cerium, protocol, &options.out)?;
        let verdict = match (passed, expect_pass) {
            (true, true) => "PASS",
            (false, false) => "KNOWN FAILURE (expected)",
            (true, false) => "NOW PASSES: mark it \"pass\" in the expectations file",
            (false, true) => "REGRESSION: this scenario is expected to pass",
        };
        if passed != expect_pass {
            all_ok = false;
        }
        println!("== {}: {verdict}\n", scenario.name);
    }
    Ok(all_ok)
}

fn parse_args(args: Vec<String>) -> io::Result<Options> {
    let mut flags = BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        if !FLAGS.contains(&flag.as_str()) {
            return Err(invalid(&format!("unknown argument {flag}\n\n{USAGE}")));
        }
        let Some(value) = args.next() else {
            return Err(invalid(&format!("{flag} needs a value\n\n{USAGE}")));
        };
        flags.insert(flag, value);
    }

    let get = |flag: &str| flags.get(flag).map(PathBuf::from);
    let Some(vanilla) = get("--vanilla") else {
        return Err(invalid(USAGE));
    };
    let Some(cerium) = get("--cerium") else {
        return Err(invalid(USAGE));
    };
    Ok(Options {
        vanilla,
        cerium,
        expect: or_default(get("--expect"), "parity/expected.toml"),
        pins: or_default(get("--pins"), "vanilla.toml"),
        out: or_default(get("--out"), "target/parity"),
        only: flags.get("--only").cloned(),
    })
}

fn or_default(value: Option<PathBuf>, default: &str) -> PathBuf {
    match value {
        Some(path) => path,
        None => PathBuf::from(default),
    }
}

/// Reads `key = value` from the pins file, without surrounding quotes.
fn read_pin(path: &Path, key: &str) -> io::Result<String> {
    let text = fs::read_to_string(path)?;
    let prefix = format!("{key} = ");
    for line in text.lines() {
        if let Some(value) = line.strip_prefix(&prefix) {
            return Ok(value.trim().trim_matches('"').to_string());
        }
    }
    let message = format!("{key} is missing from {}", path.display());
    Err(invalid(&message))
}

/// Reads lines like `first-run = "fail"`. Returns whether each scenario is
/// expected to pass.
fn read_expectations(path: &Path) -> io::Result<BTreeMap<String, bool>> {
    let text = fs::read_to_string(path)?;
    let mut expected = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            return Err(invalid(&format!("bad expectation line: {line}")));
        };
        let pass = match value.trim() {
            "\"pass\"" => true,
            "\"fail\"" => false,
            other => {
                let message = format!("expected \"pass\" or \"fail\", got {other}");
                return Err(invalid(&message));
            }
        };
        expected.insert(name.trim().to_string(), pass);
    }
    Ok(expected)
}

/// Runs one scenario on both targets and prints the differences. Returns
/// whether every observation matched.
fn run_scenario(
    scenario: &Scenario,
    vanilla: &Target,
    cerium: &Target,
    protocol: u32,
    out: &Path,
) -> io::Result<bool> {
    let base = out.join(scenario.name);
    // A vanilla failure means the harness itself is broken, so it is fatal.
    let expected = match run_target(scenario, vanilla, protocol, &base) {
        Ok(observations) => observations,
        Err(err) => {
            let message = format!("vanilla failed in {}: {err}", scenario.name);
            return Err(io::Error::other(message));
        }
    };
    // A CeriumMC failure is just a parity difference.
    let actual = match run_target(scenario, cerium, protocol, &base) {
        Ok(observations) => observations,
        Err(err) => vec![Observation {
            name: "error".to_string(),
            text: err.to_string(),
        }],
    };
    Ok(report(&expected, &actual))
}

/// Runs a scenario in a fresh directory and saves each observation to
/// `<out>/<scenario>/<target>/<observation>.txt` for inspection.
fn run_target(
    scenario: &Scenario,
    target: &Target,
    protocol: u32,
    base: &Path,
) -> io::Result<Vec<Observation>> {
    let dir = base.join(target.name());
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    let run_dir = dir.join("run");
    fs::create_dir_all(&run_dir)?;
    let cx = Context {
        target,
        dir: &run_dir,
        protocol,
    };
    let observations = (scenario.run)(&cx)?;
    for observation in &observations {
        let file = dir.join(format!("{}.txt", observation.name));
        fs::write(file, &observation.text)?;
    }
    Ok(observations)
}

fn find<'a>(observations: &'a [Observation], name: &str) -> &'a str {
    for observation in observations {
        if observation.name == name {
            return &observation.text;
        }
    }
    "<missing>"
}

/// Prints the changed lines of every observation that differs. Returns
/// whether all observations match.
fn report(vanilla: &[Observation], cerium: &[Observation]) -> bool {
    let mut names: Vec<&str> = Vec::new();
    for observation in vanilla.iter().chain(cerium) {
        if !names.contains(&observation.name.as_str()) {
            names.push(&observation.name);
        }
    }

    let mut all_match = true;
    for name in names {
        let expected = find(vanilla, name);
        let actual = find(cerium, name);
        if expected == actual {
            println!("   {name}: same");
            continue;
        }
        all_match = false;
        println!("   {name}: differs (- vanilla, + cerium)");
        let lines = diff::diff(expected, actual);
        let changed: Vec<&Line<'_>> = lines
            .iter()
            .filter(|line| !matches!(line, Line::Same(_)))
            .collect();
        for line in changed.iter().take(MAX_DIFF_LINES) {
            match line {
                Line::Vanilla(text) => println!("     - {text}"),
                Line::Cerium(text) => println!("     + {text}"),
                Line::Same(_) => {}
            }
        }
        if changed.len() > MAX_DIFF_LINES {
            let more = changed.len() - MAX_DIFF_LINES;
            println!("     ... and {more} more changed lines");
        }
    }
    all_match
}
