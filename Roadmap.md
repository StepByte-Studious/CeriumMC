# CeriumMC Roadmap: Vanilla Parity

> A ground-up Rust implementation of the Minecraft dedicated server that behaves 1:1 like vanilla.

## Target

| Field | Value |
| --- | --- |
| Vanilla version | **26.3** (stable, built 2026-09-15) |
| Protocol version | **777** |
| World (data) version | **5023** |
| Data pack format | 121.0 |
| Resource pack format | 97.1 |
| Upstream reference | Official `server.jar` (SHA-1 `33680f5f…e05f4c`), pinned in [`vanilla.toml`](vanilla.toml). Local research copy extracted in `.cached/` |

Path shorthand used throughout this document:

- `JAR/` = `.cached/META-INF/versions/26.3/extracted-26.3jar/`
- `nm/` = `JAR/net/minecraft/`
- `data/` = `JAR/data/minecraft/`

## Clean-room rules

1. **We study behaviour, never transcribe code.** We may read data files (`data/`), generated reports, protocol behaviour and the observed in-game results of the vanilla jar. We may also use class and package *names* to know where a system lives. We never copy-paste decompiled code or translate it line by line into Rust.
2. **Every research finding goes into `docs/research/<topic>.md`, written in our own words.** That doc covers what the system does, its edge cases and how we measured them. Rust code is written from those notes, not from the decompiled source.
3. **Vanilla is the oracle.** When we're unsure about behaviour, we run the vanilla jar and measure it. Tests compare CeriumMC output to vanilla output.
4. **We don't redistribute Mojang assets or data.** Generated tables are produced at build time from a vanilla jar fetched and hash-checked in CI, or from the local `.cached` copy for research. They are never committed to the repo.

## Definition of "done" (parity)

A system is at parity when all of these hold:

- Its differential tests pass against the vanilla 26.3 jar.
- A vanilla 26.3 client cannot tell the difference: same packets, same world, same outcomes.
- Data saved by CeriumMC loads in vanilla, and data saved by vanilla loads in CeriumMC, through the compatibility layer.

## Architectural constraints from day one

Our own goals (efficiency, speed, storage, plugins) come *after* parity. They still shape the architecture now, so we don't have to rewrite later:

| Goal | Constraint applied from the first commit |
| --- | --- |
| ≤ 1 GB RAM for 20–30 players | A memory budget for each chunk, entity and player, tracked in CI (step 5). Paletted containers stay compact in memory. No per-block heap objects. Arena or slab allocation for hot data. |
| Near-instant boot | Registries, block states and shapes are generated at build time into static tables. Vanilla builds these at startup. Lazy datapack parsing where it doesn't change behaviour. Spawn-chunk preparation runs in parallel. |
| 80–90% smaller worlds | All persistence goes through a `WorldStorage` trait (step 33). Anvil `.mca` is *one backend*, used for compatibility and import/export. The custom format plugs in later. |
| Plugin API | An internal event/hook layer is reserved at the boundaries: join, chat, block change, entity spawn, command. It does nothing until the plugin API phase. |
| Any CPU architecture | Runs on x86_64, AArch64 and 32-bit ARM. That covers desktops and servers, Apple Silicon, ARM cloud VMs, a Raspberry Pi and Android phones (Termux). Code is portable Rust only: no x86-only crates or intrinsics without a scalar fallback, and SIMD is chosen at runtime by feature detection. Game math gives bit-identical results on every architecture: IEEE `f64`/`f32` only, no fused multiply-add (FMA), no platform `libm` differences in gameplay paths. |
| Any core count | Adapts to the hardware *by design*. The same binary runs well on 1 core or on 64. A single deterministic tick thread owns game state. Chunk generation, lighting, compression and I/O run as jobs on a worker pool sized from the detected core count. On a single core, that pool collapses into cooperative jobs run on the same thread between ticks. Results never depend on thread count or scheduling, so a 1-core and a 32-core run of the same seed and inputs produce identical worlds. No busy-waiting and no spin locks. Idle CPU is near zero. |
| Any storage (SSD, HDD, SD card, eMMC) | Fast on slow disks too. I/O is batched and written sequentially: region writes are coalesced, fsync is not called per chunk, and reads are read-ahead. All of it runs asynchronously, off the tick thread, so disk latency never stalls a tick. The number of open files and the write amplification are kept low, which protects SD cards and flash. Hot chunks are cached within the RAM budget. |

## Development workflow: GitHub CI only

**There is no local Rust toolchain on the development PC.** GitHub Actions does all building, linting, testing, benchmarking and report generation. Every step's "Done when" therefore means *green in GitHub CI*.

- **Feedback loop:** push the branch, CI runs, read the logs and artifacts.
- **CI fetches the vanilla jar itself.** It downloads the 26.3 server jar from Mojang's official launcher metadata at runtime, checks it against the pinned SHA-256 and caches it. It never commits the jar. Vanilla reports are produced in CI and kept as CI artifacts, never committed.
- **Build matrix:** `x86_64` Linux, Windows and macOS. `aarch64` Linux (native ARM runners), `aarch64` macOS, `aarch64-linux-android` (cross-compiled), and `armv7-unknown-linux-gnueabihf` (Raspberry Pi, cross-compiled and tested under QEMU).
- **Release artifacts:** CI produces binaries for every target in the matrix.

---

## Phase 0: Foundation & research infrastructure

### 1. Workspace bootstrap
- **Status:** ✅ Done (2026-11-03)
- **Goal:** Set up the repository skeleton and engineering standards.
- **Upstream reference:** n/a
- **Deliverables:**
  - A Cargo workspace with crates: `cerium-util`, `cerium-nbt`, `cerium-codec`, `cerium-protocol`, `cerium-registry`, `cerium-world`, `cerium-worldgen`, `cerium-entity`, `cerium-server`, `cerium-bin`.
  - GitHub Actions workflows: `fmt` and `clippy` (deny warnings), build and test across the full target matrix (see "Development workflow"), cross-compilation through `cross` or the `cargo-zigbuild` tool, QEMU test runs for ARMv7 and Android targets, and a dependency cache.
  - A dependency policy: every crate must build on all matrix targets, and C dependencies need explicit approval.
  - `LICENSE`, and `CONTRIBUTING.md` covering the clean-room policy and the CI-only workflow.
