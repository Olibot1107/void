use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use clap::Parser;
use reqwest::blocking::multipart::{Form as MultipartForm, Part};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

mod cli;
mod logging;
mod npm;

use cli::Cli;
use cli::Commands;
use logging::log_error;
use logging::log_info;
use logging::log_success;
use logging::log_verbose;
use logging::log_warn;
use logging::muted;
use logging::paint;
use logging::set_cli_runtime_config;
use logging::Loader;
use npm::copy_dir_recursive;
use npm::encode_npm_name;
use npm::ensure_npm_import_cache;
use npm::escape_toml_string;
use npm::extract_author_name;
use npm::extract_repository_url;
use npm::install_npm_dependencies;
use npm::npm_main_to_js_path;
use npm::npm_import_cache_root;
use npm::npm_name_to_void_name;
use npm::npm_wrapper_script;
use npm::read_existing_import_version;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    npm_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PackageVersion {
    name: String,
    version: String,
    description: String,
    author: String,
    tarball_url: String,
    github_repo: String,
    readme: String,
    created_at: String,
    #[serde(default)]
    downloads: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct PackageSummary {
    name: String,
    version: String,
    description: String,
    author: String,
    created_at: String,
    #[serde(default)]
    downloads: i64,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    ok: bool,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ApiLoginResponse {
    ok: bool,
    message: String,
    token: Option<String>,
    username: Option<String>,
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

#[derive(Debug, Deserialize, Serialize, Default)]
struct AuthStore {
    sessions: HashMap<String, AuthSession>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AuthSession {
    token: String,
    username: String,
    saved_at: String,
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

struct NpmImportCacheResult {
    cache_dir: PathBuf,
    used_cache: bool,
    source_units: usize,
}

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn main() {
    let cli = Cli::parse();
    set_cli_runtime_config(cli.verbose, cli.color);
    if cli.verbose {
        log_verbose("verbose mode enabled");
    }

    let result = match cli.command {
        Some(Commands::Init { name }) => cmd_init(name),
        Some(Commands::Publish {
            registry,
            token,
            github,
            file,
        }) => cmd_publish(&registry, token.as_deref(), github.as_deref(), file.as_deref()),
        Some(Commands::Login {
            username,
            password,
            registry,
        }) => cmd_login(&registry, &username, &password),
        Some(Commands::Logout { registry }) => cmd_logout(&registry),
        Some(Commands::Whoami { registry }) => cmd_whoami(&registry),
        Some(Commands::Search { query, registry }) => cmd_search(&registry, &query),
        Some(Commands::Info {
            name,
            version,
            registry,
            readme,
        }) => cmd_info(&registry, &name, version.as_deref(), readme),
        Some(Commands::List) => cmd_list(),
        Some(Commands::Uninstall { name }) => cmd_uninstall(&name),
        Some(Commands::Clean {
            lock,
            cache,
            imports,
            all,
        }) => cmd_clean(lock, cache, imports, all),
        Some(Commands::Doctor { registry }) => cmd_doctor(&registry),
        Some(Commands::Install {
            name,
            version,
            registry,
        }) => match name {
            Some(pkg_name) => cmd_install(&registry, &pkg_name, version.as_deref()),
            None => {
                print_install_help();
                Ok(())
            }
        },
        Some(Commands::NpmImport {
            package,
            version,
            alias,
            install,
            registry,
            token,
            with_npm_deps,
            out_dir,
        }) => cmd_npm_import(
            &package,
            version.as_deref(),
            alias.as_deref(),
            install,
            &registry,
            token.as_deref(),
            with_npm_deps,
            out_dir.as_deref(),
        ),
        None => {
            print_default_help();
            Ok(())
        }
    };

    if let Err(err) = result {
        log_error(&format!("vpm error: {err}"));
        std::process::exit(1);
    }
}

fn print_default_help() {
    println!("{}", paint("Void Package Manager", "1;36", false));
    println!("{}", muted(&format!("version {}", env!("CARGO_PKG_VERSION"))));
    println!();

    println!("{}", paint("Usage", "1;36", false));
    println!("  vpm <command> [options]");
    println!("  vpm --help");
    println!();

    println!("{}", paint("Commands", "1;36", false));
    println!("  init        Create a new voidpkg.toml in this directory");
    println!("  install     Install a package from the registry");
    println!("  uninstall   Remove a package (aliases: remove, delete, rm)");
    println!("  info        Show package details");
    println!("  search      Search packages by text");
    println!("  list        Show installed packages");
    println!("  publish     Publish current package");
    println!("  npm-import  Convert npm package to Void package");
    println!("  doctor      Check environment and registry health");
    println!("  server      Start the registry server wrapper");
    println!();

    println!("{}", paint("Global Options", "1;36", false));
    println!("  -h, --help                     Show help");
    println!("  --version                      Show vpm version");
    println!("  -v, --verbose                  Enable verbose logs");
    println!("  --color <auto|always|never>    Control ANSI color output");
    println!();

    println!("Install command help: vpm install --help");
    println!("Full command help:    vpm --help");
}

fn print_install_help() {
    println!("{}", paint("vpm install", "1;36", false));
    println!();
    println!("{}", paint("Usage", "1;36", false));
    println!("  vpm install <name> [--version <VERSION>] [--registry <URL>]");
    println!();

    println!("{}", paint("Quick Commands", "1;36", false));
    println!("  install      vpm install <name> [--version <VERSION>] [--registry <URL>]");
    println!("  uninstall    vpm uninstall <name>  (aliases: remove, delete, rm)");
    println!("  info         vpm info <name> [--version <VERSION>] [--readme]");
    println!("  list         vpm list");
    println!("  clean        vpm clean [--all|--lock|--imports|--cache]");
    println!("  doctor       vpm doctor [--registry <URL>]");
    println!("  server       vpm server");
    println!();
    println!("{}", paint("Auth Commands", "1;36", false));
    println!("  vpm login <username> <password> [--registry <URL>]");
    println!("  vpm logout [--registry <URL>]");
    println!("  vpm whoami [--registry <URL>]");
    println!();
    println!("{}", paint("Output Controls", "1;36", false));
    println!("  --verbose");
    println!("  --color <auto|always|never>");
    println!();
    println!("Full command list: vpm --help");
}

fn new_http_client() -> Result<Client, String> {
    log_verbose("creating HTTP client with 8s connect / 45s request timeout");
    Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(45))
        .user_agent("vpm/0.1")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))
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
    log_success(&format!("Created {}", manifest_path.display()));
    Ok(())
}

fn cmd_login(registry: &str, username: &str, password: &str) -> Result<(), String> {
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("Username and password are required".to_string());
    }

    let client = new_http_client()?;
    let loader = Loader::start(format!(
        "Authenticating with {}",
        normalize_registry(registry)
    ));
    let url = format!("{}/api/login", normalize_registry(registry));
    log_verbose(&format!("POST {}", url));
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "username": username.trim(),
            "password": password,
        }))
        .send()
        .map_err(|e| format!("Login request failed: {e}"))?;

    let status = response.status();
    let api: ApiLoginResponse = response
        .json()
        .map_err(|e| format!("Could not parse login response: {e}"))?;

    if !status.is_success() || !api.ok {
        return Err(api.message);
    }

    let token = api
        .token
        .as_deref()
        .ok_or_else(|| "Login response did not include a token".to_string())?;
    let confirmed_user = api
        .username
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(username.trim());

    save_auth_session(registry, confirmed_user, token)?;
    loader.done();
    log_success(&format!(
        "Logged in as '{}' for {}",
        confirmed_user,
        normalize_registry(registry)
    ));
    log_info(&format!("Saved auth at {}", auth_store_path().display()));
    Ok(())
}

