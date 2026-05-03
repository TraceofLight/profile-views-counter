use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    allowed_usernames: Arc<Vec<String>>,
    ip_salt: Arc<String>,
}

#[derive(Deserialize, Debug)]
struct ViewParams {
    username: String,
    #[serde(default = "default_label")]
    label: String,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default)]
    style: Style,
    #[serde(default, deserialize_with = "deserialize_bool_lenient")]
    abbreviated: bool,
    #[serde(default)]
    base: i64,
}

fn default_label() -> String {
    "views".to_string()
}
fn default_color() -> String {
    "555555".to_string()
}

#[derive(Deserialize, Debug, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum Style {
    #[default]
    Flat,
    FlatSquare,
    Plastic,
    ForTheBadge,
}

fn deserialize_bool_lenient<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    Ok(matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
}

async fn views_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<ViewParams>,
) -> impl IntoResponse {
    if !valid_username(&p.username) {
        return error_response("invalid username");
    }

    let username_lower = p.username.to_ascii_lowercase();
    if !state.allowed_usernames.iter().any(|u| u == &username_lower) {
        return error_response("not allowed");
    }

    let ip = extract_client_ip(&headers);
    let ip_hash = hash_ip(&state.ip_salt, &ip);

    let claimed: Option<i64> = match sqlx::query_scalar(
        "INSERT INTO view_events (username, ip_hash) VALUES (?, ?) \
         ON CONFLICT(username, ip_hash) DO UPDATE SET last_seen = CURRENT_TIMESTAMP \
         WHERE view_events.last_seen < datetime('now', '-1 hour') \
         RETURNING 1",
    )
    .bind(&p.username)
    .bind(&ip_hash)
    .fetch_optional(&state.db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "dedup query failed");
            return error_response("db error");
        }
    };

    let count: i64 = if claimed.is_some() {
        match sqlx::query_scalar(
            "INSERT INTO views (username, count) VALUES (?, 1) \
             ON CONFLICT(username) DO UPDATE SET count = count + 1, updated_at = CURRENT_TIMESTAMP \
             RETURNING count",
        )
        .bind(&p.username)
        .fetch_one(&state.db)
        .await
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "failed to increment counter");
                return error_response("db error");
            }
        }
    } else {
        sqlx::query_scalar("SELECT count FROM views WHERE username = ?")
            .bind(&p.username)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or(0)
    };

    let total = count.saturating_add(p.base);
    let display = format_count(total, p.abbreviated);
    let label = sanitize_text(&p.label, 32);
    let color = sanitize_color(&p.color);
    let svg = render_badge(&label, &display, &color, p.style);

    badge_response(svg)
}

fn extract_client_ip(headers: &HeaderMap) -> String {
    for header in ["cf-connecting-ip", "x-real-ip"] {
        if let Some(v) = headers.get(header).and_then(|v| v.to_str().ok()) {
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "unknown".to_string()
}

fn hash_ip(salt: &str, ip: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(ip.as_bytes());
    let bytes = hasher.finalize();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

async fn health() -> &'static str {
    "ok"
}

fn badge_response(svg: String) -> axum::response::Response {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
            (
                header::CACHE_CONTROL,
                "public, max-age=300, s-maxage=300",
            ),
        ],
        svg,
    )
        .into_response()
}

fn error_response(msg: &str) -> axum::response::Response {
    let svg = render_badge("error", msg, "e05d44", Style::Flat);
    badge_response(svg)
}

fn valid_username(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn sanitize_text(s: &str, max_len: usize) -> String {
    let trimmed: String = s.chars().take(max_len).collect();
    trimmed
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn sanitize_color(c: &str) -> String {
    let stripped = c.trim_start_matches('#');
    if (stripped.len() == 3 || stripped.len() == 6 || stripped.len() == 8)
        && stripped.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return stripped.to_string();
    }
    match c.to_ascii_lowercase().as_str() {
        "brightgreen" => "4c1".into(),
        "green" => "97ca00".into(),
        "yellow" => "dfb317".into(),
        "yellowgreen" => "a4a61d".into(),
        "orange" => "fe7d37".into(),
        "red" => "e05d44".into(),
        "blue" => "007ec6".into(),
        "lightgrey" | "lightgray" => "9f9f9f".into(),
        "grey" | "gray" => "555555".into(),
        _ => "555555".into(),
    }
}

fn format_count(n: i64, abbreviated: bool) -> String {
    if !abbreviated {
        let s = n.abs().to_string();
        let mut out = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                out.insert(0, ',');
            }
            out.insert(0, c);
        }
        if n < 0 {
            out.insert(0, '-');
        }
        return out;
    }
    let abs = n.unsigned_abs() as f64;
    let formatted = if abs < 1_000.0 {
        format!("{}", n)
    } else if abs < 1_000_000.0 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else if abs < 1_000_000_000.0 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    };
    formatted.replace(".0", "")
}