- **Done when:** GitHub CI is green on every matrix target for an empty workspace, and the policy doc is merged.

### 2. Research process
- **Status:** ✅ Done (2026-11-03)
- **Goal:** Make studying upstream repeatable and auditable.
- **Upstream reference:** `JAR/`, `.cached/version.json`
- **Deliverables:**
  - `docs/research/README.md` (the process and allowed sources), `docs/research/_TEMPLATE.md` (sections: system, observed behaviour, edge cases, measurement method, open questions, sources), and `docs/research/INDEX.md` (maps every roadmap step to its notes).
  - `vanilla.toml`: the pinned version, URLs, sizes, SHA-1 and SHA-256 hashes, all verified against Mojang's official metadata.
  - `scripts/fetch-vanilla.sh`, which:
    - cross-checks the pins against Mojang's live metadata;
    - downloads the jar and verifies its size, SHA-1 and SHA-256, and the version facts embedded in it;
    - verifies the hash of the inner jar.
    - Its `--check-cached` mode verifies the local `.cached/` copy offline.
  - A CI job that lints the script with shellcheck, caches the jar and runs the full verification.
- **Done when:** The research docs exist, and the CI fetch-and-verify job is green.

### 3. Vanilla data-generator pipeline
- **Status:** ✅ Done (2026-11-03)
- **Goal:** Produce machine-readable vanilla facts without reading code.
- **Upstream reference:** The vanilla jar's built-in data generator (`--reports`), and `data/`
- **Deliverables:**
  - `scripts/gen-reports.sh` runs the pinned jar's data generator (Java 25) into `.vanilla/reports/`. Reports produced: `blocks.json`, `registries.json`, `packets.json`, `commands.json`, `datapack.json`, `json-rpc-api-schema.json`, biome parameters and default item components.
  - A pinned fingerprint of the reports (`reports_sha256` in `vanilla.toml`), proving that every machine produces identical output.
  - A CI job that generates the reports and shares them with every build job through the private Actions cache. They are never committed or uploaded as public artifacts.
  - `build.rs` code generation in `cerium-registry`. It reads the reports, validates them (contiguous IDs, one default state per block, the block report matches the block registry), and generates the registry and block-state tables.
- **Done when:** A clean CI run produces the reports with the pinned fingerprint, and the code generation compiles and passes its tests on every matrix target.

### 4. Differential parity harness
- **Goal:** Infrastructure that compares CeriumMC with vanilla automatically.
- **Upstream reference:** The vanilla server jar, run live
- **Deliverables:**
  - The `cerium-harness` crate, with the `cerium-parity` binary.
  - **Server launcher:** runs vanilla or CeriumMC in a fresh directory, waits until it is reachable, sends console commands, and stops it cleanly.
  - **Recording proxy:** a transparent TCP proxy that records both directions of a connection. A frame decoder turns the recording into a readable packet list.
  - **Test client:** a minimal Minecraft client (status ping). It is deliberately independent of `cerium-protocol`, so the tests never share code with the server they check.
  - **Scenarios and diff:** scenarios record named observations, and a line diff shows exactly where CeriumMC differs from vanilla. The first scenarios are `first-run` and `status-ping`.
  - **`parity/expected.toml`:** the pass/fail state of each scenario. CI fails on a regression, and also when a scenario starts passing without this file being updated.
  - Vanilla output is never committed (clean-room rule 4). The harness runs vanilla live on every CI run instead of storing golden files.
- **Done when:** CI runs every scenario against both servers, produces a readable diff, and matches `parity/expected.toml`.

### 5. Benchmark and budget harness
- **Goal:** Measure the four core goals continuously.
- **Upstream reference:** The vanilla jar, used as a baseline
- **Deliverables:**
  - `criterion` micro-benchmarks, plus an end-to-end bench that records boot time, RSS at N simulated players, MSPT and world size on disk.
  - Hardware profiles run in CI:
    - **multi-core** (all runner cores);
    - **single-core** (pinned to 1 CPU with `taskset` or a cgroup limit);
    - **low-RAM** (cgroup memory cap of 1 GB);
    - **slow-disk** (cgroup `io.max` throttling to HDD and SD-card speeds);
    - **ARM** (native `aarch64` runners).
  - A Raspberry Pi class profile: a self-hosted runner if one becomes available, or QEMU `armv7`/`aarch64` with speed figures normalized.
  - Results stored for each commit. CI fails on regressions beyond a set threshold in any profile.
- **Done when:** The vanilla baseline numbers are recorded, and CeriumMC numbers for every profile are reported on every PR.

### 6. Logging, errors and crash reports
- **Goal:** Operator-visible output in vanilla's format.
- **Upstream reference:** `nm/CrashReport*`, `nm/SystemReport`, the vanilla `logs/latest.log` format
- **Deliverables:**
  - Console and log formatting (based on `tracing`) that matches vanilla's line layout.
  - Log rotation (`logs/YYYY-MM-DD-N.log.gz`).
  - Crash reports written to `crash-reports/` with the same section layout.
  - The watchdog (`max-tick-time`).
- **Done when:** A log diff of the startup and shutdown sequence differs from vanilla only in timestamps and paths.

### 7. Configuration and CLI
- **Goal:** Identical server bootstrap files and flags.
- **Upstream reference:** `nm/server/dedicated` (properties handling), and a fresh vanilla run's `server.properties` and `eula.txt`
- **Deliverables:**
  - `server.properties` read and write covering every key, default, ordering and comment header, plus vanilla's tolerance for invalid values.
  - `eula.txt` gating.
  - CLI flags: `--nogui`, `--port`, `--world`, `--universe`, `--initSettings`, `--forceUpgrade`, `--eraseCache`, `--recreateRegionFiles`, `--safeMode`, `--demo`, `--bonusChest`, `--serverId`, `--jfrProfile`, `--pidFile`.
- **Done when:** First-run output files are byte-identical to vanilla's, excluding timestamps.