fn cmd_logout(registry: &str) -> Result<(), String> {
    if remove_auth_session(registry)? {
        log_success(&format!("Logged out from {}", normalize_registry(registry)));
    } else {
        log_warn(&format!("No saved login for {}", normalize_registry(registry)));
    }
    Ok(())
}

fn cmd_whoami(registry: &str) -> Result<(), String> {
    let session = load_auth_session(registry)?;
    if let Some(session) = session {
        log_success(&format!(
            "Logged in as '{}' for {}",
            session.username,
            normalize_registry(registry)
        ));
        log_info(&format!("Saved at {}", session.saved_at));
        return Ok(());
    }

    Err(format!(
        "Not logged in for {}. Run: vpm login <username> <password> --registry {}",
        normalize_registry(registry),
        normalize_registry(registry)
    ))
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
        npm_name: None,
    };

    let token_owned = resolve_auth_token(registry, token)?.ok_or_else(|| {
        format!(
            "Publishing requires auth. Run: vpm login <username> <password> --registry {} or pass --token",
            normalize_registry(registry)
        )
    })?;

    let client = new_http_client()?;
    let loader = Loader::start(format!("Publishing {}@{}", payload.name, payload.version));
    let api = if let Some(path) = file {
        log_verbose(&format!("publishing multipart file '{}'", path.display()));
        publish_multipart(&client, registry, Some(token_owned.as_str()), &payload, path)?
    } else {
        log_verbose("publishing JSON payload");
        publish_json(&client, registry, Some(token_owned.as_str()), &payload)?
    };

    if !api.ok {
        return Err(api.message);
    }

    loader.done();
    log_success(&format!("Published {}@{}", payload.name, payload.version));
    Ok(())
}

