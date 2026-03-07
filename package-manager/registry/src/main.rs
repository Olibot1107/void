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
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{FromRow, Row, SqlitePool};
use uuid::Uuid;

const SESSION_COOKIE: &str = "void_session";
const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;
const INDEX_TEMPLATE: &str = include_str!("../templates/index.html");
const AUTH_GUEST_TEMPLATE: &str = include_str!("../templates/auth_guest.html");
const AUTH_USER_TEMPLATE: &str = include_str!("../templates/auth_user.html");
const PACKAGE_CARD_TEMPLATE: &str = include_str!("../templates/package_card.html");

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
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct PackageSummary {
    name: String,
    version: String,
    description: String,
    author: String,
    created_at: String,
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
    let addr = env::var("VOID_REGISTRY_ADDR").unwrap_or_else(|_| "127.0.0.1:4090".to_string());
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

    let db_url = format!("sqlite://{}", db_abs_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await?;

    init_db(&pool).await?;

    let state = AppState {
        db: pool,
        upload_dir,
        public_base_url,
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/uploads/{file}", get(upload_file_handler))
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/publish", post(publish_form_handler))
        .route("/api/login", post(api_login_handler))
        .route("/api/publish", post(publish_api_handler))
        .route("/api/publish/upload", post(publish_upload_api_handler))
        .route("/api/packages", get(list_packages_handler))
        .route("/api/packages/{name}", get(get_package_handler))
        .route("/api/search", get(search_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Void registry running at http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
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
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn table_has_column(pool: &SqlitePool, table: &str, column: &str) -> Result<bool, sqlx::Error> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;

    for row in rows {
        let name: String = row.try_get("name")?;
        if name == column {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn index_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SearchQuery>,
) -> Result<Html<String>, (StatusCode, String)> {
    let current_user = auth_user_from_cookie(&state.db, &jar)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let packages = fetch_packages(&state.db, query.q.as_deref())
        .await
        .map_err(internal_error)?;

    let page = render_index_page(
        current_user.as_ref(),
        &packages,
        query.q.as_deref().unwrap_or(""),
        query.message.as_deref(),
    );

    Ok(Html(page))
}

async fn upload_file_handler(
    AxumPath(file): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let safe_name = match sanitize_filename(&file) {
        Some(v) => v,
        None => {
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
            let content_type = content_type_for_path(&file_path);
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            );
            (StatusCode::OK, headers, bytes).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found".to_string()).into_response(),
    }
}

async fn register_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(payload): Form<RegisterForm>,
) -> impl IntoResponse {
    if validate_username(&payload.username).is_err() {
        return (jar, Redirect::to("/?message=register_invalid")).into_response();
    }

    if payload.password.len() < 8 {
        return (jar, Redirect::to("/?message=password_short")).into_response();
    }

    let password_hash = match hash_password(&payload.password) {
        Ok(v) => v,
        Err(_) => return (jar, Redirect::to("/?message=register_failed")).into_response(),
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
                return (jar, Redirect::to("/?message=user_exists")).into_response();
            }
            return (jar, Redirect::to("/?message=register_failed")).into_response();
        }
    };

    let token = match create_session(&state.db, inserted.last_insert_rowid()).await {
        Ok(v) => v,
        Err(_) => return (jar, Redirect::to("/?message=register_failed")).into_response(),
    };

    let jar = jar.add(session_cookie(&token));
    (jar, Redirect::to("/?message=registered")).into_response()
}

async fn login_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(payload): Form<LoginForm>,
) -> impl IntoResponse {
    let user = match find_user_by_username(&state.db, payload.username.trim()).await {
        Ok(Some(v)) => v,
        Ok(None) => return (jar, Redirect::to("/?message=bad_credentials")).into_response(),
        Err(_) => return (jar, Redirect::to("/?message=login_failed")).into_response(),
    };

    if !verify_password(&payload.password, &user.password_hash) {
        return (jar, Redirect::to("/?message=bad_credentials")).into_response();
    }

    let token = match create_session(&state.db, user.id).await {
        Ok(v) => v,
        Err(_) => return (jar, Redirect::to("/?message=login_failed")).into_response(),
    };

    let jar = jar.add(session_cookie(&token));
    (jar, Redirect::to("/?message=logged_in")).into_response()
}

async fn logout_handler(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        let _ = sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(cookie.value())
            .execute(&state.db)
            .await;
    }

    let jar = jar.remove(Cookie::build((SESSION_COOKIE, "")).path("/").build());
    (jar, Redirect::to("/?message=logged_out")).into_response()
}

async fn publish_form_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    multipart: Multipart,
) -> impl IntoResponse {
    let user = match auth_user_from_cookie(&state.db, &jar).await {
        Ok(Some(v)) => v,
        Ok(None) => return (jar, Redirect::to("/?message=login_required")).into_response(),
        Err(_) => return (jar, Redirect::to("/?message=publish_failed")).into_response(),
    };

    let draft = match parse_publish_multipart(multipart).await {
        Ok(v) => v,
        Err(_) => return (jar, Redirect::to("/?message=publish_failed")).into_response(),
    };

    let payload = match finalize_publish_draft(&state, draft) {
        Ok(v) => v,
        Err(_) => return (jar, Redirect::to("/?message=publish_failed")).into_response(),
    };

    match insert_package(&state.db, payload, &user).await {
        Ok(()) => (jar, Redirect::to("/?message=published")).into_response(),
        Err(_) => (jar, Redirect::to("/?message=publish_failed")).into_response(),
    }
}