### 8. Threading and runtime model
- **Goal:** Lock in the concurrency architecture before any game code exists.
- **Upstream reference:** `nm/server`, `nm/util/thread` (observed tick and worker split)
- **Deliverables:** An ADR (architecture decision record) covering:
  - **Hardware-adaptive execution:**
    - Core count is detected at startup and can be overridden in config.
    - On multi-core, a worker pool runs chunk generation, lighting, compression and I/O.
    - On single-core, one thread runs everything: the tick comes first, then queued jobs fill the gap until the next tick deadline.
  - Async networking (a tokio runtime on multi-core, a current-thread runtime on single-core);
  - one deterministic main tick thread that owns game state;
  - a job system with priorities (tick-critical > player-visible chunks > background);
  - message passing between threads, with no shared mutable game state;
  - **determinism guarantee:** the order of results applied to the world never depends on thread count or scheduling;
  - the I/O model for slow storage: batched, sequential, async writes with back-pressure;
  - the shutdown sequence;
  - where the plugin hook seam sits.
- **Done when:**
  - The ADR is approved.
  - A prototype runs an empty 20 TPS loop with clean shutdown on Ctrl-C and `stop`, in both 1-core and N-core modes.
  - A CI test proves identical output in 1-thread and N-thread runs of a job-heavy workload.

---

## Phase 1: Core primitives

### 9. Positions, shapes and math
- **Goal:** Bit-exact spatial primitives.
- **Upstream reference:** `nm/core` (BlockPos, SectionPos, Direction), `nm/world/phys`, `nm/util/Mth`
- **Deliverables:**
  - `BlockPos`, `SectionPos`, `ChunkPos`, `Vec3`, `AABB`, `Direction`, `Axis`.
  - Packed `i64` encodings.
  - Java-compatible math helpers: floor/ceil semantics, `sin`/`cos` lookup-table behaviour, Java `float` vs `double` rounding.
- **Done when:** Property tests match the vanilla outputs captured for millions of sampled inputs, with identical results on every CI matrix architecture (x86_64, AArch64, ARMv7).

### 10. Java-compatible random sources
- **Goal:** Deterministic RNG, identical to vanilla, for worldgen and gameplay.
- **Upstream reference:** `nm/world/level/levelgen` (LegacyRandomSource, Xoroshiro, PositionalRandomFactory), `nm/util/RandomSource`, `nm/util/random`
- **Deliverables:** A `java.util.Random`-equivalent LCG, Xoroshiro128++, positional and forked factories, seed mixing, `String.hashCode` semantics for seeds, weighted random lists.
- **Done when:** Sequences match vanilla for fixed seeds across every factory type.

### 11. Noise primitives
- **Goal:** Bit-exact noise.
- **Upstream reference:** `nm/world/level/levelgen/synth`, `data/worldgen/noise`
- **Deliverables:** Improved Perlin, simplex, perlin octaves, normal noise, blended noise.
- **Done when:** A grid of samples for each `worldgen/noise` entry matches vanilla bit-for-bit (`f64::to_bits`).

### 12. Identifiers and text components
- **Goal:** Resource identifiers and the chat component model.
- **Upstream reference:** `nm/IdentifierException`, `nm/network/chat`, `nm/ChatFormatting`
- **Deliverables:** Identifier parsing and validation rules, the `Component` tree (text, translatable, score, selector, keybind, NBT, object), styles, click and hover events, JSON and NBT codecs.
- **Done when:** A round-trip corpus of vanilla components serializes identically.

### 13. NBT
- **Goal:** The full NBT stack.
- **Upstream reference:** `nm/nbt`
- **Deliverables:**
  - All 13 tag types.
  - Binary read and write (named and network/nameless root).
  - SNBT parser and printer, including vanilla's quoting and number-suffix rules.
  - Accounting for depth (512) and size quotas.
  - Zero-copy readers for hot paths.
- **Done when:** Fuzzing passes, and round-trips of vanilla `level.dat`, player and chunk files are byte-identical.

### 14. Codec framework
- **Goal:** A Rust analogue of data-driven encode and decode with vanilla error behaviour.
- **Upstream reference:** The `datafixerupper` library's role (behaviour only), and `data/` JSON shapes
- **Deliverables:**
  - Derive macros for decoding from JSON, NBT and network.
  - Partial results and error accumulation.
  - Optional fields with vanilla defaults.
  - "Either" and dispatch (`type` field) codecs.
- **Done when:** Every file under `data/` decodes, and the files that should fail produce matching errors.

### 15. Data components
- **Goal:** The item component system.
- **Upstream reference:** `nm/core/component`, `nm/world/item/component`
- **Deliverables:** A typed component map, prototypes and patches (added and removed), network and NBT codecs, hashed component representations for the protocol.
- **Done when:** Every item's default components match the vanilla report, and patch round-trips are exact.

### 16. Data-version and upgrade layer
- **Goal:** Load worlds from older vanilla versions.
- **Upstream reference:** `nm/util/datafix`, `nm/util/datafix/fixes`
- **Deliverables:**
  - A decision on the minimum supported import version (proposal: 1.20.5 and later).
  - Our own schema-based upgrade passes for chunk, level, player, entity and structure NBT.
  - `--forceUpgrade` support.
- **Done when:** Sample worlds from each supported version load, and they match vanilla's upgraded output.

---

## Phase 2: Networking

### 17. Transport layer
- **Goal:** The wire format.
- **Upstream reference:** `nm/network`
- **Deliverables:** VarInt and VarLong, length-prefixed framing, zlib compression with a threshold, AES/CFB8 encryption, backpressure, per-connection buffers sized for low RAM.
- **Done when:** Fuzzing passes, and the codec reads and writes captured vanilla traffic.

### 18. Packet definitions and protocol states
- **Goal:** Every packet, generated from reports.
- **Upstream reference:** `nm/network/protocol/*`, `packets.json`
- **Deliverables:**
  - Codegen for packet IDs in every state and direction.
  - Typed packet structs.
  - The Handshake → Status | Login → Configuration ⇄ Play state machine.
  - Bundle packets.
- **Done when:** Every packet ID in `packets.json` maps to a typed struct, and an encode/decode round-trip test exists for each one.