fn publish_json(
    client: &Client,
    registry: &str,
    token: Option<&str>,
    payload: &PublishPayload,
) -> Result<ApiMessage, String> {
    let url = format!("{}/api/publish", normalize_registry(registry));
    log_verbose(&format!("POST {}", url));
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

fn publish_npm_import_guest(
    client: &Client,
    registry: &str,
    payload: &PublishPayload,
) -> Result<ApiMessage, String> {
    let url = format!("{}/api/publish/npm-import", normalize_registry(registry));
    log_verbose(&format!("POST {}", url));
    let response = client
        .post(url)
        .json(payload)
        .send()
        .map_err(|e| e.to_string())?;
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
    log_verbose(&format!("POST {} (multipart)", url));
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
    let client = new_http_client()?;
    let url = format!("{}/api/search", normalize_registry(registry));
    let loader = Loader::start(format!("Searching '{}' on {}", query, normalize_registry(registry)));
    log_verbose(&format!("GET {}?q={}", url, query));

    let response = client
        .get(url)
        .query(&[("q", query)])
        .send()
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Registry returned status {}", response.status()));
    }

    let packages: Vec<PackageSummary> = response.json().map_err(|e| e.to_string())?;
    loader.done();

    if packages.is_empty() {
        log_warn(&format!("No packages found for '{query}'"));
        return Ok(());
    }

    for pkg in packages {
        println!(
            "{} - {} (author: {}, downloads: {})",
            paint(&format!("{}@{}", pkg.name, pkg.version), "1;36", false),
            pkg.description,
            pkg.author,
            pkg.downloads
        );
    }

    Ok(())
}

