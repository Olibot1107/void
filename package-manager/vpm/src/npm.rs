use std::ffi::OsStr;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use tar::Archive;

use crate::logging::{log_info, log_success};
use crate::{remove_dir_tree, NpmImportCacheResult};

pub(crate) fn encode_npm_name(name: &str) -> String {
    name.replace('/', "%2f")
}

fn sanitize_cache_segment(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }

    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "pkg".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn npm_import_cache_root() -> PathBuf {
    if let Ok(custom) = std::env::var("VPM_CACHE_DIR")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom).join("npm-import");
    }

    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME")
        && !xdg.trim().is_empty()
    {
        return PathBuf::from(xdg).join("vpm").join("npm-import");
    }

    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home)
            .join(".cache")
            .join("vpm")
            .join("npm-import");
    }

    PathBuf::from(".vpm-cache").join("npm-import")
}

fn npm_import_cache_dir(package: &str, version: &str) -> PathBuf {
    npm_import_cache_root()
        .join(sanitize_cache_segment(package))
        .join(sanitize_cache_segment(version))
}

pub(crate) fn read_existing_import_version(module_dir: &Path) -> Option<String> {
    let package_json = module_dir.join("package.json");
    let raw = fs::read_to_string(package_json).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn ensure_npm_import_cache(
    client: &Client,
    package: &str,
    version: &str,
    tarball_url: &str,
) -> Result<NpmImportCacheResult, String> {
    let cache_dir = npm_import_cache_dir(package, version);
    let cache_package_dir = cache_dir.join("npm").join("package");

    if cache_package_dir.exists() {
        if cache_has_legacy_void_units(&cache_package_dir)? {
            log_info("Detected legacy void-only npm cache; rebuilding npm import cache.");
            remove_dir_tree(&cache_dir)?;
        } else {
            let source_units = count_npm_source_units(&cache_package_dir)?;
            return Ok(NpmImportCacheResult {
                cache_dir,
                used_cache: true,
                source_units,
            });
        }
    }

    if cache_dir.exists() {
        remove_dir_tree(&cache_dir)?;
    }
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    let result = (|| -> Result<usize, String> {
        let tarball_bytes = client
            .get(tarball_url)
            .send()
            .map_err(|e| format!("Failed to download npm tarball: {e}"))?
            .bytes()
            .map_err(|e| format!("Failed to read npm tarball bytes: {e}"))?;

        let npm_dir = cache_dir.join("npm");
        extract_npm_tarball(&tarball_bytes, &npm_dir)?;
        let npm_package_dir = npm_dir.join("package");
        count_npm_source_units(&npm_package_dir)
    })();

    match result {
        Ok(source_units) => Ok(NpmImportCacheResult {
            cache_dir,
            used_cache: false,
            source_units,
        }),
        Err(err) => {
            let _ = remove_dir_tree(&cache_dir);
            Err(err)
        }
    }
}

fn cache_has_legacy_void_units(root: &Path) -> Result<bool, String> {
    let mut stack = vec![root.to_path_buf()];

    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if path.extension().and_then(OsStr::to_str) != Some("void") {
                continue;
            }

            let content = fs::read_to_string(&path).unwrap_or_default();
            if content.contains("module.exports.kind = \"npm_void_unit\"")
                || content.contains("Converted to Void-only package format")
            {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub(crate) fn npm_name_to_void_name(name: &str) -> String {
    let mut out = String::from("npm_");
    let mut last_was_sep = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }

    while out.ends_with('_') {
        out.pop();
    }

    if out == "npm" || out == "npm_" {
        "npm_pkg".to_string()
    } else {
        out
    }
}

fn extract_npm_tarball(bytes: &[u8], destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;

    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(decoder);
    archive.unpack(destination).map_err(|e| e.to_string())
}

fn count_npm_source_units(root: &Path) -> Result<usize, String> {
    let mut count = 0usize;
    let mut stack = vec![root.to_path_buf()];

    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if is_js_like_source(&path) {
                count += 1;
            }
        }
    }

    Ok(count)
}

pub(crate) fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        remove_dir_tree(destination)?;
    }
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;

    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dest_path = destination.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn is_js_like_source(path: &Path) -> bool {
    let file_name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
    if file_name.ends_with(".d.ts") {
        return true;
    }

    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("js" | "mjs" | "cjs" | "ts" | "mts" | "cts" | "jsx" | "tsx")
    )
}

pub(crate) fn install_npm_dependencies(package_dir: &Path) -> Result<(), String> {
    let npm_bin = if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    };
    let display_dir = package_dir.display().to_string();
    log_info(&format!(
        "Installing npm dependencies in {display_dir} (--with-npm-deps)"
    ));

    let status = Command::new(npm_bin)
        .arg("install")
        .arg("--omit=dev")
        .current_dir(package_dir)
        .status()
        .map_err(|e| format!("Failed to execute '{npm_bin} install --omit=dev': {e}"))?;

    if !status.success() {
        return Err(format!(
            "npm install failed in {} with status {}",
            package_dir.display(),
            status
        ));
    }

    log_success("Installed npm dependencies.");
    Ok(())
}

pub(crate) fn npm_main_to_js_path(main_js: &str) -> String {
    let trimmed = main_js.trim_start_matches("./");
    let selected = if trimmed.is_empty() {
        PathBuf::from("index.js")
    } else {
        PathBuf::from(trimmed)
    };

    let normalized = if selected.extension().is_none() {
        selected.with_extension("js")
    } else {
        selected
    };

    normalized.to_string_lossy().replace('\\', "/")
}

pub(crate) fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_void_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn npm_wrapper_script(npm_name: &str, npm_version: &str, main_js: &str) -> String {
    let npm_name_escaped = escape_void_string(npm_name);
    let npm_version_escaped = escape_void_string(npm_version);
    let main_js_escaped = escape_void_string(main_js);

    format!(
        "module.exports.name = \"{npm_name_escaped}\"\n\
module.exports.version = \"{npm_version_escaped}\"\n\
module.exports.kind = \"npm_bridge\"\n\
module.exports.entry_js = \"npm/package/{main_js_escaped}\"\n\
module.exports.runtime = \"node_bridge\"\n\
module.exports.warning = \"Auto-imported npm package. Uses Node.js bridge for JS runtime.\"\n\
\n\
module.exports.run_entry = fn () {{\n\
use \"cmd\" as cmd\n\
return cmd.run(\"node '\" + __dirname + \"/\" + module.exports.entry_js + \"'\")\n\
}}\n\
\n\
module.exports.run = fn (relative_js) {{\n\
use \"cmd\" as cmd\n\
return cmd.run(\"node '\" + __dirname + \"/npm/package/\" + relative_js + \"'\")\n\
}}\n"
    )
}

pub(crate) fn extract_repository_url(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .get("url")
        .and_then(|url| url.as_str())
        .map(|url| url.to_string())
}

pub(crate) fn extract_author_name(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .get("name")
        .and_then(|name| name.as_str())
        .map(|name| name.to_string())
}