### 19. Status and server-list ping
- **Goal:** The server appears correctly in the multiplayer list.
- **Upstream reference:** `nm/network/protocol/status`, `nm/server/network`
- **Deliverables:** MOTD, version and protocol, player count and sample, `server-icon.png` favicon, `enforcesSecureChat`, pong, the legacy `0xFE` ping.
- **Done when:** A vanilla client shows identical list entries for both servers.

### 20. Login
- **Goal:** Authenticate players.
- **Upstream reference:** `nm/network/protocol/login`, `nm/network/protocol/cookie`, `nm/server/network`
- **Deliverables:**
  - Offline mode with UUID derivation.
  - Online mode: RSA keypair, shared secret, Mojang session-server `hasJoined`, `prevent-proxy-connections`.
  - Set-compression, login plugin messages, cookies, transfer packets, `accepts-transfers`.
- **Done when:** A real client logs in under both modes, and a mismatched-key login is rejected exactly as vanilla rejects it.

### 21. Configuration phase
- **Goal:** Client setup before play.
- **Upstream reference:** `nm/network/protocol/configuration`, `nm/network/protocol/common`, `nm/server/packs`
- **Deliverables:**
  - Known-packs negotiation.
  - Registry data sync, omitting entries the client already knows.
  - Feature flags and tag sync.
  - Server resource-pack push (required or optional, with a prompt).
  - Custom report details, server links, server-pushed dialogs and code of conduct.
  - Re-entering configuration from play.
- **Done when:** Packet captures of the configuration phase match vanilla's ordering and contents.

### 22. Play bootstrap (M1)
- **Goal:** The player spawns into a world.
- **Upstream reference:** `nm/network/protocol/game`, `nm/server/players`, `nm/server/level`
- **Deliverables:** The login (join game) packet, dimension and world info, spawn position, abilities, initial teleport and teleport confirmation, chunk-batch start and finish, game-event "start waiting for chunks", an empty or void world.
- **Done when:** **M1:** A vanilla client joins a void world and can look around with no errors.

### 23. Connection lifecycle
- **Goal:** Robust connections.
- **Upstream reference:** `nm/server/network`, `nm/network/protocol/common`
- **Deliverables:** Keep-alive, ping/pong, timeouts, disconnect reasons, rate limiting (`rate-limit`), maximum packet sizes, invalid-packet handling, `network-compression-threshold`.
- **Done when:** Misbehaving-client tests produce vanilla's disconnect messages.

### 24. Chat and secure profiles
- **Goal:** Vanilla chat, including signed chat.
- **Upstream reference:** `nm/network/chat`, `data/chat_type`
- **Deliverables:** Chat sessions and profile public keys, message signing and chain validation, last-seen tracking, chat types, `enforce-secure-profile`, command argument signing, system and disguised messages.
- **Done when:** Chat between two real clients shows as verified, and tampered messages trigger vanilla's behaviour.

### 25. Synched entity data and tracking
- **Goal:** The entity metadata protocol.
- **Upstream reference:** `nm/network/syncher`, `nm/server/level` (entity tracking)
- **Deliverables:** Metadata serializers for every entity type, dirty tracking, tracking ranges for each entity category, the logic for which entity-update packet to send (absolute or relative move, rotation, velocity, head yaw).
- **Done when:** The entity packet stream matches vanilla for scripted entity scenarios.

### 26. Network compatibility suite
- **Goal:** Lock down the protocol.
- **Upstream reference:** Step 4 harness
- **Deliverables:** A replay suite of captured vanilla sessions, and a nightly real-client smoke test (join, move, chat, disconnect).
- **Done when:** The suite runs in CI, with zero protocol diffs on the covered scenarios.

---

## Phase 3: Registries & data-driven content

### 27. Static registries and block states
- **Goal:** Every built-in registry, with IDs identical to vanilla.
- **Upstream reference:** `nm/core/registries`, `blocks.json`, `registries.json`
- **Deliverables:** Codegen for blocks, items, entity types, fluids, sounds, particles and so on. Block-state property permutations, global state IDs and default states. All tables are static, with no runtime construction.
- **Done when:** Every registry ID and every block-state ID matches the reports.

### 28. Block property tables
- **Goal:** The static per-block data.
- **Upstream reference:** `nm/world/level/block`, `nm/world/phys/shapes`, `nm/world/level/material`
- **Deliverables:** Hardness, blast resistance, friction, speed and jump factors, light emission and opacity, collision, outline, occlusion and interaction shapes (VoxelShape with an exact merge and optimize algorithm), sound types, map colours, instrument, push reaction, replaceability.
- **Done when:** A property dump for every state matches values measured from vanilla.

### 29. Dynamic (datapack) registries
- **Goal:** Registries loaded from data.
- **Upstream reference:** `data/` (dimension_type, worldgen/biome, damage_type, chat_type, enchantment, *_variant, painting_variant, jukebox_song, instrument, trim_*, banner_pattern, dialog, timeline, world_clock, trade_set, villager_trade, …)
- **Deliverables:** Loaders for every dynamic registry, ordering and ID assignment, and network sync payloads.
- **Done when:** The registry contents and network IDs match vanilla, and every `data/` registry folder loads.

### 30. Datapack system
- **Goal:** Datapack discovery and lifecycle.
- **Upstream reference:** `nm/server/packs`, `data/datapacks` (minecart_improvements, redstone_experiments, trade_rebalance)
- **Deliverables:** Vanilla, built-in feature and world datapacks. `pack.mcmeta` with supported formats and overlays, filters, feature flags, `/datapack` enable/disable/list, `/reload` semantics, `initial-enabled-packs` and `initial-disabled-packs`.
- **Done when:** Enabling and disabling the experimental packs changes content exactly as in vanilla.

### 31. Tags
- **Goal:** The tag system.
- **Upstream reference:** `nm/tags`, `data/tags`
- **Deliverables:** Tag loading, `replace` semantics, nested tag references, optional entries, per-registry tag sets, network sync.
- **Done when:** Every resolved tag matches vanilla's resolved contents.

### 32. Loot, predicates, recipes and functions (loading)
- **Goal:** Parse the remaining data-driven content.
- **Upstream reference:** `nm/world/level/storage/loot`, `data/loot_table`, `data/predicate`, `data/recipe`, `nm/world/item/crafting`
- **Deliverables:** Loot tables, pools, entries, conditions, functions and number providers. Predicates and item modifiers. Every recipe serializer. `.mcfunction` loading and parsing (execution comes in step 71).
- **Done when:** All vanilla data loads with zero errors, and seeded loot rolls match vanilla.

