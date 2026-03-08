use std::env;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{Form, Multipart, Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{FromRow, Row, SqlitePool};
use uuid::Uuid;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SESSION_COOKIE: &str = "void_session";
const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;
const INDEX_TEMPLATE: &str = include_str!("../templates/index.html");
const AUTH_GUEST_TEMPLATE: &str = include_str!("../templates/auth_guest.html");
const AUTH_USER_TEMPLATE: &str = include_str!("../templates/auth_user.html");
const PACKAGE_CARD_TEMPLATE: &str = include_str!("../templates/package_card.html");
const PACKAGE_DETAIL_TEMPLATE: &str = include_str!("../templates/package_detail.html");
const VERSION_ITEM_TEMPLATE: &str = include_str!("../templates/version_item.html");
const UPLOAD_INSPECT_TEMPLATE: &str = include_str!("../templates/upload_inspect.html");
const NPM_GHOST_AUTHOR: &str = "npm_ghost";

fn log_info(event: &str, detail: impl AsRef<str>) {
    println!(
        "[void-registry][{}][INFO][{}] {}",
        Utc::now().to_rfc3339(),
        event,
        detail.as_ref()
    );
}

fn log_warn(event: &str, detail: impl AsRef<str>) {
    eprintln!(
        "[void-registry][{}][WARN][{}] {}",
        Utc::now().to_rfc3339(),
        event,
        detail.as_ref()
    );
}

fn log_error(event: &str, detail: impl AsRef<str>) {
    eprintln!(
        "[void-registry][{}][ERROR][{}] {}",
        Utc::now().to_rfc3339(),
        event,
        detail.as_ref()
    );
}

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    upload_dir: PathBuf,
    public_base_url: String,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct PackageVersion {
    name: String,
    version: String,
    description: String,
    author: String,
    tarball_url: String,
    github_repo: String,
    readme: String,
    created_at: String,
    downloads: i64,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct PackageSummary {
    name: String,
    version: String,
    description: String,
    author: String,
    created_at: String,
    downloads: i64,
}

#[derive(Debug, Deserialize)]
struct PublishRequest {
    name: String,
    version: String,
    description: Option<String>,
    tarball_url: Option<String>,
    github_repo: Option<String>,
    readme: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NpmImportPublishRequest {
    name: String,
    version: String,
    npm_name: String,
    description: Option<String>,
    tarball_url: Option<String>,
    github_repo: Option<String>,
    readme: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterForm {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    ok: bool,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ApiLoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct ApiLoginResponse {
    ok: bool,
    message: String,
    token: Option<String>,
    username: Option<String>,
}

#[derive(Debug, FromRow)]
struct UserRecord {
    id: i64,
    username: String,
    password_hash: String,
}

#[derive(Debug, Clone)]
struct AuthUser {
    id: i64,
    username: String,
}

#[derive(Default)]
struct PublishDraft {
    name: String,
    version: String,
    description: String,
    tarball_url: String,
    github_repo: String,
    readme: String,
    uploaded_file: Option<UploadedFile>,
}

struct UploadedFile {
    file_name: String,
    bytes: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args()
        .nth(1)
        .as_deref()
        .is_some_and(|arg| matches!(arg, "--version" | "-v"))
    {
        println!("{VERSION}");
        return Ok(());
    }

    log_info("startup", "booting void-registry");
    let addr = env::var("VOID_REGISTRY_ADDR").unwrap_or_else(|_| "0.0.0.0:4090".to_string());
    let public_base_url = env::var("VOID_REGISTRY_PUBLIC_URL")
        .unwrap_or_else(|_| default_public_url_from_addr(&addr));

    let db_path = env::var("VOID_REGISTRY_DB").unwrap_or_else(|_| "registry.db".to_string());
    let db_abs_path = absolute_path(&db_path)?;
    if let Some(parent) = db_abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&db_abs_path)?;

    let uploads_path = env::var("VOID_REGISTRY_UPLOADS").unwrap_or_else(|_| "uploads".to_string());
    let upload_dir = absolute_path(&uploads_path)?;
    std::fs::create_dir_all(&upload_dir)?;
    log_info(
        "startup",
        format!(
            "config addr={} public_base={} db={} uploads={}",
            addr,
            public_base_url,
            db_abs_path.display(),
            upload_dir.display()
        ),
    );

    let db_url = format!("sqlite://{}", db_abs_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await?;
    log_info("startup", format!("connected to database {}", db_url));

    init_db(&pool).await?;
    log_info("startup", "database schema ready");

    let state = AppState {
        db: pool,
        upload_dir,
        public_base_url,
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/packages/{name}", get(package_detail_handler))
        .route("/packages/{name}/{version}", get(package_version_detail_handler))
        .route("/uploads/{file}/inspect", get(upload_file_inspect_handler))
        .route("/uploads/{file}", get(upload_file_handler))
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/publish", post(publish_form_handler))
        .route("/api/login", post(api_login_handler))
        .route("/api/publish", post(publish_api_handler))
        .route("/api/publish/npm-import", post(publish_npm_import_api_handler))
        .route("/api/publish/upload", post(publish_upload_api_handler))
        .route("/api/packages", get(list_packages_handler))
        .route("/api/packages/{name}/{version}", get(get_package_version_handler))
        .route("/api/packages/{name}", get(get_package_handler))
        .route("/api/search", get(search_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Void registry running at http://{addr}");
    log_info("startup", format!("listening on {}", addr));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    log_info("shutdown", "server stopped");

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    log_warn("shutdown", "received ctrl-c, shutting down gracefully");
}

async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    log_info("db", "running schema initialization");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS packages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            version TEXT NOT NULL,
            description TEXT NOT NULL,
            author TEXT NOT NULL,
            tarball_url TEXT NOT NULL,
            github_repo TEXT NOT NULL,
            readme TEXT NOT NULL,
            user_id INTEGER,
            created_at TEXT NOT NULL,
            UNIQUE(name, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS package_downloads (
            name TEXT NOT NULL,
            version TEXT NOT NULL,
            downloads INTEGER NOT NULL DEFAULT 0,
            last_downloaded_at TEXT NOT NULL,
            PRIMARY KEY(name, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

    if !table_has_column(pool, "packages", "user_id").await? {
        sqlx::query("ALTER TABLE packages ADD COLUMN user_id INTEGER")
            .execute(pool)
            .await?;
    }
    if !table_has_column(pool, "packages", "github_repo").await? {
        sqlx::query("ALTER TABLE packages ADD COLUMN github_repo TEXT NOT NULL DEFAULT ''")
            .execute(pool)
            .await?;
    }

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_package_downloads_name ON package_downloads(name)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
        .execute(pool)
        .await?;

    log_info("db", "schema initialization complete");

    Ok(())
}

async fn table_has_column(pool: &SqlitePool, table: &str, column: &str) -> Result<bool, sqlx::Error> {
    log_info("db", format!("checking column {}.{}", table, column));
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;

    for row in rows {
        let name: String = row.try_get("name")?;
        if name == column {
            log_info("db", format!("column present {}.{}", table, column));
            return Ok(true);
        }
    }

    log_info("db", format!("column missing {}.{}", table, column));
    Ok(false)
}

async fn index_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SearchQuery>,
) -> Result<Html<String>, (StatusCode, String)> {
    log_info(
        "http.index",
        format!(
            "rendering index q='{}' message='{}'",
            query.q.as_deref().unwrap_or(""),
            query.message.as_deref().unwrap_or("")
        ),
    );
    let current_user = auth_user_from_cookie(&state.db, &jar)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let packages = fetch_packages(&state.db, query.q.as_deref())
        .await
        .map_err(internal_error)?;
    log_info(
        "http.index",
        format!("resolved {} package cards", packages.len()),
    );

    let page = render_index_page(
        current_user.as_ref(),
        &packages,
        query.q.as_deref().unwrap_or(""),
        query.message.as_deref(),
        &state.public_base_url,
    );

    Ok(Html(page))
}

async fn package_detail_handler(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Html<String>, (StatusCode, String)> {
    log_info("http.package_detail", format!("render package '{}'", name));
    let versions = fetch_package_versions(&state.db, &name)
        .await
        .map_err(internal_error)?;
    if versions.is_empty() {
        return Err((StatusCode::NOT_FOUND, "Package not found".to_string()));
    }

    let page = render_package_detail_page(&name, &versions[0], &versions, &state.public_base_url);
    Ok(Html(page))
}

async fn package_version_detail_handler(
    State(state): State<AppState>,
    AxumPath((name, version)): AxumPath<(String, String)>,
) -> Result<Html<String>, (StatusCode, String)> {
    log_info(
        "http.package_version_detail",
        format!("render package '{}@{}'", name, version),
    );
    let versions = fetch_package_versions(&state.db, &name)
        .await
        .map_err(internal_error)?;
    if versions.is_empty() {
        return Err((StatusCode::NOT_FOUND, "Package not found".to_string()));
    }

    let selected = versions
        .iter()
        .find(|pkg| pkg.version == version)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Version not found".to_string()))?;

    let page = render_package_detail_page(&name, selected, &versions, &state.public_base_url);
    Ok(Html(page))
}

async fn upload_file_handler(
    AxumPath(file): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    log_info("http.upload_file", format!("request file='{}'", file));
    let safe_name = match sanitize_filename(&file) {
        Some(v) => v,
        None => {
            log_warn("http.upload_file", "rejected invalid file name");
            return (
                StatusCode::BAD_REQUEST,
                "invalid file name".to_string(),
            )
                .into_response();
        }
    };

    let file_path = state.upload_dir.join(safe_name);
    match std::fs::read(&file_path) {
        Ok(bytes) => {
            log_info(
                "http.upload_file",
                format!("served '{}' ({} bytes)", file_path.display(), bytes.len()),
            );
            let content_type = content_type_for_path(&file_path);
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            );
            (StatusCode::OK, headers, bytes).into_response()
        }
        Err(err) => {
            log_warn(
                "http.upload_file",
                format!("not found '{}' ({})", file_path.display(), err),
            );
            (StatusCode::NOT_FOUND, "not found".to_string()).into_response()
        }
    }
}

async fn upload_file_inspect_handler(
    AxumPath(file): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Html<String>, (StatusCode, String)> {
    log_info("http.upload_inspect", format!("inspect file='{}'", file));
    let safe_name = sanitize_filename(&file)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "invalid file name".to_string()))?;
    let file_path = state.upload_dir.join(&safe_name);
    let bytes = std::fs::read(&file_path).map_err(|err| {
        (
            StatusCode::NOT_FOUND,
            format!("Could not read '{}': {}", file_path.display(), err),
        )
    })?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha256 = format!("{:x}", hasher.finalize());
    let content_type = content_type_for_path(&file_path);
    let (preview_kind, preview) = build_upload_preview(&safe_name, &bytes);

    let page = UPLOAD_INSPECT_TEMPLATE
        .replace("{{FILE_NAME}}", &escape_html(&safe_name))
        .replace("{{FILE_SIZE}}", &bytes.len().to_string())
        .replace("{{SHA256}}", &sha256)
        .replace("{{CONTENT_TYPE}}", content_type)
        .replace(
            "{{RAW_URL}}",
            &escape_html(&format!("/uploads/{}", safe_name)),
        )
        .replace("{{PREVIEW_KIND}}", &escape_html(preview_kind))
        .replace("{{PREVIEW}}", &escape_html(&preview));

    Ok(Html(page))
}

async fn register_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(payload): Form<RegisterForm>,
) -> impl IntoResponse {
    log_info(
        "http.register",
        format!("attempt username='{}'", payload.username.trim()),
    );
    if validate_username(&payload.username).is_err() {
        log_warn("http.register", "invalid username format");
        return (jar, Redirect::to("/?message=register_invalid")).into_response();
    }

    if payload.password.len() < 8 {
        log_warn("http.register", "password too short");
        return (jar, Redirect::to("/?message=password_short")).into_response();
    }

    let password_hash = match hash_password(&payload.password) {
        Ok(v) => v,
        Err(err) => {
            log_error("http.register", format!("password hash failed: {}", err));
            return (jar, Redirect::to("/?message=register_failed")).into_response();
        }
    };

    let created_at = Utc::now().to_rfc3339();
    let result = sqlx::query("INSERT INTO users (username, password_hash, created_at) VALUES (?, ?, ?)")
        .bind(payload.username.trim())
        .bind(password_hash)
        .bind(created_at)
        .execute(&state.db)
        .await;

    let inserted = match result {
        Ok(v) => v,
        Err(err) => {
            if err.to_string().to_lowercase().contains("unique") {
                log_warn("http.register", "username already exists");
                return (jar, Redirect::to("/?message=user_exists")).into_response();
            }
            log_error("http.register", format!("insert failed: {}", err));
            return (jar, Redirect::to("/?message=register_failed")).into_response();
        }
    };

    let token = match create_session(&state.db, inserted.last_insert_rowid()).await {
        Ok(v) => v,
        Err(err) => {
            log_error("http.register", format!("create session failed: {}", err));
            return (jar, Redirect::to("/?message=register_failed")).into_response();
        }
    };

    log_info("http.register", "registration succeeded");
    let jar = jar.add(session_cookie(&token));
    (jar, Redirect::to("/?message=registered")).into_response()
}

async fn login_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(payload): Form<LoginForm>,
) -> impl IntoResponse {
    log_info(
        "http.login",
        format!("attempt username='{}'", payload.username.trim()),
    );
    let user = match find_user_by_username(&state.db, payload.username.trim()).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            log_warn("http.login", "user not found");
            return (jar, Redirect::to("/?message=bad_credentials")).into_response();
        }
        Err(err) => {
            log_error("http.login", format!("lookup failed: {}", err));
            return (jar, Redirect::to("/?message=login_failed")).into_response();
        }
    };

    if !verify_password(&payload.password, &user.password_hash) {
        log_warn("http.login", "invalid password");
        return (jar, Redirect::to("/?message=bad_credentials")).into_response();
    }

    let token = match create_session(&state.db, user.id).await {
        Ok(v) => v,
        Err(err) => {
            log_error("http.login", format!("create session failed: {}", err));
            return (jar, Redirect::to("/?message=login_failed")).into_response();
        }
    };

    log_info("http.login", format!("login succeeded user='{}'", user.username));
    let jar = jar.add(session_cookie(&token));
    (jar, Redirect::to("/?message=logged_in")).into_response()
}

async fn logout_handler(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        log_info("http.logout", "session cookie present, deleting session");
        let _ = sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(cookie.value())
            .execute(&state.db)
            .await;
    } else {
        log_info("http.logout", "no session cookie, noop logout");
    }

    let jar = jar.remove(Cookie::build((SESSION_COOKIE, "")).path("/").build());
    log_info("http.logout", "logout completed");
    (jar, Redirect::to("/?message=logged_out")).into_response()
}

async fn publish_form_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    multipart: Multipart,
) -> impl IntoResponse {
    log_info("http.publish_form", "publish form request received");
    let user = match auth_user_from_cookie(&state.db, &jar).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            log_warn("http.publish_form", "missing auth cookie");
            return (jar, Redirect::to("/?message=login_required")).into_response();
        }
        Err(err) => {
            log_error("http.publish_form", format!("auth lookup failed: {}", err));
            return (jar, Redirect::to("/?message=publish_failed")).into_response();
        }
    };

    let draft = match parse_publish_multipart(multipart).await {
        Ok(v) => v,
        Err(err) => {
            log_error("http.publish_form", format!("multipart parse failed: {}", err));
            return (jar, Redirect::to("/?message=publish_failed")).into_response();
        }
    };

    let payload = match finalize_publish_draft(&state, draft) {
        Ok(v) => v,
        Err(err) => {
            log_error("http.publish_form", format!("payload validation failed: {}", err));
            return (jar, Redirect::to("/?message=publish_failed")).into_response();
        }
    };
    log_info(
        "http.publish_form",
        format!(
            "user='{}' publishing {}@{}",
            user.username, payload.name, payload.version
        ),
    );

    match insert_package(&state.db, payload, &user).await {
        Ok(()) => {
            log_info("http.publish_form", "publish succeeded");
            (jar, Redirect::to("/?message=published")).into_response()
        }
        Err(err) => {
            log_error("http.publish_form", format!("publish failed: {}", err));
            (jar, Redirect::to("/?message=publish_failed")).into_response()
        }
    }
}

