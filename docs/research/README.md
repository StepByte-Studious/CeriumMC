# Research process

Every CeriumMC system starts as a research note. We work out how vanilla *behaves* and write it down in our own words. Code is then written from the note, not from vanilla's code. This is how we stay clean-room while matching vanilla 1:1.

Read the clean-room rules in [CONTRIBUTING.md](../../CONTRIBUTING.md) first.

## The upstream reference

The pinned vanilla version lives in [`vanilla.toml`](../../vanilla.toml). There are two copies of the vanilla jar:

| Copy | Where | Used for |
| --- | --- | --- |
| Research copy | `.cached/` (local only, git-ignored) | Reading data files, running the data generator, finding where a system lives |
| CI copy | `.vanilla/` (downloaded by `scripts/fetch-vanilla.sh`) | Automated reports and differential tests |

`.cached/` holds the extracted bundler. The game files are under `.cached/META-INF/versions/<version>/extracted-<version>jar/`:

- `data/minecraft/`: vanilla's data pack (worldgen, loot tables, recipes, tags, registries). This is our main source of facts.
- `assets/`: client assets. Rarely needed by a server.
- `net/minecraft/**`: compiled classes. Use package and class **names** only, to find where a system lives.

To check that your `.cached/` copy is the pinned version (works offline, needs only Git Bash):

```sh
bash scripts/fetch-vanilla.sh --check-cached
```

## Allowed sources

| Source | Allowed | Notes |
| --- | --- | --- |
| Data files in `data/minecraft/` | Yes | Primary source of data-driven facts |
| Data generator reports (`--reports`) | Yes | Blocks, registries, packets, commands (roadmap step 3) |
| Observing the running server and client | Yes | Packet captures, in-game tests, world files it writes |
| Class and package names | Yes | Only to locate a system |
| Community documentation (for example the Minecraft Wiki) | Yes, as a secondary source | Always confirm it against vanilla. It can be outdated |
| Decompiled method bodies | **No** | Never copy them, translate them or paraphrase them line by line |

## Writing a note

1. Copy [`_TEMPLATE.md`](_TEMPLATE.md) to `docs/research/<topic>.md`, using a lowercase, hyphenated topic name (for example `paletted-containers.md`).
2. Fill in every section. Use "None known" rather than deleting a section.
3. Describe *what* happens and *why it matters*, never *how vanilla's code does it*.
4. Record how each fact was verified, so anyone can repeat the measurement.
5. Add the note to [`INDEX.md`](INDEX.md) next to its roadmap step.
6. Set the status:
   - **Draft:** still being researched.
   - **Verified:** every claim has a measurement or a test against vanilla.
   - **Implemented:** the code is written and the differential tests pass.

A roadmap step's code may only be merged once its notes are **Verified**.