---

## Phase 4: World storage & chunks

### 33. Level storage layout and storage trait
- **Goal:** World directory semantics behind a pluggable backend.
- **Upstream reference:** `nm/world/level/storage`
- **Deliverables:**
  - A `WorldStorage` trait.
  - `level.dat` and `level.dat_old`, `session.lock`.
  - Directories: `playerdata/`, `stats/`, `advancements/`, `data/`, dimension folders.
  - World version and compatibility checks, and safe (atomic) writes.
- **Done when:** A vanilla world opens, and a world created by CeriumMC opens in vanilla.

### 34. Anvil compatibility backend
- **Goal:** Read and write vanilla region files.
- **Upstream reference:** `nm/world/level/chunk/storage`
- **Deliverables:**
  - The `.mca` sector layout, timestamps, compression types (gzip, zlib, LZ4, none), external `.mcc` oversized chunks, the `region/`, `entities/` and `poi/` stores, `--recreateRegionFiles`.
  - An I/O layer friendly to HDDs and SD cards:
    - writes to the same region are coalesced and written in sector order;
    - reads use read-ahead in the direction of player movement;
    - a bounded LRU cache of open file handles;
    - all I/O is async and off the tick thread.
- **Done when:** Region files round-trip and load in vanilla, and in the slow-disk CI profile, saving and loading cause zero tick stalls.

### 35. Chunk data model
- **Goal:** A memory-efficient in-memory chunk.
- **Upstream reference:** `nm/world/level/chunk`
- **Deliverables:**
  - Sections.
  - Paletted containers: single-value, linear, hashmap and global palettes, with vanilla's bits-per-entry thresholds for the network format.
  - Heightmaps (all types), 3D biome storage, block-entity maps.
  - The memory budget for each chunk is measured.
- **Done when:** The network encoding of chunks matches vanilla byte-for-byte, and the per-chunk RAM budget is met.

### 36. Chunk serialization parity
- **Goal:** Exact chunk NBT.
- **Upstream reference:** `nm/world/level/chunk/storage`, `nm/world/level/chunk`
- **Deliverables:** Read and write of sections, palettes, block entities, pending block and fluid ticks, structure starts and references, light data, status, `PostProcessing`, blending data.
- **Done when:** Vanilla chunk files round-trip with semantic equality, ignoring only key order.

### 37. Chunk status pipeline and tickets
- **Goal:** Chunk loading, generation scheduling and tickets.
- **Upstream reference:** `nm/world/level/chunk/status`, `nm/server/level`
- **Deliverables:**
  - Status steps from empty through `structure_starts`, biomes, noise, surface, carvers, features, light and spawn to full.
  - The ticket system, its levels and the ticket types.
  - Forced chunks.
  - View and simulation distance, and the spawn-chunk radius gamerule.
- **Done when:** The loaded, ticking and entity-ticking chunk sets match vanilla for scripted player movement.

### 38. Chunk and light sending
- **Goal:** Stream the world to clients.
- **Upstream reference:** `nm/network/protocol/game`, `nm/server/level`
- **Deliverables:** Chunk and light packets, chunk batching with client-reported rate, view-center updates, forget-chunk, sending order (nearest first).
- **Done when:** The client renders a vanilla-generated world with no holes, and the packet pacing is comparable.

### 39. Lighting engine
- **Goal:** Sky and block light identical to vanilla.
- **Upstream reference:** `nm/world/level/lighting`
- **Deliverables:** Block-light and sky-light propagation and removal, section light storage, edge handling between chunks, the light phase of chunk generation, the lighting update queue.
- **Done when:** Light arrays are bit-identical to vanilla after generation and after scripted block edits.

### 40. Saved data and save semantics
- **Goal:** The world's non-chunk persistent state.
- **Upstream reference:** `nm/world/level/saveddata`, `nm/world/level/saveddata/maps`
- **Deliverables:** Map data, raids, scoreboard, world border, forced chunks, structure feature indexes, random sequences, command storage. Autosave intervals and `save-all`, `save-off` and `save-on` semantics. A full save on `stop`.
- **Done when:** The files in `data/` round-trip with vanilla.

---

## Phase 5: World generation

### 41. Density functions and noise router
- **Goal:** The core of terrain generation.
- **Upstream reference:** `nm/world/level/levelgen` (NoiseRouter, RandomState), `data/worldgen/density_function`, `data/worldgen/noise_settings`
- **Deliverables:** Every density-function type, plus the markers (interpolated, flat_cache, cache_2d, cache_once, cache_all_in_cell) with matching evaluation semantics. Spline evaluation. Random-state construction from the seed.
- **Done when:** Router output samples match vanilla bit-for-bit for a set of seeds.

### 42. Noise chunk generator
- **Goal:** Produce the base terrain.
- **Upstream reference:** `nm/world/level/levelgen` (NoiseBasedChunkGenerator, NoiseChunk, Aquifer), `nm/world/level/levelgen/blending`
- **Deliverables:** Cell interpolation, the aquifer system, ore veins, the initial-density height estimate, blending with old chunks.
- **Done when:** The block states after the noise step match vanilla for seeded chunks.

### 43. Biome sources
- **Goal:** Biome placement.
- **Upstream reference:** `nm/world/level/biome`, `data/worldgen/multi_noise_biome_source_parameter_list`, `data/worldgen/biome`
- **Deliverables:** The climate sampler, the multi-noise parameter-list search tree, fixed, checkerboard and the_end biome sources, biome zoom (fuzzy offset), biome lookup at a block position.
- **Done when:** The biome grid matches vanilla across a range of seeds, and `/locate biome` gives the same results.

### 44. Surface (material) rules
- **Goal:** Surface block layers.
- **Upstream reference:** `data/worldgen/material_rule`, `data/worldgen/material_condition`
- **Deliverables:** Every rule and condition type, surface depth and secondary noise, stone depth, preliminary surface level.
- **Done when:** Chunk blocks after the surface step match vanilla.