async fn api_login_handler(
    State(state): State<AppState>,
    Json(payload): Json<ApiLoginRequest>,
) -> impl IntoResponse {
    log_info(
        "api.login",
        format!("attempt username='{}'", payload.username.trim()),
    );
    let user = match find_user_by_username(&state.db, payload.username.trim()).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            log_warn("api.login", "user not found");
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiLoginResponse {
                    ok: false,
                    message: "Invalid username or password".to_string(),
                    token: None,
                    username: None,
                }),
            )
                .into_response();
        }
        Err(_) => {
            log_error("api.login", "database lookup failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiLoginResponse {
                    ok: false,
                    message: "Login failed".to_string(),
                    token: None,
                    username: None,
                }),
            )
                .into_response();
        }
    };

    if !verify_password(&payload.password, &user.password_hash) {
        log_warn("api.login", "invalid password");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiLoginResponse {
                ok: false,
                message: "Invalid username or password".to_string(),
                token: None,
                username: None,
            }),
        )
            .into_response();
    }

    match create_session(&state.db, user.id).await {
        Ok(token) => {
            log_info("api.login", format!("login succeeded user='{}'", user.username));
            (
                StatusCode::OK,
                Json(ApiLoginResponse {
                    ok: true,
                    message: "Login successful".to_string(),
                    token: Some(token),
                    username: Some(user.username),
                }),
            )
                .into_response()
        }
        Err(err) => {
            log_error("api.login", format!("create session failed: {}", err));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiLoginResponse {
                    ok: false,
                    message: "Could not create session".to_string(),
                    token: None,
                    username: None,
                }),
            )
                .into_response()
        }
    }
}

