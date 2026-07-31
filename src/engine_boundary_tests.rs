use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_LIVE_ENGINE_TERMS: &[&str] = &[
    "property_siege_classic",
    "Property Siege Classic",
    "front_verandah",
    "Front Verandah",
    "back_garden",
    "Back Garden",
    "garage",
    "Garage",
    "brute_in_garage",
    "Garage Brute",
    "shambler_front_gate",
    "Front Gate Shambler",
    "crawler_in_weeds",
    "Crawler In The Weeds",
    "house_keys",
    "House Keys",
    "medkit",
    "Medkit",
    "barricade_kit",
    "Barricade Kit",
    "cricket_bat",
    "Battered Cricket Bat",
    "torch",
    "Torch",
];

#[test]
fn live_engine_source_does_not_name_property_siege_demo_content() {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for path in rust_files_under(&src_root) {
        if path
            .file_name()
            .is_some_and(|name| name == "engine_boundary_tests.rs")
        {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        let live_source = contents
            .split_once("#[cfg(test)]")
            .map(|(live_source, _)| live_source)
            .unwrap_or(contents.as_str());

        for (line_index, line) in live_source.lines().enumerate() {
            for term in FORBIDDEN_LIVE_ENGINE_TERMS {
                if line.contains(term) {
                    violations.push(format!(
                        "{}:{} contains forbidden demo term {:?}: {}",
                        path.strip_prefix(&src_root)
                            .unwrap_or(path.as_path())
                            .display(),
                        line_index + 1,
                        term,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "live engine source must stay scenario-pack agnostic:\n{}",
        violations.join("\n")
    );
}

fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