### 45. Carvers
- **Goal:** Caves and canyons from carvers.
- **Upstream reference:** `nm/world/level/levelgen/carver`, `data/worldgen/carver`
- **Deliverables:** Cave, nether-cave and canyon carvers, carving masks, aquifer interaction.
- **Done when:** Carved chunks match vanilla.

### 46. Features and placement
- **Goal:** Decoration: trees, ores, vegetation and the rest.
- **Upstream reference:** `nm/world/level/levelgen/feature`, `nm/world/level/levelgen/placement`, `data/worldgen/feature`, `data/worldgen/placed_feature`, `data/worldgen/block_state_provider`
- **Deliverables:** Placement modifiers, the decoration step order and per-biome feature indexing, and the decoration seed for each feature. Feature types are delivered in sub-phases:
  - **46a:** ores and simple blocks.
  - **46b:** trees, foliage and trunk placers, decorators.
  - **46c:** vegetation and patches.
  - **46d:** lakes, springs, geodes, dripstone, sculk, and the rest.
- **Done when:** Fully decorated chunks match vanilla for a set of seeds.

### 47. Structures, jigsaw and templates
- **Goal:** The data-driven structure system.
- **Upstream reference:** `nm/world/level/levelgen/structure`, `structure/pools`, `structure/templatesystem`, `data/worldgen/structure`, `data/worldgen/structure_set`, `data/worldgen/template_pool`, `data/worldgen/processor_list`, `data/structure`
- **Deliverables:** Structure sets and placement (random spread, concentric rings, exclusion zones), structure starts and references, jigsaw assembly, NBT structure templates, processors, terrain adaptation (beard), `/locate structure`.
- **Done when:** Villages, ancient cities, trial chambers and bastions match vanilla in placement and contents.

### 48. Hard-coded structures
- **Goal:** Structures whose pieces are generated by code.
- **Upstream reference:** `nm/world/level/levelgen/structure/structures`
- **Deliverables:** Stronghold, mineshaft, nether fortress, ocean monument, woodland mansion, end city, igloo, swamp hut, desert and jungle temples, shipwreck, ocean ruins, buried treasure, ruined portal, nether fossil.
- **Done when:** The structure pieces and their contents match vanilla for a set of seeds.

### 49. Nether, End and world presets
- **Goal:** The remaining dimensions and presets.
- **Upstream reference:** `nm/world/level/dimension/end`, `nm/world/level/levelgen/flat`, `nm/world/level/levelgen/presets`, `data/worldgen/world_preset`, `data/worldgen/flat_level_generator_preset`, `data/dimension_type`
- **Deliverables:** Nether and End generation, end islands, end spikes, gateways, the dragon fight state, the flat, debug, single-biome, large-biomes and amplified presets, `level-type` and `generator-settings`.
- **Done when:** Every preset generates chunks identical to vanilla.

### 50. Worldgen parity certification (M2)
- **Goal:** Prove that worldgen matches vanilla.
- **Upstream reference:** Step 4 harness
- **Deliverables:** An automated diff of an N×N chunk area for K seeds in all three dimensions. Spawn-point selection (`NoiseSpawnFinder` behaviour). Generation-speed benchmarks.
- **Done when:** **M2:** The diff shows zero block differences on every matrix architecture, in both 1-thread and N-thread modes, and a client walks around vanilla-identical terrain.

---

## Phase 6: Game logic core

### 51. Tick loop and ordering
- **Goal:** Vanilla's tick sequence.
- **Upstream reference:** `nm/server` (server tick), `nm/server/level` (level tick), `nm/world/clock`, `nm/world/timeline`, `nm/world/level/timers`
- **Deliverables:**
  - The 20 TPS scheduler with catch-up behaviour.
  - The exact order of work within a tick: time, weather, world border, block ticks, fluid ticks, raids, chunk ticks, entities, block entities and so on.
  - The `/tick` command (rate, freeze, step, sprint).
  - Day/night, world clocks, timelines.
- **Done when:** The order of observable events matches vanilla in instrumented scenarios.

### 52. Scheduled and random ticks
- **Goal:** Block and fluid tick scheduling.
- **Upstream reference:** `nm/world/ticks`
- **Deliverables:** Level tick containers, tick priorities, per-chunk tick storage, random ticks (`randomTickSpeed`), precipitation ticks, ice and snow formation.
- **Done when:** Tick-order tests and random-tick distribution tests (seeded) match vanilla.

### 53. Block behaviour I: interaction basics
- **Goal:** Place, break and update blocks.
- **Upstream reference:** `nm/world/level/block`
- **Deliverables:** Placement context and rules, state updates for shape and neighbours with update flags, block breaking (mining speed, the right tool, drops through loot), use-on-block interactions, building blocks (stairs, slabs, doors, fences, walls, panes…).
- **Done when:** A placement and update scenario suite matches vanilla world state.

### 54. Block behaviour II: dynamic blocks
- **Goal:** Blocks that change on their own.
- **Upstream reference:** `nm/world/level/block`
- **Deliverables:** Crops, saplings and growth, bonemeal, farmland, leaves decay, fire spread, gravity blocks, snow and ice, coral, sculk, dripstone, vines, kelp and similar plants, sculk-fed and copper aging (oxidation, waxing).
- **Done when:** Seeded random-tick scenarios match vanilla.

### 55. Redstone
- **Goal:** Exact redstone, including its quirks.
- **Upstream reference:** `nm/world/level/redstone`, `nm/world/level/block/piston`, `data/datapacks/redstone_experiments`
- **Deliverables:** Wire power and its update order (default and experimental), torches with burnout, repeaters, comparators, observers, pistons (quasi-connectivity, push limit, block dropping), rails and powered rails, hoppers, droppers and dispensers, note blocks, target blocks, sculk sensors, crafters.
- **Done when:** A redstone contraption test bank (clocks, 0-tick behaviour, flying machines, update-order-sensitive builds) behaves identically.

### 56. Fluids
- **Goal:** Water and lava.
- **Upstream reference:** `nm/world/level/material`
- **Deliverables:** Flow, spread and source creation, waterlogging, fluid-block interactions (cobblestone, stone, obsidian, basalt), bubble columns, the dimension-dependent lava speed.
- **Done when:** Fluid spread scenarios match vanilla tick-for-tick.