async fn api_login_handler(
    State(state): State<AppState>,
    Json(payload): Json<ApiLoginRequest>,
) -> impl IntoResponse {
    let user = match find_user_by_username(&state.db, payload.username.trim()).await {
        Ok(Some(v)) => v,
        Ok(None) => {
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
        Ok(token) => (
            StatusCode::OK,
            Json(ApiLoginResponse {
                ok: true,
                message: "Login successful".to_string(),
                token: Some(token),
                username: Some(user.username),
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiLoginResponse {
                ok: false,
                message: "Could not create session".to_string(),
                token: None,
                username: None,
            }),
        )
            .into_response(),
    }
}

async fn publish_api_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<PublishRequest>,
) -> impl IntoResponse {
    let user = match auth_user_for_api(&state.db, &jar, &headers).await {
        Ok(Some(v)) => v,
        Ok(None) => {
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

    match validate_publish_request(payload) {
        Ok(valid) => match insert_package(&state.db, valid, &user).await {
            Ok(()) => (
                StatusCode::CREATED,
                Json(ApiMessage {
                    ok: true,
                    message: "Package published".to_string(),
                }),
            )
                .into_response(),
            Err(err) => (
                StatusCode::BAD_REQUEST,
                Json(ApiMessage {
                    ok: false,
                    message: err,
                }),
            )
                .into_response(),
        },
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

async fn publish_upload_api_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    multipart: Multipart,
) -> impl IntoResponse {
    let user = match auth_user_for_api(&state.db, &jar, &headers).await {
        Ok(Some(v)) => v,
        Ok(None) => {
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

    match insert_package(&state.db, payload, &user).await {
        Ok(()) => (
            StatusCode::CREATED,
            Json(ApiMessage {
                ok: true,
                message: "Package published".to_string(),
            }),
        )
            .into_response(),
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
    let packages = fetch_packages(&state.db, None).await.map_err(internal_error)?;
    Ok(Json(packages))
}

async fn get_package_handler(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<Vec<PackageVersion>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, PackageVersion>(
        r#"
        SELECT name, version, description, author, tarball_url, github_repo, readme, created_at
        FROM packages
        WHERE name = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(name)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(rows))
}

async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<PackageSummary>>, (StatusCode, String)> {
    let rows = fetch_packages(&state.db, params.q.as_deref())
        .await
        .map_err(internal_error)?;

    Ok(Json(rows))
}

async fn fetch_packages(pool: &SqlitePool, query: Option<&str>) -> Result<Vec<PackageSummary>, sqlx::Error> {
    if let Some(q) = query
        && !q.trim().is_empty()
    {
        let like = format!("%{}%", q.trim());
        return sqlx::query_as::<_, PackageSummary>(
            r#"
            SELECT p.name, p.version, p.description, p.author, p.created_at
            FROM packages p
            INNER JOIN (
                SELECT name, MAX(created_at) AS max_created
                FROM packages
                GROUP BY name
            ) latest
            ON p.name = latest.name AND p.created_at = latest.max_created
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

    sqlx::query_as::<_, PackageSummary>(
        r#"
        SELECT p.name, p.version, p.description, p.author, p.created_at
        FROM packages p
        INNER JOIN (
            SELECT name, MAX(created_at) AS max_created
            FROM packages
            GROUP BY name
        ) latest
        ON p.name = latest.name AND p.created_at = latest.max_created
        ORDER BY p.created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn auth_user_from_cookie(pool: &SqlitePool, jar: &CookieJar) -> Result<Option<AuthUser>, String> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        return auth_user_from_token(pool, cookie.value())
            .await
            .map_err(|e| format!("Database error: {e}"));
    }
    Ok(None)
}

async fn auth_user_for_api(
    pool: &SqlitePool,
    jar: &CookieJar,
    headers: &HeaderMap,
) -> Result<Option<AuthUser>, String> {
    if let Some(user) = auth_user_from_cookie(pool, jar).await? {
        return Ok(Some(user));
    }

    if let Some(token) = extract_bearer_token(headers) {
        return auth_user_from_token(pool, &token)
            .await
            .map_err(|e| format!("Database error: {e}"));
    }

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

    Ok(row.map(|(id, username)| AuthUser { id, username }))
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
    let mut draft = PublishDraft::default();

    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let name = field.name().unwrap_or_default().to_string();

        if name == "package_file" {
            let original_name = field.file_name().unwrap_or("package.bin").to_string();
            let bytes = field.bytes().await.map_err(|e| e.to_string())?.to_vec();
            if !bytes.is_empty() {
                if bytes.len() > MAX_UPLOAD_BYTES {
                    return Err("Uploaded file is too large".to_string());
                }
                draft.uploaded_file = Some(UploadedFile {
                    file_name: original_name,
                    bytes,
                });
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
        return Err("name and version are required".to_string());
    }

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

    Ok(format!(
        "{}/uploads/{}",
        public_base.trim_end_matches('/'),
        file_name
    ))
}

async fn insert_package(pool: &SqlitePool, payload: PublishRequest, user: &AuthUser) -> Result<(), String> {
    let existing_owner = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM packages WHERE name = ? AND user_id IS NOT NULL LIMIT 1",
    )
    .bind(&payload.name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Database error: {e}"))?;

    if let Some(owner_id) = existing_owner
        && owner_id != user.id
    {
        return Err("Package name is already owned by another account".to_string());
    }

    let description = payload.description.unwrap_or_default();
    let tarball_url = payload.tarball_url.unwrap_or_default();
    let github_repo = payload.github_repo.unwrap_or_default();
    let readme = payload.readme.unwrap_or_default();
    let created_at = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO packages (name, version, description, author, tarball_url, github_repo, readme, user_id, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(payload.name)
    .bind(payload.version)
    .bind(description)
    .bind(&user.username)
    .bind(tarball_url)
    .bind(github_repo)
    .bind(readme)
    .bind(user.id)
    .bind(created_at)
    .execute(pool)
    .await
    .map_err(|e| format!("Database error: {e}"))?;

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
) -> String {
    let auth_section = render_auth_section(current_user);
    let package_cards = render_package_cards(packages);
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

fn render_package_cards(packages: &[PackageSummary]) -> String {
    if packages.is_empty() {
        return "<p class=\"muted\">No packages yet.</p>".to_string();
    }

    let mut html = String::new();
    for pkg in packages {
        let card = PACKAGE_CARD_TEMPLATE
            .replace("{{NAME}}", &escape_html(&pkg.name))
            .replace("{{VERSION}}", &escape_html(&pkg.version))
            .replace("{{AUTHOR}}", &escape_html(&pkg.author))
            .replace("{{DESCRIPTION}}", &escape_html(&pkg.description));
        html.push_str(&card);
    }

    html
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
