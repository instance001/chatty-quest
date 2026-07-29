# Chatty Quest

![RD Engine Logo](assets/ui/branding/RD-Engine-logo.png)

![Chatty Quest Banner](assets/ui/branding/chatty-quest-logo.png)

Chatty Quest is the first game built on the `RD Engine`, the `Radiant Determinism Engine`.

The short version:

- templates are the nouns
- buckets are the current grammar
- reducers are the verbs
- narration is the accent
- media is the illustration

Chatty Quest is a Rust desktop adventure engine where deterministic templates and bucketed state create the world, while a narrator layer presents that world as a chat-forward Dungeon Master experience.

`Radiant Determinism` means the experience can feel dynamic, reactive, funny, and alive, while every meaningful gameplay payoff is grounded in deterministic state, reducer-confirmed mutations, and visible UI updates.

Current release status:

- `v0.1` is accepted
- the desktop `egui/eframe` shell is playable end-to-end
- datapack discovery, deterministic run generation, reducer actions, `MockNarrator`, media focus, diagnostics, and JSON save/load are all wired and working
- the current branch also includes `v0.2`-lane siege-depth work for barricades, noise pressure, route forecasting, guided command surfaces, and authored garage-finale payoffs
- `cargo test` currently passes with `67` automated tests
- `cargo clippy --all-targets --all-features -- -D warnings` passes
- the original `v0.1` live manual sweep passed on `2026-06-11`, and the current branch has refreshed sweep/audit notes dated `2026-07-28`

`v0.1` is focused on one playable scenario pack:

- `Property Siege Classic`

The goal is to prove:

- datapack-driven scenario loading
- deterministic run state
- reducer-owned game truth
- a replaceable narrator seam
- save/load reliability

## Storage Layout

Chatty Quest stores writable runtime data under the active app root. Source runs use the repository folder. Packaged executable runs use the executable's folder. Set `CHATTY_QUEST_BASE_PATH` to force a portable root for testing, scripts, or custom installs.

On first run the app creates the writable skeleton it needs, including `runtime/`, `runtime/saves/`, `runtime/config/`, `runtime/logs/`, `models/`, `datasets/`, and `handoff/`. Bundled gameplay content should ship beside the app root under `assets/datapacks/`.

## RD Engine Loop Map

```mermaid
flowchart TB
    datapack["Datapack templates<br/>locations, items, enemies, bosses, objectives"] --> runGen["Run generation<br/>scenario rules + starting buckets"]
    runGen --> state["Runtime state<br/>canonical game truth"]

    player["Player command<br/>chat-forward adventure input"] --> reducer["Reducer<br/>validates and mutates truth"]
    state --> reducer
    reducer --> rejected["Rejected action<br/>clear reason, no hidden mutation"]
    rejected --> ui["UI shell<br/>visible proof surfaces"]

    reducer --> result["Reducer result<br/>events, log lines, media focus, diagnostics data"]
    result --> state
    result --> narrator["Narrator seam<br/>MockNarrator now, future LLM later"]
    narrator --> prose["DM-style prose<br/>flavor over confirmed facts"]

    state --> derived["Derived views<br/>map, inventory, character, diagnostics, media panel"]
    derived --> ui
    prose --> ui
    ui --> player

    state --> save["JSON save/load<br/>structured state, not prose truth"]
    save --> state
```

## Screenshot Tour

Main menu and setup shell:

![Main menu screenshot](assets/ui/screenshots/Screenshot-main-menu.png)

Active run examples:

| Front Verandah | Laundry |
| --- | --- |
| ![Front Verandah screenshot](assets/ui/screenshots/Screenshot-front-verandah.png) | ![Laundry screenshot](assets/ui/screenshots/Screenshot-laundry.png) |

| Crawler encounter | Inventory tab |
| --- | --- |
| ![Crawler encounter screenshot](assets/ui/screenshots/Screenshot-crawler-in-weeds.png) | ![Inventory screenshot](assets/ui/screenshots/Screenshot-inventory.png) |

Release and verification docs:

- [docs/ZERO_KNOWLEDGE_USER_MANUAL.md](docs/ZERO_KNOWLEDGE_USER_MANUAL.md) - zero spoilers
- [docs/FULL_SPOILERS_USER_MANUAL.md](docs/FULL_SPOILERS_USER_MANUAL.md) - full spoilers
- [docs/V0_1_RELEASE_NOTES.md](docs/V0_1_RELEASE_NOTES.md)
- [docs/V0_1_ACCEPTANCE_AUDIT.md](docs/V0_1_ACCEPTANCE_AUDIT.md)
- [docs/V0_1_MANUAL_SWEEP.md](docs/V0_1_MANUAL_SWEEP.md)
- [docs/V0_2_MILESTONE_PLAN.md](docs/V0_2_MILESTONE_PLAN.md)
- [docs/V0_2_BARRICADE_SPEC.md](docs/V0_2_BARRICADE_SPEC.md)
- [docs/V0_2_NOISE_PRESSURE_SPEC.md](docs/V0_2_NOISE_PRESSURE_SPEC.md)

Media credit:

- in-game media for this release was created with help from [instance001/chatty-art](https://github.com/instance001/chatty-art)

Core project docs:

- [docs/DESIGN_INTENT.md](docs/DESIGN_INTENT.md)
- [docs/PROJECT_OVERVIEW.md](docs/PROJECT_OVERVIEW.md)
- [docs/RD_ENGINE_PRINCIPLES.md](docs/RD_ENGINE_PRINCIPLES.md)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/COMMAND_BOUNDARY_SPEC.md](docs/COMMAND_BOUNDARY_SPEC.md)
- [docs/NARRATOR_CONTEXT_SPEC.md](docs/NARRATOR_CONTEXT_SPEC.md)
- [docs/ROLLING_SUMMARY_SPEC.md](docs/ROLLING_SUMMARY_SPEC.md)
- [docs/IMPLEMENTATION_ROADMAP.md](docs/IMPLEMENTATION_ROADMAP.md)
- [docs/UI_SHELL_SPEC.md](docs/UI_SHELL_SPEC.md)

Branding art used by the app and docs lives under [`assets/ui/branding/`](assets/ui/branding/).

Captured app screenshots for docs and release use live under [`assets/ui/screenshots/`](assets/ui/screenshots/).

## Related Docs

- [GLOSSARY.md](GLOSSARY.md)

## License

Chatty Quest is licensed under the GNU Affero General Public License v3.0.

See [LICENSE](LICENSE) for details.