### 57. Block entities
- **Goal:** Every block entity.
- **Upstream reference:** `nm/world/level/block/entity`
- **Deliverables:** Chests and barrels, furnaces, hoppers, brewing stands, beacons, spawners, trial spawners and vaults, crafters, signs (editing and waxing), banners, skulls, lecterns, jukeboxes, campfires, beehives, decorated pots, chiseled bookshelves, conduits, end gateways, structure blocks, command blocks, test blocks. Ticking, NBT and network sync for each.
- **Done when:** The per-type behaviour and NBT round-trip suites pass.

### 58. World systems
- **Goal:** Global world mechanics.
- **Upstream reference:** `nm/world/level` (explosions), `nm/world/level/border`, `nm/world/level/gameevent`, `nm/world/level/gamerules`
- **Deliverables:** Explosions (ray sampling, block damage, knockback, drops), weather cycles, lightning, the world border, game events and vibrations, every gamerule with its side effects.
- **Done when:** Explosion and vibration scenarios match vanilla, and every gamerule is honoured.

---

## Phase 7: Entities

### 59. Entity core
- **Goal:** The entity framework.
- **Upstream reference:** `nm/world/entity`, `nm/world/level/entity`
- **Deliverables:**
  - An ADR on the entity data layout, choosing between an ECS-style store and vanilla-style objects under the RAM budget.
  - Entity IDs and UUIDs, entity sections, the `entities/` storage backend, passengers and vehicles, persistence, removal reasons.
- **Done when:** Vanilla entity files round-trip, and the entity memory budget is met.

### 60. Movement and collision physics
- **Goal:** Bit-exact physics.
- **Upstream reference:** `nm/world/entity`, `nm/world/phys/shapes`
- **Deliverables:** Movement with collision against VoxelShapes, step-up, gravity and drag constants for each entity type, fluid pushing, block effects (soul sand, honey, cobwebs, powder snow, slime), fall damage.
- **Done when:** Scripted motion traces match vanilla positions exactly over N ticks.

### 61. Player entity
- **Goal:** The server-side player.
- **Upstream reference:** `nm/server/level` (server player), `nm/world/entity/player`, `nm/world/food`
- **Deliverables:** Movement validation ("moved too quickly" and "moved wrongly"), abilities and gamemodes, hunger, saturation and exhaustion, XP, health and regeneration, death screen, respawn, spectator mode, player data persistence.
- **Done when:** A survival-loop scenario (sprint, jump, starve, regenerate, die, respawn) matches vanilla.

### 62. Combat, damage and effects
- **Goal:** The damage pipeline.
- **Upstream reference:** `nm/world/damagesource`, `data/damage_type`, `nm/world/entity/ai/attributes`, `nm/world/attribute`, `nm/world/effect`
- **Deliverables:** Damage types and scaling, armour and toughness, enchantment protection, invulnerability frames, attack cooldown, critical hits, sweeping, knockback, shields, attributes and modifiers, status effects (including hidden effects).
- **Done when:** A damage calculation corpus matches vanilla values.

### 63. Item use behaviours
- **Goal:** What items do.
- **Upstream reference:** `nm/world/item`, `nm/world/item/component`
- **Deliverables:** Use and consume mechanics, tools and durability, bows, crossbows, tridents, maces and spears, buckets, food, potions, fireworks, elytra, totems, ender pearls, brushes, bundles, goat horns, and component-driven behaviours (consumable, equippable, tool, weapon, blocks_attacks…).
- **Done when:** A per-item behaviour suite passes.

### 64. AI framework and pathfinding
- **Goal:** The AI infrastructure for mobs.
- **Upstream reference:** `nm/world/entity/ai` (goal, behavior, memory, sensing, navigation, control, targeting), `nm/world/level/pathfinder`
- **Deliverables:**
  - Goal selectors and target selectors.
  - The brain system: memories, sensors, activities, behaviours.
  - Move, look and jump controls.
  - Node evaluators (walk, swim, fly, amphibious) and the path cache.
- **Done when:** Seeded pathfinding and goal-selection traces match vanilla.

### 65. Mobs and spawning
- **Goal:** Every mob, and natural spawning.
- **Upstream reference:** `nm/world/entity/monster`, `nm/world/entity/animal`, `nm/world/entity/npc`, `nm/world/entity/npc/villager`, `nm/world/entity/boss`, `nm/world/entity/raid`, `nm/world/item/trading`, `data/villager_trade`, `data/trade_set`
- **Deliverables:**
  - Hostile, passive and neutral mobs, and mob variants.
  - Bosses (ender dragon, wither), the warden.
  - Villagers: professions, POI, gossip, trades, breeding.
  - Raids, patrols, phantoms, the wandering trader.
  - Natural spawning: mob caps and categories, spawn rules, despawning.
- **Done when:** Per-mob behaviour suites pass, and spawn statistics match vanilla across seeded runs.

### 66. Other entities
- **Goal:** Non-mob entities.
- **Upstream reference:** `nm/world/entity/projectile`, `nm/world/entity/item`, `nm/world/entity/vehicle`, `nm/world/entity/decoration`
- **Deliverables:** Projectiles (arrows, tridents, fireballs, snowballs, wind charges…), item entities with merging and despawn, XP orbs with merging, boats and minecarts (including the minecart_improvements pack), paintings, item frames, armour stands, leash knots, falling blocks, primed TNT, area effect clouds, display and interaction entities, markers.
- **Done when:** Behaviour suites pass, and motion traces match vanilla.

---

## Phase 8: Player systems

### 67. Menus and containers
- **Goal:** Inventory interaction.
- **Upstream reference:** `nm/world/inventory`
- **Deliverables:** Menus with state IDs, every click type (pickup, quick-move, swap, clone, throw, quick-craft drag, pickup-all), slot validation, every menu type (player, chest variants, crafting, furnace variants, anvil, enchanting, smithing, grindstone, stonecutter, loom, cartography, brewing, beacon, merchant, hopper, crafter, horse and other mounts), resyncing on desync.
- **Done when:** A click-sequence corpus produces identical inventories and packets.

