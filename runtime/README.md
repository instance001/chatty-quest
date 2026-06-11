# Runtime

This folder holds user and session state for Chatty Quest.

Expected long-term roles:

- `config/` for local app settings
- `saves/` for JSON save files
- `logs/` for play and diagnostics logs
- `cache/` for temporary runtime artifacts
- `exports/` for explicit exported files

`v0.1` actively needs save files and may optionally use config/log support.