fn cmd_info(registry: &str, name: &str, version: Option<&str>, readme: bool) -> Result<(), String> {
    validate_package_name(name)?;

    let client = new_http_client()?;
    let loader = Loader::start(format!("Fetching metadata for {}", name));
    let versions = fetch_registry_package_versions(&client, registry, name)?;
    loader.done();
    if versions.is_empty() {
        return Err(format!("Package '{name}' not found"));
    }

    let selected = if let Some(target_version) = version {
        versions
            .iter()
            .find(|pkg| pkg.version == target_version)
            .ok_or_else(|| format!("Version '{target_version}' not found for '{name}'"))?
    } else {
        versions
            .first()
            .ok_or_else(|| format!("Package '{name}' not found"))?
    };

    println!("{}", paint(&format!("{}@{}", selected.name, selected.version), "1;36", false));
    println!("author: {}", selected.author);
    println!("downloads: {}", selected.downloads);
    println!("published: {}", selected.created_at);
    println!("description: {}", selected.description);
    if !selected.github_repo.trim().is_empty() {
        println!("source: {}", normalize_repo_clone_url(&selected.github_repo));
    }
    if !selected.tarball_url.trim().is_empty() {
        println!("tarball: {}", selected.tarball_url);
    }
    println!(
        "install latest: vpm install {} --registry {}",
        selected.name,
        normalize_registry(registry)
    );
    println!(
        "install exact: vpm install {} --version {} --registry {}",
        selected.name,
        selected.version,
        normalize_registry(registry)
    );

    let version_list = versions
        .iter()
        .map(|pkg| pkg.version.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    println!("versions: {version_list}");

    if readme {
        if selected.readme.trim().is_empty() {
            println!("\nREADME: <empty>");
        } else {
            println!("\nREADME:\n{}", selected.readme);
        }
    } else {
        log_info("hint: pass --readme to print the README");
    }

    Ok(())
}

fn cmd_list() -> Result<(), String> {
    let lock = read_lockfile_or_default(Path::new("void.lock"))?;
    if lock.packages.is_empty() {
        log_warn("No packages in void.lock");
        return Ok(());
    }

    let mut names = lock.packages.keys().cloned().collect::<Vec<_>>();
    names.sort();

    for name in names {
        if let Some(pkg) = lock.packages.get(&name) {
            let installed = PathBuf::from("void_modules").join(&name).exists();
            let status = if installed {
                paint("installed", "1;32", false)
            } else {
                paint("missing", "1;33", false)
            };
            println!(
                "{}@{} [{}] registry={}",
                name, pkg.version, status, pkg.registry
            );
        }
    }
    Ok(())
}

fn cmd_uninstall(name: &str) -> Result<(), String> {
    validate_package_name(name)?;

    let mut removed_any = false;
    let module_dir = PathBuf::from("void_modules").join(name);
    if module_dir.exists() {
        remove_dir_tree(&module_dir)?;
        log_success(&format!("Removed {}", module_dir.display()));
        removed_any = true;
    }

    let lock_path = Path::new("void.lock");
    let mut lock = read_lockfile_or_default(lock_path)?;
    if lock.packages.remove(name).is_some() {
        write_lockfile(lock_path, &lock)?;
        log_success(&format!("Removed {name} from void.lock"));
        removed_any = true;
    }

    if !removed_any {
        return Err(format!("Package '{name}' not found in void_modules or void.lock"));
    }

    Ok(())
}

fn cmd_clean(lock: bool, cache: bool, imports: bool, all: bool) -> Result<(), String> {
    let remove_lock = all || lock;
    let remove_cache = all || cache;
    let remove_imports = all || imports;
    let mut removed_any = false;

    let modules_dir = Path::new("void_modules");
    if modules_dir.exists() {
        remove_dir_tree(modules_dir)?;
        log_success("Removed void_modules");
        removed_any = true;
    } else {
        log_verbose("void_modules not present");
    }

    if remove_imports {
        let imports_dir = Path::new("vpm-imports");
        if imports_dir.exists() {
            remove_dir_tree(imports_dir)?;
            log_success("Removed vpm-imports");
            removed_any = true;
        } else {
            log_verbose("vpm-imports not present");
        }
    }

    if remove_lock {
        let lock_path = Path::new("void.lock");
        if lock_path.exists() {
            fs::remove_file(lock_path).map_err(|e| e.to_string())?;
            log_success("Removed void.lock");
            removed_any = true;
        } else {
            log_verbose("void.lock not present");
        }
    }

    if remove_cache {
        let cache_root = npm_import_cache_root();
        if cache_root.exists() {
            remove_dir_tree(&cache_root)?;
            log_success(&format!("Removed npm import cache {}", cache_root.display()));
            removed_any = true;
        } else {
            log_verbose("npm import cache not present");
        }
    }

    if !removed_any {
        log_warn("Nothing to clean");
        return Ok(());
    }

    if !(remove_lock || remove_cache || remove_imports) {
        log_info("Tip: add --all to clean lock, cache, and vpm-imports too.");
    }
    Ok(())
}

fn cmd_doctor(registry: &str) -> Result<(), String> {
    log_info("Running vpm diagnostics");
    let mut issues = 0usize;

    if command_exists("cargo") {
        log_success("cargo: found");
    } else {
        log_warn("cargo: not found in PATH");
        issues += 1;
    }

    if command_exists("git") {
        log_success("git: found");
    } else {
        log_warn("git: not found in PATH");
        issues += 1;
    }

    let lock_path = Path::new("void.lock");
    if lock_path.exists() {
        log_success(&format!("lockfile: {}", lock_path.display()));
    } else {
        log_info("lockfile: void.lock missing (expected for new projects)");
    }

    let modules_dir = Path::new("void_modules");
    if modules_dir.exists() {
        log_success("void_modules: present");
    } else {
        log_info("void_modules: missing (run vpm install <name> to create)");
    }

    let client = new_http_client()?;
    let loader = Loader::start(format!("Checking registry {}", normalize_registry(registry)));
    let ping_url = format!("{}/api/packages", normalize_registry(registry));
    log_verbose(&format!("GET {}", ping_url));
    let ping_result = client.get(ping_url).send();
    loader.done();
    match ping_result {
        Ok(resp) if resp.status().is_success() => {
            log_success("registry: reachable");
        }
        Ok(resp) => {
            issues += 1;
            log_warn(&format!("registry: status {}", resp.status()));
        }
        Err(err) => {
            issues += 1;
            log_warn(&format!("registry: unreachable ({err})"));
        }
    }

    if issues == 0 {
        log_success("doctor finished with no issues");
    } else {
        log_warn(&format!("doctor found {issues} issue(s)"));
    }
    Ok(())
}

fn cmd_install(registry: &str, name: &str, version: Option<&str>) -> Result<(), String> {
    validate_package_name(name)?;

    let client = new_http_client()?;
    let loader = Loader::start(format!("Installing {name}"));
    let selected = if let Some(target_version) = version {
        fetch_registry_package_version(&client, registry, name, target_version)?
    } else {
        let versions = fetch_registry_package_versions(&client, registry, name)?;
        versions
            .first()
            .cloned()
            .ok_or_else(|| format!("Package '{name}' not found"))?
    };

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
        log_verbose(&format!("GET {}", download_url));
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

    loader.done();
    log_success(&format!("Installed {}@{}", selected.name, selected.version));
    log_info(&format!("Saved metadata at {}", metadata_path.display()));
    Ok(())
}

fn fetch_registry_package_versions(
    client: &Client,
    registry: &str,
    name: &str,
) -> Result<Vec<PackageVersion>, String> {
    let url = format!("{}/api/packages/{}", normalize_registry(registry), name);
    log_verbose(&format!("GET {}", url));
    let response = client.get(url).send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Registry returned status {}", response.status()));
    }
    response.json().map_err(|e| e.to_string())
}