async fn publish_api_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<PublishRequest>,
) -> impl IntoResponse {
    log_info("api.publish", "json publish request received");
    let user = match auth_user_for_api(&state.db, &jar, &headers).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            log_warn("api.publish", "authentication required");
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiMessage {
                    ok: false,
                    message: "Authentication required".to_string(),
                }),
            )
                .into_response();
        }
        Err(err) => {
            log_error("api.publish", format!("auth failed: {}", err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response();
        }
    };
    log_info(
        "api.publish",
        format!(
            "user='{}' publishing {}@{}",
            user.username, payload.name, payload.version
        ),
    );

    match validate_publish_request(payload) {
        Ok(valid) => match insert_package(&state.db, valid, &user).await {
            Ok(()) => {
                log_info("api.publish", "publish succeeded");
                (
                    StatusCode::CREATED,
                    Json(ApiMessage {
                        ok: true,
                        message: "Package published".to_string(),
                    }),
                )
                    .into_response()
            }
            Err(err) => (
                StatusCode::BAD_REQUEST,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response(),
        },
        Err(err) => {
            log_warn("api.publish", format!("validation failed: {}", err));
            (
                StatusCode::BAD_REQUEST,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response()
        }
    }
}

async fn publish_npm_import_api_handler(
    State(state): State<AppState>,
    Json(payload): Json<NpmImportPublishRequest>,
) -> impl IntoResponse {
    log_info("api.publish_npm_import", "guest npm-import publish request received");

    match validate_npm_import_publish_request(payload) {
        Ok(valid) => {
            log_info(
                "api.publish_npm_import",
                format!("guest publishing {}@{}", valid.name, valid.version),
            );
            match insert_package_guest(&state.db, valid, NPM_GHOST_AUTHOR).await {
                Ok(()) => {
                    log_info("api.publish_npm_import", "publish succeeded");
                    (
                        StatusCode::CREATED,
                        Json(ApiMessage {
                            ok: true,
                            message: "Package published".to_string(),
                        }),
                    )
                        .into_response()
                }
                Err(err) => (
                    StatusCode::BAD_REQUEST,
                    Json(ApiMessage {
                        ok: false,
                        message: err,
                    }),
                )
                    .into_response(),
            }
        }
        Err(err) => {
            log_warn(
                "api.publish_npm_import",
                format!("validation failed: {}", err),
            );
            (
                StatusCode::BAD_REQUEST,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response()
        }
    }
}

async fn publish_upload_api_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    multipart: Multipart,
) -> impl IntoResponse {
    log_info("api.publish_upload", "multipart publish request received");
    let user = match auth_user_for_api(&state.db, &jar, &headers).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            log_warn("api.publish_upload", "authentication required");
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiMessage {
                    ok: false,
                    message: "Authentication required".to_string(),
                }),
            )
                .into_response();
        }
        Err(err) => {
            log_error("api.publish_upload", format!("auth failed: {}", err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response();
        }
    };

    let draft = match parse_publish_multipart(multipart).await {
        Ok(v) => v,
        Err(err) => {
            log_warn("api.publish_upload", format!("multipart parse failed: {}", err));
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response();
        }
    };

    let payload = match finalize_publish_draft(&state, draft) {
        Ok(v) => v,
        Err(err) => {
            log_warn("api.publish_upload", format!("validation failed: {}", err));
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response();
        }
    };
    log_info(
        "api.publish_upload",
        format!(
            "user='{}' publishing {}@{}",
            user.username, payload.name, payload.version
        ),
    );

    match insert_package(&state.db, payload, &user).await {
        Ok(()) => {
            log_info("api.publish_upload", "publish succeeded");
            (
                StatusCode::CREATED,
                Json(ApiMessage {
                    ok: true,
                    message: "Package published".to_string(),
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(ApiMessage {
                ok: false,
                message: err,
            }),
        )
            .into_response(),
    }
}

async fn list_packages_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<PackageSummary>>, (StatusCode, String)> {
    log_info("api.list_packages", "list latest packages");
    let packages = fetch_packages(&state.db, None).await.map_err(internal_error)?;
    log_info(
        "api.list_packages",
        format!("returning {} package summaries", packages.len()),
    );
    Ok(Json(packages))
}

async fn get_package_handler(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<Vec<PackageVersion>>, (StatusCode, String)> {
    log_info("api.get_package", format!("fetch package '{}'", name));
    let mut rows = fetch_package_versions(&state.db, &name)
        .await
        .map_err(internal_error)?;
    if let Some(latest) = rows.first_mut() {
        if let Err(err) = increment_download(&state.db, &latest.name, &latest.version).await {
            log_warn(
                "api.get_package",
                format!("download increment failed for {}@{}: {}", latest.name, latest.version, err),
            );
        } else {
            latest.downloads += 1;
        }
    }
    log_info(
        "api.get_package",
        format!("package '{}' has {} versions", name, rows.len()),
    );

    Ok(Json(rows))
}

async fn get_package_version_handler(
    State(state): State<AppState>,
    AxumPath((name, version)): AxumPath<(String, String)>,
) -> Result<Json<PackageVersion>, (StatusCode, String)> {
    log_info(
        "api.get_package_version",
        format!("fetch package '{}@{}'", name, version),
    );
    let mut pkg = fetch_package_version(&state.db, &name, &version)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Package version not found".to_string()))?;

    if let Err(err) = increment_download(&state.db, &pkg.name, &pkg.version).await {
        log_warn(
            "api.get_package_version",
            format!("download increment failed for {}@{}: {}", pkg.name, pkg.version, err),
        );
    } else {
        pkg.downloads += 1;
    }

    Ok(Json(pkg))
}

async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<PackageSummary>>, (StatusCode, String)> {
    log_info(
        "api.search",
        format!("query='{}'", params.q.as_deref().unwrap_or("")),
    );
    let rows = fetch_packages(&state.db, params.q.as_deref())
        .await
        .map_err(internal_error)?;
    log_info("api.search", format!("returning {} rows", rows.len()));

    Ok(Json(rows))
}

async fn fetch_packages(pool: &SqlitePool, query: Option<&str>) -> Result<Vec<PackageSummary>, sqlx::Error> {
    if let Some(q) = query
        && !q.trim().is_empty()
    {
        log_info("db.fetch_packages", format!("search mode q='{}'", q.trim()));
        let like = format!("%{}%", q.trim());
        return sqlx::query_as::<_, PackageSummary>(
            r#"
            SELECT p.name, p.version, p.description, p.author, p.created_at,
                   COALESCE(d.total_downloads, 0) AS downloads
            FROM packages p
            INNER JOIN (
                SELECT name, MAX(created_at) AS max_created
                FROM packages
                GROUP BY name
            ) latest
            ON p.name = latest.name AND p.created_at = latest.max_created
            LEFT JOIN (
                SELECT name, SUM(downloads) AS total_downloads
                FROM package_downloads
                GROUP BY name
            ) d
            ON p.name = d.name
            WHERE p.name LIKE ? OR p.description LIKE ? OR p.author LIKE ?
            ORDER BY p.created_at DESC
            LIMIT 100
            "#,
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_all(pool)
        .await;
    }

    log_info("db.fetch_packages", "latest mode (no search query)");

    sqlx::query_as::<_, PackageSummary>(
        r#"
        SELECT p.name, p.version, p.description, p.author, p.created_at,
               COALESCE(d.total_downloads, 0) AS downloads
        FROM packages p
        INNER JOIN (
            SELECT name, MAX(created_at) AS max_created
            FROM packages
            GROUP BY name
        ) latest
        ON p.name = latest.name AND p.created_at = latest.max_created
        LEFT JOIN (
            SELECT name, SUM(downloads) AS total_downloads
            FROM package_downloads
            GROUP BY name
        ) d
        ON p.name = d.name
        ORDER BY p.created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_package_versions(pool: &SqlitePool, name: &str) -> Result<Vec<PackageVersion>, sqlx::Error> {
    sqlx::query_as::<_, PackageVersion>(
        r#"
        SELECT p.name, p.version, p.description, p.author, p.tarball_url, p.github_repo, p.readme, p.created_at,
               COALESCE(d.downloads, 0) AS downloads
        FROM packages p
        LEFT JOIN package_downloads d
        ON p.name = d.name AND p.version = d.version
        WHERE p.name = ?
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(name)
    .fetch_all(pool)
    .await
}

async fn fetch_package_version(
    pool: &SqlitePool,
    name: &str,
    version: &str,
) -> Result<Option<PackageVersion>, sqlx::Error> {
    sqlx::query_as::<_, PackageVersion>(
        r#"
        SELECT p.name, p.version, p.description, p.author, p.tarball_url, p.github_repo, p.readme, p.created_at,
               COALESCE(d.downloads, 0) AS downloads
        FROM packages p
        LEFT JOIN package_downloads d
        ON p.name = d.name AND p.version = d.version
        WHERE p.name = ? AND p.version = ?
        LIMIT 1
        "#,
    )
    .bind(name)
    .bind(version)
    .fetch_optional(pool)
    .await
}

async fn increment_download(pool: &SqlitePool, name: &str, version: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO package_downloads (name, version, downloads, last_downloaded_at)
        VALUES (?, ?, 1, ?)
        ON CONFLICT(name, version) DO UPDATE
        SET downloads = downloads + 1,
            last_downloaded_at = excluded.last_downloaded_at
        "#,
    )
    .bind(name)
    .bind(version)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| format!("Database error: {e}"))?;
    Ok(())
}

async fn auth_user_from_cookie(pool: &SqlitePool, jar: &CookieJar) -> Result<Option<AuthUser>, String> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        log_info("auth.cookie", "session cookie present, validating token");
        return auth_user_from_token(pool, cookie.value())
            .await
            .map_err(|e| format!("Database error: {e}"));
    }
    log_info("auth.cookie", "no session cookie");
    Ok(None)
}

async fn auth_user_for_api(
    pool: &SqlitePool,
    jar: &CookieJar,
    headers: &HeaderMap,
) -> Result<Option<AuthUser>, String> {
    if let Some(user) = auth_user_from_cookie(pool, jar).await? {
        log_info("auth.api", format!("authenticated via cookie as '{}'", user.username));
        return Ok(Some(user));
    }

    if let Some(token) = extract_bearer_token(headers) {
        log_info("auth.api", "bearer token found, validating");
        return auth_user_from_token(pool, &token)
            .await
            .map_err(|e| format!("Database error: {e}"));
    }

    log_warn("auth.api", "no auth cookie or bearer token");
    Ok(None)
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty() {
        return Some(token.trim().to_string());
    }
    None
}

async fn auth_user_from_token(pool: &SqlitePool, token: &str) -> Result<Option<AuthUser>, sqlx::Error> {
    log_info(
        "auth.token",
        format!("validating token prefix='{}...'", &token.chars().take(8).collect::<String>()),
    );
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query_as::<_, (i64, String)>(
        r#"
        SELECT u.id, u.username
        FROM sessions s
        INNER JOIN users u ON u.id = s.user_id
        WHERE s.token = ? AND s.expires_at > ?
        LIMIT 1
        "#,
    )
    .bind(token)
    .bind(now)
    .fetch_optional(pool)
    .await?;

    let user = row.map(|(id, username)| AuthUser { id, username });
    if let Some(found) = user.as_ref() {
        log_info("auth.token", format!("token valid for user='{}'", found.username));
    } else {
        log_warn("auth.token", "token invalid or expired");
    }
    Ok(user)
}

async fn find_user_by_username(pool: &SqlitePool, username: &str) -> Result<Option<UserRecord>, sqlx::Error> {
    sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, password_hash FROM users WHERE username = ? LIMIT 1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

async fn create_session(pool: &SqlitePool, user_id: i64) -> Result<String, String> {
    log_info("auth.session", format!("creating session for user_id={}", user_id));
    let token = random_token();
    let created_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + Duration::days(30)).to_rfc3339();

    sqlx::query("INSERT INTO sessions (user_id, token, created_at, expires_at) VALUES (?, ?, ?, ?)")
        .bind(user_id)
        .bind(&token)
        .bind(created_at)
        .bind(expires_at)
        .execute(pool)
        .await
        .map_err(|e| format!("Database error: {e}"))?;

    log_info("auth.session", format!("session created for user_id={}", user_id));
    Ok(token)
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn session_cookie(token: &str) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .build()
}

fn normalize_npm_name_for_void(npm_name: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;

    for ch in npm_name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }

    out.trim_matches('_').to_string()
}

fn validate_npm_import_publish_request(
    mut payload: NpmImportPublishRequest,
) -> Result<PublishRequest, String> {
    payload.name = payload.name.trim().to_string();
    payload.version = payload.version.trim().to_string();
    payload.npm_name = payload.npm_name.trim().to_string();
    payload.description = payload.description.map(|s| s.trim().to_string());
    payload.tarball_url = payload.tarball_url.map(|s| s.trim().to_string());
    payload.github_repo = payload.github_repo.map(|s| s.trim().to_string());

    validate_name(&payload.name)?;
    validate_version(&payload.version)?;

    if payload.npm_name.is_empty() {
        return Err("npm_name is required for npm-import guest publish".to_string());
    }

    let normalized = normalize_npm_name_for_void(&payload.npm_name);
    if normalized.is_empty() {
        return Err("npm_name is not valid".to_string());
    }
    let expected_prefixed = format!("npm_{}", normalized);
    if payload.name != normalized && payload.name != expected_prefixed {
        return Err(format!(
            "npm-import guest publish only allows '{}' or '{}' package names",
            normalized, expected_prefixed
        ));
    }

    let tarball = payload.tarball_url.as_deref().unwrap_or_default();
    if tarball.is_empty() {
        return Err("npm-import guest publish requires tarball_url".to_string());
    }
    if !tarball.starts_with("https://registry.npmjs.org/") {
        return Err("npm-import guest publish requires an npm registry tarball URL".to_string());
    }
    if !tarball.contains(&payload.version) {
        return Err("tarball_url must match the published version".to_string());
    }

    Ok(PublishRequest {
        name: payload.name,
        version: payload.version,
        description: payload.description,
        tarball_url: payload.tarball_url,
        github_repo: payload.github_repo,
        readme: payload.readme,
    })
}

fn validate_publish_request(mut payload: PublishRequest) -> Result<PublishRequest, String> {
    payload.name = payload.name.trim().to_string();
    payload.version = payload.version.trim().to_string();
    payload.description = payload.description.map(|s| s.trim().to_string());
    payload.tarball_url = payload.tarball_url.map(|s| s.trim().to_string());
    payload.github_repo = payload.github_repo.map(|s| s.trim().to_string());

    validate_name(&payload.name)?;
    validate_version(&payload.version)?;

    let tarball = payload.tarball_url.as_deref().unwrap_or_default();
    let github = payload.github_repo.as_deref().unwrap_or_default();

    if tarball.is_empty() && github.is_empty() {
        return Err("Provide a GitHub repo URL, tarball URL, or upload file".to_string());
    }

    if !github.is_empty() {
        validate_github_repo(github)?;
    }

    Ok(payload)
}

fn finalize_publish_draft(state: &AppState, mut draft: PublishDraft) -> Result<PublishRequest, String> {
    if let Some(uploaded) = draft.uploaded_file {
        let file_url = save_upload_and_build_url(
            &state.upload_dir,
            &state.public_base_url,
            &draft.name,
            &draft.version,
            uploaded,
        )?;
        draft.tarball_url = file_url;
    }

    validate_publish_request(PublishRequest {
        name: draft.name,
        version: draft.version,
        description: if draft.description.trim().is_empty() {
            None
        } else {
            Some(draft.description)
        },
        tarball_url: if draft.tarball_url.trim().is_empty() {
            None
        } else {
            Some(draft.tarball_url)
        },
        github_repo: if draft.github_repo.trim().is_empty() {
            None
        } else {
            Some(draft.github_repo)
        },
        readme: if draft.readme.trim().is_empty() {
            None
        } else {
            Some(draft.readme)
        },
    })
}

async fn parse_publish_multipart(mut multipart: Multipart) -> Result<PublishDraft, String> {
    log_info("publish.multipart", "parsing multipart fields");
    let mut draft = PublishDraft::default();

    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let name = field.name().unwrap_or_default().to_string();

        if name == "package_file" {
            let original_name = field.file_name().unwrap_or("package.bin").to_string();
            let bytes = field.bytes().await.map_err(|e| e.to_string())?.to_vec();
            if !bytes.is_empty() {
                if bytes.len() > MAX_UPLOAD_BYTES {
                    log_warn(
                        "publish.multipart",
                        format!("file too large name='{}' bytes={}", original_name, bytes.len()),
                    );
                    return Err("Uploaded file is too large".to_string());
                }
                draft.uploaded_file = Some(UploadedFile {
                    file_name: original_name,
                    bytes,
                });
                log_info(
                    "publish.multipart",
                    format!(
                        "received upload file='{}' bytes={}",
                        draft
                            .uploaded_file
                            .as_ref()
                            .map(|f| f.file_name.as_str())
                            .unwrap_or(""),
                        draft.uploaded_file.as_ref().map(|f| f.bytes.len()).unwrap_or(0)
                    ),
                );
            }
            continue;
        }

        let value = field.text().await.map_err(|e| e.to_string())?;
        match name.as_str() {
            "name" => draft.name = value.trim().to_string(),
            "version" => draft.version = value.trim().to_string(),
            "description" => draft.description = value.trim().to_string(),
            "tarball_url" => draft.tarball_url = value.trim().to_string(),
            "github_repo" => draft.github_repo = value.trim().to_string(),
            "readme" => draft.readme = value,
            _ => {}
        }
    }

    if draft.name.is_empty() || draft.version.is_empty() {
        log_warn("publish.multipart", "missing required name/version fields");
        return Err("name and version are required".to_string());
    }

    log_info(
        "publish.multipart",
        format!("parsed draft {}@{}", draft.name, draft.version),
    );

    Ok(draft)
}

fn save_upload_and_build_url(
    upload_dir: &Path,
    public_base: &str,
    package_name: &str,
    version: &str,
    uploaded: UploadedFile,
) -> Result<String, String> {
    let safe_original = sanitize_filename(&uploaded.file_name).unwrap_or_else(|| "package.bin".to_string());
    let extension = Path::new(&safe_original)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin");

    let safe_name = slugify(package_name);
    let safe_version = slugify(version);
    let file_name = format!(
        "{}-{}-{}.{}",
        safe_name,
        safe_version,
        Uuid::new_v4().simple(),
        extension
    );

    let full_path = upload_dir.join(&file_name);
    std::fs::write(&full_path, uploaded.bytes).map_err(|e| e.to_string())?;
    log_info(
        "publish.upload",
        format!(
            "stored upload for {}@{} at {}",
            package_name,
            version,
            full_path.display()
        ),
    );

    Ok(format!(
        "{}/uploads/{}",
        public_base.trim_end_matches('/'),
        file_name
    ))
}

async fn insert_package(pool: &SqlitePool, payload: PublishRequest, user: &AuthUser) -> Result<(), String> {
    insert_package_with_owner(pool, payload, Some(user), None).await
}

async fn insert_package_guest(
    pool: &SqlitePool,
    payload: PublishRequest,
    author_name: &str,
) -> Result<(), String> {
    insert_package_with_owner(pool, payload, None, Some(author_name)).await
}

async fn insert_package_with_owner(
    pool: &SqlitePool,
    payload: PublishRequest,
    user: Option<&AuthUser>,
    author_override: Option<&str>,
) -> Result<(), String> {
    let actor = user.map(|u| format!("user='{}'", u.username)).unwrap_or_else(|| "user='<guest>'".to_string());
    log_info(
        "db.insert_package",
        format!("attempt by {} for {}@{}", actor, payload.name, payload.version),
    );
    let existing_owner = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM packages WHERE name = ? AND user_id IS NOT NULL LIMIT 1",
    )
    .bind(&payload.name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Database error: {e}"))?;

    if let Some(owner_id) = existing_owner {
        match user {
            Some(current_user) if owner_id == current_user.id => {}
            _ => {
                log_warn(
                    "db.insert_package",
                    format!("ownership conflict for package '{}'", payload.name),
                );
                return Err("Package name is already owned by another account".to_string());
            }
        }
    }

    let description = payload.description.unwrap_or_default();
    let tarball_url = payload.tarball_url.unwrap_or_default();
    let github_repo = payload.github_repo.unwrap_or_default();
    let readme = payload.readme.unwrap_or_default();
    let created_at = Utc::now().to_rfc3339();
    let package_name = payload.name.clone();
    let package_version = payload.version.clone();
    let author = author_override
        .map(ToString::to_string)
        .or_else(|| user.map(|u| u.username.clone()))
        .unwrap_or_else(|| NPM_GHOST_AUTHOR.to_string());
    let user_id = user.map(|u| u.id);

    sqlx::query(
        r#"
        INSERT INTO packages (name, version, description, author, tarball_url, github_repo, readme, user_id, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&payload.name)
    .bind(&payload.version)
    .bind(description)
    .bind(author.clone())
    .bind(tarball_url)
    .bind(github_repo)
    .bind(readme)
    .bind(user_id)
    .bind(created_at)
    .execute(pool)
    .await
    .map_err(|e| format!("Database error: {e}"))?;

    log_info(
        "db.insert_package",
        format!(
            "inserted {}@{} by '{}'",
            package_name, package_version, author
        ),
    );

    Ok(())
}

fn validate_username(username: &str) -> Result<(), String> {
    let trimmed = username.trim();
    if trimmed.len() < 3 {
        return Err("Username must be at least 3 characters".to_string());
    }
    if trimmed.len() > 32 {
        return Err("Username must be at most 32 characters".to_string());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Username may only contain letters, numbers, '-' and '_'".to_string());
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Package name cannot be empty".to_string());
    }
    if name.len() > 120 {
        return Err("Package name too long".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Package name may only contain letters, numbers, '-' and '_'".to_string());
    }
    Ok(())
}

fn validate_version(version: &str) -> Result<(), String> {
    if version.is_empty() {
        return Err("Version cannot be empty".to_string());
    }
    if version.len() > 40 {
        return Err("Version is too long".to_string());
    }
    if !version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
    {
        return Err("Version contains invalid characters".to_string());
    }
    Ok(())
}

fn validate_github_repo(url: &str) -> Result<(), String> {
    if !url.starts_with("https://github.com/") {
        return Err("GitHub URL must start with https://github.com/".to_string());
    }
    let path = url.trim_start_matches("https://github.com/");
    if !path.contains('/') {
        return Err("GitHub URL must include owner/repo".to_string());
    }
    Ok(())
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt_seed = Uuid::new_v4();
    let salt = SaltString::encode_b64(salt_seed.as_bytes())
        .map_err(|e| format!("Salt generation error: {e}"))?;

    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| format!("Password hash error: {e}"))
}

fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(v) => v,
        Err(_) => return false,
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn message_text(code: &str) -> Option<&'static str> {
    match code {
        "registered" => Some("Account created and logged in."),
        "logged_in" => Some("Logged in successfully."),
        "logged_out" => Some("Logged out."),
        "published" => Some("Package published."),
        "login_required" => Some("Login is required for publishing."),
        "user_exists" => Some("That username is already taken."),
        "bad_credentials" => Some("Invalid username or password."),
        "password_short" => Some("Password must be at least 8 characters."),
        "register_invalid" => Some("Username format is invalid."),
        "register_failed" => Some("Could not create account."),
        "login_failed" => Some("Could not complete login."),
        "publish_failed" => Some("Publish failed. Provide GitHub URL or upload package file."),
        _ => None,
    }
}

fn render_index_page(
    current_user: Option<&AuthUser>,
    packages: &[PackageSummary],
    search: &str,
    message_code: Option<&str>,
    registry_url: &str,
) -> String {
    let auth_section = render_auth_section(current_user);
    let package_cards = render_package_cards(packages, registry_url);
    let message_banner = message_code
        .and_then(message_text)
        .map(|m| format!("<div class=\"notice\">{}</div>", escape_html(m)))
        .unwrap_or_default();

    INDEX_TEMPLATE
        .replace("{{MESSAGE_BANNER}}", &message_banner)
        .replace("{{AUTH_SECTION}}", &auth_section)
        .replace("{{PACKAGE_CARDS}}", &package_cards)
        .replace("{{SEARCH_VALUE}}", &escape_html(search))
}

fn render_auth_section(current_user: Option<&AuthUser>) -> String {
    if let Some(user) = current_user {
        return AUTH_USER_TEMPLATE.replace("{{USERNAME}}", &escape_html(&user.username));
    }

    AUTH_GUEST_TEMPLATE.to_string()
}

fn render_package_cards(packages: &[PackageSummary], registry_url: &str) -> String {
    if packages.is_empty() {
        return "<p class=\"muted\">No packages yet.</p>".to_string();
    }

    let mut html = String::new();
    for pkg in packages {
        let install_cmd = format!(
            "vpm install {} --registry {}",
            pkg.name,
            normalize_registry(registry_url)
        );
        let package_link = format!("/packages/{}", pkg.name);
        let card = PACKAGE_CARD_TEMPLATE
            .replace("{{NAME}}", &escape_html(&pkg.name))
            .replace("{{VERSION}}", &escape_html(&pkg.version))
            .replace("{{AUTHOR}}", &escape_html(&pkg.author))
            .replace("{{DESCRIPTION}}", &escape_html(&pkg.description))
            .replace("{{DOWNLOADS}}", &pkg.downloads.to_string())
            .replace("{{PACKAGE_LINK}}", &escape_html(&package_link))
            .replace("{{INSTALL_CMD}}", &escape_html(&install_cmd));
        html.push_str(&card);
    }

    html
}

fn render_package_detail_page(
    name: &str,
    selected: &PackageVersion,
    versions: &[PackageVersion],
    registry_url: &str,
) -> String {
    let install_latest = format!(
        "vpm install {} --registry {}",
        name,
        normalize_registry(registry_url)
    );
    let install_selected = format!(
        "vpm install {} --version {} --registry {}",
        name,
        selected.version,
        normalize_registry(registry_url)
    );

    let total_downloads: i64 = versions.iter().map(|v| v.downloads).sum();
    let readme_html = render_readme_html(&selected.readme);
    let version_items = render_version_items(name, versions, registry_url);
    let tarball_link = if selected.tarball_url.trim().is_empty() {
        "<span class=\"muted\">No tarball URL</span>".to_string()
    } else {
        format!(
            "<a href=\"{}\" target=\"_blank\" rel=\"noopener\">Tarball</a>",
            escape_html(&selected.tarball_url)
        )
    };
    let github_url = normalize_repo_url(&selected.github_repo);
    let github_link = if github_url.is_empty() {
        "<span class=\"muted\">No repo URL</span>".to_string()
    } else {
        format!(
            "<a href=\"{}\" target=\"_blank\" rel=\"noopener\">Source Repo</a>",
            escape_html(&github_url)
        )
    };
    let server_file_tools = if let Some(file_name) =
        uploaded_file_name_from_tarball_url(&selected.tarball_url, registry_url)
    {
        let inspect_url = format!("/uploads/{}/inspect", file_name);
        let raw_url = format!("/uploads/{}", file_name);
        format!(
            "<a href=\"{}\">Inspect Server File</a> <a href=\"{}\" target=\"_blank\" rel=\"noopener\">Raw Download</a>",
            escape_html(&inspect_url),
            escape_html(&raw_url),
        )
    } else {
        "<span class=\"muted\">No uploaded server file for this version.</span>".to_string()
    };

    PACKAGE_DETAIL_TEMPLATE
        .replace("{{NAME}}", &escape_html(name))
        .replace("{{VERSION}}", &escape_html(&selected.version))
        .replace("{{DESCRIPTION}}", &escape_html(&selected.description))
        .replace("{{AUTHOR}}", &escape_html(&selected.author))
        .replace("{{CREATED_AT}}", &escape_html(&selected.created_at))
        .replace("{{DOWNLOADS}}", &total_downloads.to_string())
        .replace("{{VERSION_DOWNLOADS}}", &selected.downloads.to_string())
        .replace("{{README_HTML}}", &readme_html)
        .replace("{{VERSION_ITEMS}}", &version_items)
        .replace("{{INSTALL_LATEST_CMD}}", &escape_html(&install_latest))
        .replace("{{INSTALL_SELECTED_CMD}}", &escape_html(&install_selected))
        .replace("{{TARBALL_LINK}}", &tarball_link)
        .replace("{{GITHUB_LINK}}", &github_link)
        .replace("{{SERVER_FILE_TOOLS}}", &server_file_tools)
}

fn render_version_items(name: &str, versions: &[PackageVersion], registry_url: &str) -> String {
    let mut html = String::new();
    for version in versions {
        let version_link = format!("/packages/{}/{}", name, version.version);
        let install_cmd = format!(
            "vpm install {} --version {} --registry {}",
            name,
            version.version,
            normalize_registry(registry_url)
        );
        let item = VERSION_ITEM_TEMPLATE
            .replace("{{VERSION_LINK}}", &escape_html(&version_link))
            .replace("{{VERSION}}", &escape_html(&version.version))
            .replace("{{CREATED_AT}}", &escape_html(&version.created_at))
            .replace("{{DOWNLOADS}}", &version.downloads.to_string())
            .replace("{{INSTALL_CMD}}", &escape_html(&install_cmd));
        html.push_str(&item);
    }
    html
}

fn render_readme_html(readme: &str) -> String {
    if readme.trim().is_empty() {
        return "<p class=\"muted\">No README published for this version.</p>".to_string();
    }

    format!("<pre class=\"readme\">{}</pre>", escape_html(readme))
}

fn normalize_repo_url(input: &str) -> String {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("git+") {
        return rest.to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("github:") {
        return format!("https://github.com/{}", rest.trim_start_matches('/'));
    }
    trimmed.to_string()
}

fn uploaded_file_name_from_tarball_url(tarball_url: &str, registry_url: &str) -> Option<String> {
    let trimmed = tarball_url
        .trim()
        .split('?')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("");
    if trimmed.is_empty() {
        return None;
    }

    let registry_prefix = format!("{}/uploads/", normalize_registry(registry_url));
    if let Some(rest) = trimmed.strip_prefix(&registry_prefix) {
        return sanitize_filename(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("/uploads/") {
        return sanitize_filename(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("uploads/") {
        return sanitize_filename(rest);
    }

    if let Some((_, rest)) = trimmed.rsplit_once("/uploads/") {
        return sanitize_filename(rest);
    }

    None
}

fn build_upload_preview<'a>(file_name: &'a str, bytes: &[u8]) -> (&'a str, String) {
    const MAX_TEXT_CHARS: usize = 32_000;
    const MAX_HEX_BYTES: usize = 2048;
    const BYTES_PER_LINE: usize = 16;

    if is_text_like_file_name(file_name)
        && let Ok(text) = std::str::from_utf8(bytes)
    {
        let mut preview = text.chars().take(MAX_TEXT_CHARS).collect::<String>();
        if text.chars().count() > MAX_TEXT_CHARS {
            preview.push_str("\n\n...truncated...");
        }
        return ("text preview", preview);
    }

    let preview_len = bytes.len().min(MAX_HEX_BYTES);
    let mut out = String::new();
    for (i, chunk) in bytes[..preview_len].chunks(BYTES_PER_LINE).enumerate() {
        let offset = i * BYTES_PER_LINE;
        out.push_str(&format!("{offset:08x}: "));
        for b in chunk {
            out.push_str(&format!("{b:02x} "));
        }
        out.push('\n');
    }
    if bytes.len() > preview_len {
        out.push_str(&format!("\n...truncated, showing first {preview_len} bytes...\n"));
    }

    ("hex preview", out)
}

fn is_text_like_file_name(file_name: &str) -> bool {
    matches!(
        Path::new(file_name).extension().and_then(|ext| ext.to_str()),
        Some(
            "txt"
                | "md"
                | "markdown"
                | "json"
                | "toml"
                | "yaml"
                | "yml"
                | "void"
                | "js"
                | "mjs"
                | "cjs"
                | "ts"
                | "tsx"
                | "jsx"
                | "css"
                | "html"
                | "xml"
                | "csv"
        )
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn normalize_registry(input: &str) -> &str {
    input.trim_end_matches('/')
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or_default() {
        "tgz" => "application/gzip",
        "gz" => "application/gzip",
        "zip" => "application/zip",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

fn sanitize_filename(input: &str) -> Option<String> {
    let base = Path::new(input).file_name()?.to_str()?.trim();
    if base.is_empty() {
        return None;
    }

    let mut out = String::new();
    for ch in base.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' || ch == '.' {
            out.push('-');
        }
    }
    if out.is_empty() {
        "pkg".to_string()
    } else {
        out
    }
}

fn internal_error(err: sqlx::Error) -> (StatusCode, String) {
    log_error("db", format!("internal database error: {}", err));
    (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {err}"))
}

fn absolute_path(input: &str) -> Result<PathBuf, std::io::Error> {
    let candidate = PathBuf::from(input);
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    Ok(std::env::current_dir()?.join(candidate))
}

fn default_public_url_from_addr(addr: &str) -> String {
    if let Some((host, port)) = addr.rsplit_once(':') {
        if host == "0.0.0.0" {
            return format!("http://127.0.0.1:{port}");
        }
    }
    format!("http://{addr}")
}