fn estimate_width(text: &str, char_w: f32, padding: u32) -> u32 {
    (text.chars().count() as f32 * char_w).ceil() as u32 + padding * 2
}

fn render_badge(label: &str, value: &str, color: &str, style: Style) -> String {
    match style {
        Style::ForTheBadge => render_for_the_badge(label, value, color),
        Style::Plastic => render_plastic(label, value, color),
        Style::FlatSquare => render_flat(label, value, color, false),
        Style::Flat => render_flat(label, value, color, true),
    }
}

fn render_flat(label: &str, value: &str, color: &str, rounded: bool) -> String {
    let label_w = estimate_width(label, 6.5, 6);
    let value_w = estimate_width(value, 7.0, 6);
    let total = label_w + value_w;
    let label_cx = label_w / 2;
    let value_cx = label_w + value_w / 2;
    let rx = if rounded { 3 } else { 0 };

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="20" role="img" aria-label="{label}: {value}">
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="{total}" height="20" rx="{rx}" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_w}" height="20" fill="#555"/>
    <rect x="{label_w}" width="{value_w}" height="20" fill="#{color}"/>
    <rect width="{total}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11">
    <text x="{label_cx}" y="15" fill="#010101" fill-opacity=".3">{label}</text>
    <text x="{label_cx}" y="14">{label}</text>
    <text x="{value_cx}" y="15" fill="#010101" fill-opacity=".3">{value}</text>
    <text x="{value_cx}" y="14">{value}</text>
  </g>
</svg>"##
    )
}

fn render_plastic(label: &str, value: &str, color: &str) -> String {
    let label_w = estimate_width(label, 6.5, 6);
    let value_w = estimate_width(value, 7.0, 6);
    let total = label_w + value_w;
    let label_cx = label_w / 2;
    let value_cx = label_w + value_w / 2;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="18" role="img" aria-label="{label}: {value}">
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#fff" stop-opacity=".7"/>
    <stop offset=".1" stop-color="#aaa" stop-opacity=".1"/>
    <stop offset=".9" stop-opacity=".3"/>
    <stop offset="1" stop-opacity=".5"/>
  </linearGradient>
  <clipPath id="r"><rect width="{total}" height="18" rx="4" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_w}" height="18" fill="#555"/>
    <rect x="{label_w}" width="{value_w}" height="18" fill="#{color}"/>
    <rect width="{total}" height="18" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11">
    <text x="{label_cx}" y="14" fill="#010101" fill-opacity=".3">{label}</text>
    <text x="{label_cx}" y="13">{label}</text>
    <text x="{value_cx}" y="14" fill="#010101" fill-opacity=".3">{value}</text>
    <text x="{value_cx}" y="13">{value}</text>
  </g>
</svg>"##
    )
}

fn render_for_the_badge(label: &str, value: &str, color: &str) -> String {
    let label_upper = label.to_uppercase();
    let value_upper = value.to_uppercase();
    let label_w = estimate_width(&label_upper, 9.5, 12);
    let value_w = estimate_width(&value_upper, 9.5, 12);
    let total = label_w + value_w;
    let label_cx = label_w / 2;
    let value_cx = label_w + value_w / 2;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="28" role="img" aria-label="{label_upper}: {value_upper}">
  <g shape-rendering="crispEdges">
    <rect width="{label_w}" height="28" fill="#555"/>
    <rect x="{label_w}" width="{value_w}" height="28" fill="#{color}"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="10" letter-spacing="1.5">
    <text x="{label_cx}" y="18">{label_upper}</text>
    <text x="{value_cx}" y="18" font-weight="bold">{value_upper}</text>
  </g>
</svg>"##
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:counter.db?mode=rwc".to_string());

    let opts = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let allowed_usernames: Vec<String> = std::env::var("ALLOWED_USERNAMES")
        .unwrap_or_else(|_| "TraceofLight".to_string())
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if allowed_usernames.is_empty() {
        tracing::warn!("ALLOWED_USERNAMES is empty; all requests will be rejected");
    } else {
        tracing::info!(?allowed_usernames, "username allowlist active");
    }

    let ip_salt = std::env::var("IP_HASH_SALT").unwrap_or_else(|_| {
        tracing::warn!("IP_HASH_SALT not set; using insecure default");
        "default-insecure-salt".to_string()
    });

    let state = AppState {
        db,
        allowed_usernames: Arc::new(allowed_usernames),
        ip_salt: Arc::new(ip_salt),
    };

    let app = Router::new()
        .route("/", get(views_handler))
        .route("/api/v1/views-counter", get(views_handler))
        .route("/health", get(health))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