fn fetch_registry_package_version(
    client: &Client,
    registry: &str,
    name: &str,
    version: &str,
) -> Result<PackageVersion, String> {
    let url = format!(
        "{}/api/packages/{}/{}",
        normalize_registry(registry),
        name,
        version
    );
    log_verbose(&format!("GET {}", url));
    let response = client.get(url).send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Registry returned status {}", response.status()));
    }
    response.json().map_err(|e| e.to_string())
}

fn cmd_npm_import(
    package: &str,
    version: Option<&str>,
    alias: Option<&str>,
    install: bool,
    registry: &str,
    token: Option<&str>,
    with_npm_deps: bool,
    out_dir: Option<&Path>,
) -> Result<(), String> {
    if package.trim().is_empty() {
        return Err("Package name cannot be empty".to_string());
    }

    let void_name = alias
        .map(|value| value.to_string())
        .unwrap_or_else(|| npm_name_to_void_name(package));
    validate_package_name(&void_name)?;

    let module_dir = if install {
        PathBuf::from("void_modules").join(&void_name)
    } else if let Some(custom_dir) = out_dir {
        custom_dir.join(&void_name)
    } else {
        PathBuf::from("vpm-imports").join(&void_name)
    };

    let pinned_version = if version.is_none() {
        read_existing_import_version(&module_dir)
    } else {
        None
    };

    let client = new_http_client()?;
    let registry_versions = if install {
        log_info(&format!(
            "Checking website registry for existing {} versions",
            void_name
        ));
        fetch_registry_package_versions(&client, registry, &void_name).map_err(|err| {
            format!("Registry API is required for npm-import --install: {err}")
        })?
    } else {
        Vec::new()
    };

    let encoded_name = encode_npm_name(package);
    let metadata_url = format!("https://registry.npmjs.org/{encoded_name}");
    let metadata_loader = Loader::start(format!("Fetching npm metadata for {}", package));
    log_verbose(&format!("GET {}", metadata_url));
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
        None => {
            if install && !registry_versions.is_empty() {
                let existing = registry_versions
                    .first()
                    .map(|pkg| pkg.version.clone())
                    .unwrap_or_default();
                log_info(&format!(
                    "Using website registry version {existing} for {void_name}. Pass --version to override."
                ));
                existing
            } else if let Some(existing) = pinned_version {
                log_info(&format!(
                    "Using pinned version {existing} from previous import for {package}. Pass --version to change it."
                ));
                existing
            } else {
                root.dist_tags
                    .get("latest")
                    .cloned()
                    .ok_or_else(|| format!("npm package '{package}' does not have a latest dist-tag"))?
            }
        }
    };
    metadata_loader.done();

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
    let main_js_resolved = npm_main_to_js_path(main_js);
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

    if module_dir.exists() {
        remove_dir_tree(&module_dir)?;
    }
    fs::create_dir_all(&module_dir).map_err(|e| e.to_string())?;

    let convert_loader = Loader::start(format!(
        "Converting {}@{} into Void package",
        package, selected_version
    ));
    let cache = ensure_npm_import_cache(&client, package, &selected_version, &tarball_url)?;
    if cache.used_cache {
        log_info(&format!("Using cached npm import for {package}@{selected_version}"));
    }

    let cache_npm_dir = cache.cache_dir.join("npm");
    let npm_dir = module_dir.join("npm");
    copy_dir_recursive(&cache_npm_dir, &npm_dir)?;

    if with_npm_deps {
        install_npm_dependencies(&npm_dir.join("package"))?;
    }
    convert_loader.done();
    log_success(&format!(
        "Imported npm source files: {}",
        cache.source_units
    ));

    let wrapper = npm_wrapper_script(package, &selected_version, &main_js_resolved);
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
        "This package was imported from npm with Node bridge mode.\n\nnpm: {}@{}\nentry.js: {}\nbridge: index.void\n\nUse in Void:\n  use \"{}\" as pkg\n  pkg.run_entry()\n",
        package, selected_version, main_js_resolved, void_name
    );
    fs::write(module_dir.join("NPM_IMPORT.txt"), &source_note).map_err(|e| e.to_string())?;

    if install {
        let registry_has_version = registry_versions
            .iter()
            .any(|pkg| pkg.version == selected_version);

        if registry_has_version {
            log_info(&format!(
                "Website registry already has {}@{} (publish skipped).",
                void_name, selected_version
            ));
        } else {
            let payload = PublishPayload {
                name: void_name.clone(),
                version: selected_version.clone(),
                description: Some(format!("npm import: {package} - {description}")),
                tarball_url: Some(tarball_url.clone()),
                github_repo: if repository.trim().is_empty() {
                    None
                } else {
                    Some(repository.clone())
                },
                readme: Some(source_note.clone()),
                npm_name: Some(package.to_string()),
            };

            let api = if let Some(token_owned) = resolve_auth_token(registry, token)? {
                log_verbose("using authenticated publish for npm-import install");
                publish_json(&client, registry, Some(token_owned.as_str()), &payload)?
            } else {
                log_verbose("using guest npm-import publish endpoint");
                publish_npm_import_guest(&client, registry, &payload)?
            };
            if api.ok {
                log_success(&format!(
                    "Published {}@{} to website registry {}",
                    void_name,
                    selected_version,
                    normalize_registry(registry)
                ));
            } else {
                let message_lower = api.message.to_lowercase();
                if message_lower.contains("unique") {
                    log_info(&format!(
                        "Website registry already contains {}@{} (duplicate publish skipped).",
                        void_name, selected_version
                    ));
                } else {
                    return Err(format!("Website registry publish failed: {}", api.message));
                }
            }
        }

        update_lockfile(
            &void_name,
            &selected_version,
            registry,
            &tarball_url,
            &repository,
        )?;
    }

    log_success(&format!("Imported npm package {package}@{selected_version}"));
    log_info(&format!("Converted to Void package: {void_name}"));
    log_info(&format!("Converted output: {}", module_dir.display()));
    if install {
        log_success("Installed into void_modules.");
        log_info(&format!("Import from Void with: use \"{void_name}\" as pkg"));
    } else {
        log_info("Not installed into void_modules (default behavior).");
        log_info("To install directly, run again with: --install");
    }
    Ok(())
}

