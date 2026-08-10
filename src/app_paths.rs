use std::path::{Path, PathBuf};

const APP_ROOT_ENV: &str = "CHATTY_QUEST_BASE_PATH";

pub const APP_LAYOUT_DIRS: &[&str] = &[
    "assets",
    "assets/datapacks",
    "assets/ui",
    "assets/ui/branding",
    "assets/ui/screenshots",
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
];

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
    ensure_app_layout_at(&root)
}

pub fn ensure_app_layout_at(root: &Path) -> std::io::Result<()> {
    for relative in APP_LAYOUT_DIRS {
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

#[cfg(test)]
mod tests {
    use super::{APP_LAYOUT_DIRS, ensure_app_layout_at};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ensure_app_layout_bootstraps_binary_first_run_layout() {
        let root = std::env::temp_dir().join(format!(
            "chatty_quest_first_run_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));

        ensure_app_layout_at(&root).expect("expected app layout creation to succeed");

        for relative in APP_LAYOUT_DIRS {
            assert!(
                root.join(relative).is_dir(),
                "missing first-run directory: {relative}"
            );
        }

        std::fs::remove_dir_all(root).expect("expected temp app layout cleanup to succeed");
    }
}
