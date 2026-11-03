//! Generates Rust tables from vanilla's data generator reports.
//!
//! The reports are produced by `scripts/gen-reports.sh` into `.vanilla/reports/`
//! (or the directory in `CERIUM_VANILLA_REPORTS`). They are derived from
//! Mojang's game, so they are never committed: every build regenerates the
//! tables from them.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

const HINT: &str = "run `bash scripts/fetch-vanilla.sh`, then `bash scripts/gen-reports.sh`";

fn main() {
    println!("cargo::rerun-if-env-changed=CERIUM_VANILLA_REPORTS");
    let reports = reports_dir();
    let registries = read_json(&reports.join("registries.json"));
    let blocks = read_json(&reports.join("blocks.json"));
    let registries = as_object(&registries, "registries.json");
    let blocks = as_object(&blocks, "blocks.json");

    let mut out = String::new();
    write_registries(&mut out, registries);
    write_blocks(&mut out, blocks, registries);

    let dest = cargo_env("OUT_DIR").join("vanilla_reports.rs");
    if let Err(err) = fs::write(&dest, out) {
        panic!("cannot write {}: {err}", dest.display());
    }
}

/// Reads a path from an environment variable that Cargo always sets.
fn cargo_env(name: &str) -> PathBuf {
    match env::var_os(name) {
        Some(value) => PathBuf::from(value),
        None => panic!("Cargo did not set {name}"),
    }
}

fn reports_dir() -> PathBuf {
    match env::var_os("CERIUM_VANILLA_REPORTS") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let manifest_dir = cargo_env("CARGO_MANIFEST_DIR");
            manifest_dir.join("../../.vanilla/reports")
        }
    }
}

fn read_json(path: &Path) -> Value {
    println!("cargo::rerun-if-changed={}", path.display());
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => panic!("cannot read {}: {err} ({HINT})", path.display()),
    };
    match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => panic!("invalid JSON in {}: {err}", path.display()),
    }
}

fn as_object<'a>(value: &'a Value, what: &str) -> &'a Map<String, Value> {
    match value.as_object() {
        Some(object) => object,
        None => panic!("{what}: expected a JSON object"),
    }
}

fn as_u64(value: &Value, what: &str) -> u64 {
    match value.as_u64() {
        Some(number) => number,
        None => panic!("{what}: expected an unsigned integer"),
    }
}

/// Panics unless `ids` holds exactly the numbers `0..ids.len()`.
fn assert_contiguous(mut ids: Vec<u64>, what: &str) {
    ids.sort_unstable();
    let expected = 0..ids.len() as u64;
    let contiguous = ids.into_iter().eq(expected);
    assert!(contiguous, "{what}: IDs are not contiguous from 0");
}

fn write_registries(out: &mut String, registries: &Map<String, Value>) {
    let mut registry_ids = Vec::new();
    out.push_str("/// Built-in registries, sorted by name.\n");
    out.push_str("pub const REGISTRIES: &[RegistryInfo] = &[\n");
    for (name, registry) in registries {
        let protocol_id = as_u64(&registry["protocol_id"], name);
        registry_ids.push(protocol_id);
        let default = registry.get("default").and_then(Value::as_str);
        let entries = as_object(&registry["entries"], name);
        let entry_ids = entries
            .iter()
            .map(|(entry, value)| as_u64(&value["protocol_id"], entry))
            .collect();
        assert_contiguous(entry_ids, name);
        let count = entries.len();
        let line = format!(
            "    RegistryInfo {{ name: {name:?}, protocol_id: {protocol_id}, \
             default: {default:?}, entry_count: {count} }},\n"
        );
        out.push_str(&line);
    }
    out.push_str("];\n\n");
    assert_contiguous(registry_ids, "registries.json");
}

fn write_blocks(out: &mut String, blocks: &Map<String, Value>, registries: &Map<String, Value>) {
    let block_entries = &registries["minecraft:block"]["entries"];
    let block_registry = as_object(block_entries, "minecraft:block");
    assert_eq!(
        blocks.len(),
        block_registry.len(),
        "blocks.json and the minecraft:block registry list different numbers of blocks"
    );

    let mut state_ids = Vec::new();
    for (name, block) in blocks {
        assert!(
            block_registry.contains_key(name),
            "{name} is in blocks.json but not in the minecraft:block registry"
        );
        let states = match block["states"].as_array() {
            Some(states) => states,
            None => panic!("{name}: expected a states array"),
        };
        let defaults = states
            .iter()
            .filter(|state| state["default"].as_bool() == Some(true))
            .count();
        assert_eq!(defaults, 1, "{name} must have exactly one default state");
        for state in states {
            state_ids.push(as_u64(&state["id"], name));
        }
    }

    let block_count = blocks.len();
    let state_count = state_ids.len();
    assert_contiguous(state_ids, "blocks.json block state");
    let consts = format!(
        "/// Number of blocks in the pinned vanilla version.\n\
         pub const BLOCK_COUNT: usize = {block_count};\n\n\
         /// Number of block states. Global block state IDs run from 0 to\n\
         /// `BLOCK_STATE_COUNT - 1`.\n\
         pub const BLOCK_STATE_COUNT: usize = {state_count};\n"
    );
    out.push_str(&consts);
}