fn install_from_github(module_dir: &Path, github_repo: &str) -> Result<(), String> {
    let repo_dir = module_dir.join("repo");
    if repo_dir.exists() {
        remove_dir_tree(&repo_dir)?;
    }
    let clone_url = normalize_repo_clone_url(github_repo);
    log_verbose(&format!("cloning source from {}", clone_url));

    let result = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(&clone_url)
        .arg(&repo_dir)
        .status();

    match result {
        Ok(status) if status.success() => Ok(()),
        _ => {
            log_warn("git clone failed; writing SOURCE.txt fallback");
            let fallback = module_dir.join("SOURCE.txt");
            fs::write(&fallback, format!("GitHub source: {clone_url}\n"))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
    }
}

fn normalize_repo_clone_url(input: &str) -> String {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("git+") {
        return rest.to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("github:") {
        return format!("https://github.com/{}", rest.trim_start_matches('/'));
    }
    trimmed.to_string()
}

fn auth_store_path() -> PathBuf {
    if let Ok(custom) = std::env::var("VPM_AUTH_FILE")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom);
    }

    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home).join(".vpm").join("auth.json");
    }

    PathBuf::from(".vpm-auth.json")
}

fn read_auth_store_or_default(path: &Path) -> Result<AuthStore, String> {
    if !path.exists() {
        return Ok(AuthStore::default());
    }

    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {e}", path.display()))?;
    serde_json::from_str::<AuthStore>(&raw)
        .map_err(|e| format!("Invalid auth store '{}': {e}", path.display()))
}