### 68. Recipes and recipe book (M3)
- **Goal:** Crafting.
- **Upstream reference:** `nm/world/item/crafting`, `nm/recipebook`, `data/recipe`
- **Deliverables:** Shaped, shapeless, special, smelting, stonecutting, smithing and transmute recipes. Matching against the crafting grid. Recipe unlocking and the recipe book (display packets, place-recipe ghost items).
- **Done when:** **M3:** Every vanilla recipe crafts identically, and a full survival loop is playable.

### 69. Enchanting, brewing and anvils
- **Goal:** Item progression systems.
- **Upstream reference:** `nm/world/item/enchantment`, `data/enchantment`, `data/enchantment_provider`, `nm/world/item/alchemy`
- **Deliverables:** Data-driven enchantment effects (all effect components), the enchanting table seed and offers, anvil cost and repair rules, the grindstone, brewing recipes and potion mixing.
- **Done when:** Seeded enchant offers and anvil costs match vanilla.

### 70. Command engine and commands
- **Goal:** The full command system.
- **Upstream reference:** `nm/commands`, `nm/server/commands`, `nm/server/permissions`, `commands.json`
- **Deliverables:**
  - A Rust command-graph parser (our own design, with Brigadier-compatible behaviour).
  - Every argument type, suggestions, and the command tree packet.
  - Entity selectors.
  - Every vanilla command, including `/execute` with all its subcommands.
  - Permission levels, command feedback messages.
- **Done when:** The command tree packet is identical to vanilla's, and the command output corpus matches.

### 71. Function runtime
- **Goal:** Datapack execution.
- **Upstream reference:** `nm/commands` (execution and functions)
- **Deliverables:** `.mcfunction` execution with the vanilla execution-context stack, macros, `/function`, `/return`, `/schedule`, `#minecraft:tick` and `#minecraft:load`, command storage, `maxCommandChainLength` and other limits.
- **Done when:** A corpus of popular-style datapack tests produces identical results.

### 72. Scoreboard, teams, advancements and stats
- **Goal:** Player progression and display systems.
- **Upstream reference:** `nm/world/scores`, `nm/server/bossevents`, `nm/advancements`, `data/advancement`, `nm/stats`
- **Deliverables:** Objectives and criteria, display slots, number formats, teams, bossbars, titles and action bar, tab list header and footer, advancements (every criterion trigger, rewards, toasts), statistics, waypoints and the locator bar.
- **Done when:** Advancement-granting scenarios and stats files match vanilla.

### 73. Dimensions, travel and spawn
- **Goal:** Movement between dimensions, and respawning.
- **Upstream reference:** `nm/world/level/portal`, `nm/world/level/block` (beds, anchors), `nm/world/level/saveddata/maps`
- **Deliverables:** Nether portal creation and linking (the POI search), End portals, end gateways, dimension change packets, beds and respawn anchors, spawn-point logic, maps (rendering and decorations), lodestones and compasses.
- **Done when:** Portal linking and map pixel data match vanilla.

### 74. Player management
- **Goal:** Server-side player administration.
- **Upstream reference:** `nm/server/players`
- **Deliverables:** `ops.json`, `whitelist.json`, `banned-players.json`, `banned-ips.json`, `usercache.json`, the matching commands, `max-players`, `enforce-whitelist`, `op-permission-level`, `player-idle-timeout`, `hide-online-players`.
- **Done when:** The JSON files are interchangeable with vanilla, and the enforcement behaviour matches.

---

## Phase 9: Operations & certification

### 75. Remote interfaces
- **Goal:** Operator tooling.
- **Upstream reference:** `nm/server/rcon`, `nm/server/jsonrpc`, `nm/server/notifications`, `nm/server/dedicated`
- **Deliverables:** RCON, the Query protocol (GameSpy4), the management server (JSON-RPC methods and notifications, authentication, TLS), status heartbeat. The GUI is out of scope: CeriumMC is headless, and the decision is documented.
- **Done when:** Standard RCON, Query and management clients work identically against both servers.

### 76. Parity certification (M4)
- **Goal:** Declare 1.0 "Vanilla Parity".
- **Upstream reference:** Step 4 harness, everything above
- **Deliverables:** The full differential suite passing, a manual playthrough checklist (survival to beating the dragon, redstone builds, a farm-heavy world, datapack-heavy world), long-run stability tests (24 hours, 30 bots), performance budgets checked against our targets on every hardware profile from step 5 (multi-core, single-core, low-RAM, slow-disk, ARM and Raspberry Pi class), release binaries for every matrix target, release notes.
- **Done when:** **M4:** Every item on the checklist is green, and 1.0 is tagged.

### 77. Version-update playbook
- **Goal:** Keep pace with new Minecraft releases.
- **Upstream reference:** The next release's `.cached` jar
- **Deliverables:** A documented process: drop in the new jar, regenerate reports, auto-diff the registries, packets, data and protocol, produce a work list from the diffs, re-run the parity suite.
- **Done when:** The playbook has been rehearsed against a snapshot.

---

## Milestones

| Milestone | Reached after | What you can do |
| --- | --- | --- |
| **M1: Hello, world** | Step 22 | A vanilla client joins a void world |
| **M2: Explorer** | Step 50 | Walk around terrain identical to vanilla in all dimensions (creative-like, minimal gameplay) |
| **M3: Survivor** | Step 68 | A full survival loop: mine, craft, fight, eat, die, respawn |
| **M4: Parity 1.0** | Step 76 | Certified 1:1 with vanilla 26.3 |

## After parity: CeriumMC features (preview)

These will be planned in detail in a separate roadmap once parity work is well underway. The hooks for them are reserved in the steps noted.

- **Compact world format:** A new backend for the `WorldStorage` trait (step 33). Ideas include palette-aware columnar encoding, cross-chunk dictionary compression, deduplicated sections and zstd, with lossless conversion to and from Anvil. Target: 80–90% smaller than `.mca`.
- **Plugin API:** Built on the event/hook seam from step 8. Covers the API surface, event model, versioning and stability guarantees, the sandboxing and loading model, and performance contracts.
- **Performance features:** Tuning to hit the ≤ 1 GB RAM target for 20–30 players, faster boot (cached registry snapshots), and smarter chunk I/O. Any change that would alter behaviour stays opt-in, so the default remains vanilla-identical.
- **More features:** To be discussed.
