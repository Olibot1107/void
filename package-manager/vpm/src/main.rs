use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use reqwest::blocking::multipart::{Form as MultipartForm, Part};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tar::Archive;

const DEFAULT_REGISTRY: &str = "http://127.0.0.1:4090";

#[derive(Parser)]
#[command(name = "vpm", about = "Void Package Manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        name: Option<String>,
    },
    Publish {
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        github: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    Search {
        query: String,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    Install {
        name: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    NpmImport {
        package: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long = "as")]
        alias: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct PackageManifest {
    name: String,
    version: String,
    description: Option<String>,
    author: Option<String>,
    tarball_url: Option<String>,
    github_repo: Option<String>,
}

#[derive(Debug, Serialize)]
struct PublishPayload {
    name: String,
    version: String,
    description: Option<String>,
    tarball_url: Option<String>,
    github_repo: Option<String>,
    readme: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PackageVersion {
    name: String,
    version: String,
    description: String,
    author: String,
    tarball_url: String,
    github_repo: String,
    readme: String,
    created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct PackageSummary {
    name: String,
    version: String,
    description: String,
    author: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    ok: bool,
    message: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct LockFile {
    packages: HashMap<String, LockPackage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LockPackage {
    version: String,
    registry: String,
    tarball_url: String,
    github_repo: String,
}

#[derive(Debug, Deserialize)]
struct NpmPackageRoot {
    description: Option<String>,
    repository: Option<serde_json::Value>,
    author: Option<serde_json::Value>,
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, serde_json::Value>,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init { name } => cmd_init(name),
        Commands::Publish {
            registry,
            token,
            github,
            file,
        } => cmd_publish(&registry, token.as_deref(), github.as_deref(), file.as_deref()),
        Commands::Search { query, registry } => cmd_search(&registry, &query),
        Commands::Install {
            name,
            version,
            registry,
        } => cmd_install(&registry, &name, version.as_deref()),
        Commands::NpmImport {
            package,
            version,
            alias,
        } => cmd_npm_import(&package, version.as_deref(), alias.as_deref()),
    };

    if let Err(err) = result {
        eprintln!("vpm error: {err}");
        std::process::exit(1);
    }
}

fn cmd_init(name: Option<String>) -> Result<(), String> {
    let manifest_path = PathBuf::from("voidpkg.toml");
    if manifest_path.exists() {
        return Err("voidpkg.toml already exists".to_string());
    }

    let fallback_name = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("void-package")
        .to_string();

    let package_name = name.unwrap_or(fallback_name);
    validate_package_name(&package_name)?;

    let content = format!(
        "name = \"{}\"\nversion = \"0.1.0\"\ndescription = \"\"\nauthor = \"\"\ntarball_url = \"\"\ngithub_repo = \"\"\n",
        package_name
    );

    fs::write(&manifest_path, content).map_err(|e| e.to_string())?;
    println!("Created {}", manifest_path.display());
    Ok(())
}

fn cmd_publish(
    registry: &str,
    token: Option<&str>,
    github_override: Option<&str>,
    file: Option<&Path>,
) -> Result<(), String> {
    let manifest = load_manifest()?;
    validate_package_name(&manifest.name)?;
    if manifest.version.trim().is_empty() {
        return Err("Manifest version cannot be empty".to_string());
    }

    let readme = fs::read_to_string("README.md").ok();
    let github_repo = github_override
        .map(|s| s.trim().to_string())
        .or_else(|| manifest.github_repo.as_ref().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty());

    let payload = PublishPayload {
        name: manifest.name,
        version: manifest.version,
        description: manifest.description,
        tarball_url: manifest.tarball_url,
        github_repo,
        readme,
    };

    let client = Client::new();
    let api = if let Some(path) = file {
        publish_multipart(&client, registry, token, &payload, path)?
    } else {
        publish_json(&client, registry, token, &payload)?
    };

    if !api.ok {
        return Err(api.message);
    }

    println!("Published {}@{}", payload.name, payload.version);
    Ok(())
}

fn publish_json(
    client: &Client,
    registry: &str,
    token: Option<&str>,
    payload: &PublishPayload,
) -> Result<ApiMessage, String> {
    let url = format!("{}/api/publish", normalize_registry(registry));
    let mut request = client.post(url).json(payload);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }

    let response = request.send().map_err(|e| e.to_string())?;
    let status = response.status();
    let api: ApiMessage = response.json().map_err(|e| e.to_string())?;

    if !status.is_success() {
        return Ok(api);
    }

    Ok(api)
}

fn publish_multipart(
    client: &Client,
    registry: &str,
    token: Option<&str>,
    payload: &PublishPayload,
    file: &Path,
) -> Result<ApiMessage, String> {
    let url = format!("{}/api/publish/upload", normalize_registry(registry));
    let file_name = file
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?
        .to_string();
    let file_bytes = fs::read(file).map_err(|e| format!("Could not read file: {e}"))?;

    let mut form = MultipartForm::new()
        .text("name", payload.name.clone())
        .text("version", payload.version.clone())
        .text("description", payload.description.clone().unwrap_or_default())
        .text("tarball_url", payload.tarball_url.clone().unwrap_or_default())
        .text("github_repo", payload.github_repo.clone().unwrap_or_default())
        .text("readme", payload.readme.clone().unwrap_or_default());

    let file_part = Part::bytes(file_bytes).file_name(file_name);
    form = form.part("package_file", file_part);

    let mut request = client.post(url).multipart(form);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }

    let response = request.send().map_err(|e| e.to_string())?;
    let status = response.status();
    let api: ApiMessage = response.json().map_err(|e| e.to_string())?;

    if !status.is_success() {
        return Ok(api);
    }

    Ok(api)
}

fn cmd_search(registry: &str, query: &str) -> Result<(), String> {
    let client = Client::new();
    let url = format!("{}/api/search", normalize_registry(registry));

    let response = client
        .get(url)
        .query(&[("q", query)])
        .send()
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Registry returned status {}", response.status()));
    }

    let packages: Vec<PackageSummary> = response.json().map_err(|e| e.to_string())?;

    if packages.is_empty() {
        println!("No packages found for '{query}'");
        return Ok(());
    }

    for pkg in packages {
        println!(
            "{}@{} - {} (author: {})",
            pkg.name, pkg.version, pkg.description, pkg.author
        );
    }

    Ok(())
}

fn cmd_install(registry: &str, name: &str, version: Option<&str>) -> Result<(), String> {
    validate_package_name(name)?;

    let client = Client::new();
    let url = format!("{}/api/packages/{}", normalize_registry(registry), name);

    let response = client.get(url).send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Registry returned status {}", response.status()));
    }

    let versions: Vec<PackageVersion> = response.json().map_err(|e| e.to_string())?;
    if versions.is_empty() {
        return Err(format!("Package '{name}' not found"));
    }

    let selected = select_version(&versions, version)?;

    let module_dir = PathBuf::from("void_modules").join(&selected.name);
    fs::create_dir_all(&module_dir).map_err(|e| e.to_string())?;

    let metadata_path = module_dir.join("package.json");
    let metadata = serde_json::to_string_pretty(&selected).map_err(|e| e.to_string())?;
    fs::write(&metadata_path, metadata).map_err(|e| e.to_string())?;

    if !selected.github_repo.trim().is_empty() {
        install_from_github(&module_dir, &selected.github_repo)?;
    }

    if !selected.tarball_url.trim().is_empty() {
        let download_url = absolute_url_from_registry(registry, &selected.tarball_url);
        if let Ok(resp) = client.get(&download_url).send()
            && resp.status().is_success()
            && let Ok(bytes) = resp.bytes()
        {
            let tarball_path = module_dir.join("package.tgz");
            let _ = fs::write(&tarball_path, &bytes);
        }
    }

    update_lockfile(
        name,
        &selected.version,
        registry,
        &selected.tarball_url,
        &selected.github_repo,
    )?;

    println!("Installed {}@{}", selected.name, selected.version);
    println!("Saved metadata at {}", metadata_path.display());
    Ok(())
}

fn cmd_npm_import(package: &str, version: Option<&str>, alias: Option<&str>) -> Result<(), String> {
    if package.trim().is_empty() {
        return Err("Package name cannot be empty".to_string());
    }

    let client = Client::new();
    let encoded_name = encode_npm_name(package);
    let metadata_url = format!("https://registry.npmjs.org/{encoded_name}");
    let response = client.get(&metadata_url).send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "npm registry returned status {} for package '{}'",
            response.status(),
            package
        ));
    }

    let root: NpmPackageRoot = response.json().map_err(|e| e.to_string())?;
    let selected_version = match version {
        Some(v) => v.to_string(),
        None => root
            .dist_tags
            .get("latest")
            .cloned()
            .ok_or_else(|| format!("npm package '{package}' does not have a latest dist-tag"))?,
    };

    let version_meta = root
        .versions
        .get(&selected_version)
        .ok_or_else(|| format!("Version '{selected_version}' not found for npm package '{package}'"))?;

    let tarball_url = version_meta
        .get("dist")
        .and_then(|v| v.get("tarball"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("npm package '{package}@{selected_version}' has no dist.tarball"))?
        .to_string();

    let main_js = version_meta
        .get("main")
        .and_then(|v| v.as_str())
        .unwrap_or("index.js");
    let description = version_meta
        .get("description")
        .and_then(|v| v.as_str())
        .or(root.description.as_deref())
        .unwrap_or("npm import")
        .to_string();

    let repository = version_meta
        .get("repository")
        .and_then(extract_repository_url)
        .or_else(|| root.repository.as_ref().and_then(extract_repository_url))
        .unwrap_or_default();
    let author = version_meta
        .get("author")
        .and_then(extract_author_name)
        .or_else(|| root.author.as_ref().and_then(extract_author_name))
        .unwrap_or_else(|| "npm".to_string());

    let void_name = alias
        .map(|value| value.to_string())
        .unwrap_or_else(|| npm_name_to_void_name(package));
    validate_package_name(&void_name)?;

    let module_dir = PathBuf::from("void_modules").join(&void_name);
    if module_dir.exists() {
        fs::remove_dir_all(&module_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&module_dir).map_err(|e| e.to_string())?;

    let tarball_bytes = client
        .get(&tarball_url)
        .send()
        .map_err(|e| format!("Failed to download npm tarball: {e}"))?
        .bytes()
        .map_err(|e| format!("Failed to read npm tarball bytes: {e}"))?;
    let npm_dir = module_dir.join("npm");
    extract_npm_tarball(&tarball_bytes, &npm_dir)?;
    let npm_package_dir = npm_dir.join("package");
    match try_install_npm_dependencies(&npm_package_dir) {
        Ok(true) => println!("Installed npm dependencies inside {}", npm_package_dir.display()),
        Ok(false) => println!("Skipped npm dependency install (npm CLI not found)"),
        Err(err) => println!("Warning: npm dependency install failed: {err}"),
    }

    let wrapper = npm_wrapper_script(package, &selected_version, main_js);
    fs::write(module_dir.join("index.void"), wrapper).map_err(|e| e.to_string())?;

    let void_manifest = serde_json::json!({
        "name": void_name,
        "main": "index.void",
        "source": "npm",
        "npm_name": package,
        "npm_version": selected_version,
    });
    let manifest_content =
        serde_json::to_string_pretty(&void_manifest).map_err(|e| e.to_string())?;
    fs::write(module_dir.join("void.json"), manifest_content).map_err(|e| e.to_string())?;

    let package_metadata = serde_json::json!({
        "name": void_name,
        "version": selected_version,
        "description": description,
        "author": author,
        "tarball_url": tarball_url,
        "github_repo": repository,
        "npm_name": package,
        "main": "index.void"
    });
    let metadata_content =
        serde_json::to_string_pretty(&package_metadata).map_err(|e| e.to_string())?;
    fs::write(module_dir.join("package.json"), metadata_content).map_err(|e| e.to_string())?;

    let publish_manifest = format!(
        "name = \"{}\"\nversion = \"{}\"\ndescription = \"{}\"\nauthor = \"{}\"\ntarball_url = \"{}\"\ngithub_repo = \"{}\"\n",
        escape_toml_string(&void_name),
        escape_toml_string(&selected_version),
        escape_toml_string(&format!("npm import: {package} - {description}")),
        escape_toml_string(&author),
        escape_toml_string(&tarball_url),
        escape_toml_string(&repository),
    );
    fs::write(module_dir.join("voidpkg.toml"), publish_manifest).map_err(|e| e.to_string())?;

    let source_note = format!(
        "This package was converted from npm.\n\nnpm: {}@{}\nentry: {}\n\nUse in Void:\n  use \"{}\" as pkg\n",
        package, selected_version, main_js, void_name
    );
    fs::write(module_dir.join("NPM_IMPORT.txt"), source_note).map_err(|e| e.to_string())?;

    update_lockfile(
        &void_name,
        &selected_version,
        "https://registry.npmjs.org",
        &tarball_url,
        &repository,
    )?;

    println!("Imported npm package {package}@{selected_version}");
    println!("Converted to Void package: {void_name}");
    println!("Installed at {}", module_dir.display());
    println!("Import from Void with: use \"{void_name}\" as pkg");
    Ok(())
}

fn install_from_github(module_dir: &Path, github_repo: &str) -> Result<(), String> {
    let repo_dir = module_dir.join("repo");
    if repo_dir.exists() {
        fs::remove_dir_all(&repo_dir).map_err(|e| e.to_string())?;
    }

    let result = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(github_repo)
        .arg(&repo_dir)
        .status();

    match result {
        Ok(status) if status.success() => Ok(()),
        _ => {
            let fallback = module_dir.join("SOURCE.txt");
            fs::write(&fallback, format!("GitHub source: {github_repo}\n"))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
    }
}

fn select_version<'a>(versions: &'a [PackageVersion], desired: Option<&str>) -> Result<&'a PackageVersion, String> {
    if let Some(target) = desired {
        return versions
            .iter()
            .find(|pkg| pkg.version == target)
            .ok_or_else(|| format!("Version '{target}' not found"));
    }

    versions
        .first()
        .ok_or_else(|| "No versions available".to_string())
}