fn write_auth_store(path: &Path, store: &AuthStore) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create '{}': {e}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    fs::write(path, content)
        .map_err(|e| format!("Failed to write '{}': {e}", path.display()))
}

fn save_auth_session(registry: &str, username: &str, token: &str) -> Result<(), String> {
    let path = auth_store_path();
    let mut store = read_auth_store_or_default(&path)?;
    store.sessions.insert(
        normalize_registry(registry).to_string(),
        AuthSession {
            token: token.to_string(),
            username: username.to_string(),
            saved_at: unix_timestamp_string(),
        },
    );
    write_auth_store(&path, &store)
}

fn load_auth_session(registry: &str) -> Result<Option<AuthSession>, String> {
    let path = auth_store_path();
    let store = read_auth_store_or_default(&path)?;
    Ok(store
        .sessions
        .get(normalize_registry(registry))
        .cloned())
}

fn remove_auth_session(registry: &str) -> Result<bool, String> {
    let path = auth_store_path();
    let mut store = read_auth_store_or_default(&path)?;
    let removed = store
        .sessions
        .remove(normalize_registry(registry))
        .is_some();
    if removed {
        write_auth_store(&path, &store)?;
    }
    Ok(removed)
}

fn resolve_auth_token(registry: &str, explicit: Option<&str>) -> Result<Option<String>, String> {
    if let Some(token) = explicit
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(token.to_string()));
    }

    if let Ok(token_env) = std::env::var("VPM_TOKEN")
        && !token_env.trim().is_empty()
    {
        return Ok(Some(token_env.trim().to_string()));
    }

    if let Some(session) = load_auth_session(registry)? {
        return Ok(Some(session.token));
    }

    Ok(None)
}

