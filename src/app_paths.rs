use std::path::{Path, PathBuf};

const APP_ROOT_ENV: &str = "CHATTY_QUEST_BASE_PATH";

pub fn app_root() -> PathBuf {
    if let Some(path) = std::env::var_os(APP_ROOT_ENV)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return path;
    }

    let mut candidates = Vec::new();
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(parent) = exe_path.parent()
    {
        candidates.push(parent.to_path_buf());
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir);
    }

    for candidate in &candidates {
        if let Some(root) = find_source_root(candidate) {
            return root;
        }
    }

    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn ensure_app_layout() -> std::io::Result<()> {
    let root = app_root();
    for relative in [
        "assets/datapacks",
        "runtime",
        "runtime/cache",
        "runtime/config",
        "runtime/exports",
        "runtime/logs",
        "runtime/saves",
        "models",
        "models/local",
        "models/adapters",
        "models/capsules",
        "datasets",
        "datasets/generated",
        "datasets/templates",
        "datasets/worldbuilding",
        "handoff",
        "handoff/chatty_cog/inbox",
        "handoff/chatty_cog/outbox",
        "handoff/chatty_art/requests",
        "handoff/chatty_art/outputs",
        "handoff/chatty_lora/style_refs",
        "handoff/chatty_lora/outputs",
    ] {
        std::fs::create_dir_all(root.join(relative))?;
    }
    Ok(())
}

pub fn path(relative: impl AsRef<Path>) -> PathBuf {
    app_root().join(relative)
}

pub fn display_path(relative: impl AsRef<Path>) -> String {
    path(relative).display().to_string()
}

pub fn datapacks_root() -> PathBuf {
    path("assets/datapacks")
}

pub fn current_save_path() -> PathBuf {
    path("runtime/saves/current_run.json")
}

fn find_source_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if is_source_root(ancestor) {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn is_source_root(path: &Path) -> bool {
    path.join("Cargo.toml").exists()
        && path.join("src").join("main.rs").exists()
        && path.join("assets").join("datapacks").exists()
}