fn update_lockfile(
    name: &str,
    version: &str,
    registry: &str,
    tarball_url: &str,
    github_repo: &str,
) -> Result<(), String> {
    let lock_path = PathBuf::from("void.lock");

    let mut lock = if lock_path.exists() {
        let data = fs::read_to_string(&lock_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<LockFile>(&data).unwrap_or_default()
    } else {
        LockFile::default()
    };

    lock.packages.insert(
        name.to_string(),
        LockPackage {
            version: version.to_string(),
            registry: normalize_registry(registry).to_string(),
            tarball_url: tarball_url.to_string(),
            github_repo: github_repo.to_string(),
        },
    );

    let content = serde_json::to_string_pretty(&lock).map_err(|e| e.to_string())?;
    fs::write(lock_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_manifest() -> Result<PackageManifest, String> {
    let raw = fs::read_to_string("voidpkg.toml")
        .map_err(|_| "voidpkg.toml not found. Run: vpm init".to_string())?;
    let manifest: PackageManifest = toml::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(manifest)
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Package name cannot be empty".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Package name may only contain letters, numbers, '-' and '_'".to_string());
    }
    Ok(())
}

fn encode_npm_name(name: &str) -> String {
    name.replace('/', "%2f")
}

fn npm_name_to_void_name(name: &str) -> String {
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

fn try_install_npm_dependencies(package_dir: &Path) -> Result<bool, String> {
    if !package_dir.join("package.json").exists() {
        return Ok(false);
    }

    let status = Command::new("npm")
        .arg("install")
        .arg("--omit=dev")
        .arg("--legacy-peer-deps")
        .arg("--no-audit")
        .arg("--no-fund")
        .current_dir(package_dir)
        .status();

    match status {
        Ok(result) if result.success() => Ok(true),
        Ok(result) => Err(format!("npm install exited with status {result}")),
        Err(_) => Ok(false),
    }
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_void_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn npm_wrapper_script(npm_name: &str, npm_version: &str, main_js: &str) -> String {
    let npm_name_escaped = escape_void_string(npm_name);
    let npm_version_escaped = escape_void_string(npm_version);
    let main_js_escaped = escape_void_string(main_js);

    format!(
        "module.exports.name = \"{npm_name_escaped}\"\n\
module.exports.version = \"{npm_version_escaped}\"\n\
module.exports.kind = \"npm_bridge\"\n\
module.exports.entry_js = \"npm/package/{main_js_escaped}\"\n\
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

fn extract_repository_url(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .get("url")
        .and_then(|url| url.as_str())
        .map(|url| url.to_string())
}

fn extract_author_name(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value
        .get("name")
        .and_then(|name| name.as_str())
        .map(|name| name.to_string())
}

fn normalize_registry(input: &str) -> &str {
    input.trim_end_matches('/')
}

fn absolute_url_from_registry(registry: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    format!("{}/{}", normalize_registry(registry), url.trim_start_matches('/'))
}