fn unix_timestamp_string() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => format!("unix:{}", duration.as_secs()),
        Err(_) => "unix:0".to_string(),
    }
}

fn read_lockfile_or_default(lock_path: &Path) -> Result<LockFile, String> {
    if !lock_path.exists() {
        return Ok(LockFile::default());
    }

    let data = fs::read_to_string(lock_path)
        .map_err(|e| format!("Failed to read '{}': {e}", lock_path.display()))?;
    serde_json::from_str::<LockFile>(&data)
        .map_err(|e| format!("Invalid '{}': {e}", lock_path.display()))
}

fn write_lockfile(lock_path: &Path, lock: &LockFile) -> Result<(), String> {
    let content = serde_json::to_string_pretty(lock).map_err(|e| e.to_string())?;
    fs::write(lock_path, content)
        .map_err(|e| format!("Failed to write '{}': {e}", lock_path.display()))
}

fn update_lockfile(
    name: &str,
    version: &str,
    registry: &str,
    tarball_url: &str,
    github_repo: &str,
) -> Result<(), String> {
    let lock_path = Path::new("void.lock");
    let mut lock = read_lockfile_or_default(lock_path)?;

    lock.packages.insert(
        name.to_string(),
        LockPackage {
            version: version.to_string(),
            registry: normalize_registry(registry).to_string(),
            tarball_url: tarball_url.to_string(),
            github_repo: github_repo.to_string(),
        },
    );

    write_lockfile(lock_path, &lock)
}

fn remove_dir_tree(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    for attempt in 0..4 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(_) => std::thread::sleep(Duration::from_millis(40 * (attempt + 1) as u64)),
        }
    }

    // Fallback: remove children manually, then remove root.
    remove_dir_contents(path)?;
    fs::remove_dir(path).map_err(|e| format!("Failed to remove '{}': {e}", path.display()))?;
    Ok(())
}

fn remove_dir_contents(path: &Path) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            remove_dir_tree(&entry_path)?;
        } else {
            fs::remove_file(&entry_path).map_err(|e| {
                format!("Failed to remove file '{}': {e}", entry_path.display())
            })?;
        }
    }
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

fn normalize_registry(input: &str) -> &str {
    input.trim_end_matches('/')
}

fn absolute_url_from_registry(registry: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    format!("{}/{}", normalize_registry(registry), url.trim_start_matches('/'))
}
