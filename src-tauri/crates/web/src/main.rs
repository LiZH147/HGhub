//! Headless HPP host for the Web build: the same dispatcher as Tauri IPC.
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum::middleware::{self, Next};
use axum_server::tls_rustls::RustlsConfig;
use hghub_hpp::{ipc, PluginManager};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tower_http::services::ServeDir;

struct Context {
    manager: Arc<PluginManager>,
    token: String,
    public_https: bool,
}

#[derive(Deserialize)]
struct RpcRequest {
    action: String,
    #[serde(default)]
    params: Value,
}

/// Hashed assets never change; the entry document must always revalidate after a deploy.
async fn cache_control(request: axum::extract::Request, next: Next) -> Response {
    let shared = request.uri().path().starts_with("/assets/");
    let mut response = next.run(request).await;
    let value = if shared {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, header::HeaderValue::from_static(value));
    response
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

/// Browsers load media with the cookie, so the API accepts either the bearer token or that cookie.
fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(format!("Bearer {token}").as_str());
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|raw| {
            raw.split(';')
                .filter_map(|part| part.trim().split_once('='))
                .any(|(name, value)| name == "hg_token" && value == token)
        });
    bearer || cookie
}

async fn login(State(context): State<Arc<Context>>, Query(query): Query<HashMap<String, String>>) -> Response {
    if query.get("token").map(String::as_str) != Some(context.token.as_str()) {
        return (StatusCode::UNAUTHORIZED, "令牌无效").into_response();
    }
    let secure = context.public_https;
    let cookie = format!(
        "hg_token={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000{}",
        context.token,
        if secure { "; Secure" } else { "" }
    );
    (
        [
            (header::SET_COOKIE, cookie),
            (header::LOCATION, "/".to_string()),
        ],
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

async fn rpc(
    State(context): State<Arc<Context>>,
    headers: HeaderMap,
    Json(request): Json<RpcRequest>,
) -> impl IntoResponse {
    if !authorized(&headers, &context.token) {
        let body = json!({
            "ok": false,
            "error": { "code": "UNAUTHORIZED", "message": "缺少有效的访问令牌" }
        });
        return (StatusCode::UNAUTHORIZED, Json(body));
    }
    let body = match ipc::dispatch(&context.manager, request.action, request.params).await {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    (StatusCode::OK, Json(body))
}

#[tokio::main]
async fn main() {
    let data_dir = std::env::var("HGHUB_DATA_DIR").expect("HGHUB_DATA_DIR 未设置");
    let token = std::env::var("HGHUB_TOKEN")
        .ok()
        .filter(|value| value.len() >= 16)
        .expect("HGHUB_TOKEN 未设置或短于 16 个字符");
    // Browsers cannot reach the host loopback, so the media prefix must be the public origin.
    let public_base = std::env::var("HGHUB_PUBLIC_BASE").expect("HGHUB_PUBLIC_BASE 未设置");
    let web_root = std::env::var("HGHUB_WEB_ROOT").ok();
    let bind: SocketAddr = std::env::var("HGHUB_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8788".into())
        .parse()
        .expect("HGHUB_BIND 无效");
    let manager = Arc::new(
        PluginManager::open(PathBuf::from(data_dir)).expect("无法初始化插件宿主"),
    );
    assert!(
        manager.sessions.use_public_base(&public_base),
        "HGHUB_PUBLIC_BASE 必须在首次播放前设置"
    );
    // Initialize enabled plugins before serving: a session records the provider revision,
    // and a prepare that races the first initialization would record a stale one.
    manager.initialize_enabled().await;
    let router = Router::new()
        .route("/healthz", get(health))
        .route("/login", get(login))
        .route("/rpc", post(rpc))
        .with_state(Arc::new(Context {
            manager: manager.clone(),
            token,
            public_https: public_base.starts_with("https://"),
        }))
        .merge(hghub_hpp::media::router(&manager));
    let router = match web_root {
        Some(root) => router.fallback_service(ServeDir::new(root).append_index_html_on_directories(true)),
        None => router,
    }
    .layer(middleware::from_fn(cache_control));
    // Media is served directly over TLS; a plain listener stays available for the proxy origin.
    if let (Ok(cert), Ok(key)) = (
        std::env::var("HGHUB_TLS_CERT"),
        std::env::var("HGHUB_TLS_KEY"),
    ) {
        // rustls needs an explicit default provider when more than one crypto backend is in the graph.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let config = RustlsConfig::from_pem_file(cert, key)
            .await
            .expect("证书读取失败");
        if let Ok(plain_bind) = std::env::var("HGHUB_PLAIN_BIND") {
            let plain_router = router.clone();
            tokio::spawn(async move {
                let plain = tokio::net::TcpListener::bind(&plain_bind)
                    .await
                    .expect("HTTP 监听失败");
                axum::serve(plain, plain_router)
                    .await
                    .expect("HTTP 服务异常退出");
            });
        }
        println!("hghub-web listening https on {bind}, media base {public_base}");
        axum_server::bind_rustls(bind, config)
            .serve(router.into_make_service())
            .await
            .expect("服务异常退出");
        return;
    }
    let listener = tokio::net::TcpListener::bind(bind).await.expect("无法监听");
    println!("hghub-web listening on {bind}, media base {public_base}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("服务异常退出");
}
