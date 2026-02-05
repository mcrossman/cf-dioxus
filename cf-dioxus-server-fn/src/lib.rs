use std::sync::{Arc, LazyLock};

use axum::{http, routing, Extension};
use tower_service::Service as _;
use worker::{console_error, console_log, event, Context, Env};

mod cookies;
pub use cookies::{RequestCookies, ResponseCookies, ResponseCookie};

mod wasm_workaround {
    extern "C" {
        pub(super) fn __wasm_call_ctors();
    }
}

#[event(start)]
fn start() {
    console_error_panic_hook::set_once();

    // See https://github.com/rustwasm/wasm-bindgen/issues/4446
    unsafe { wasm_workaround::__wasm_call_ctors() };

    // Explicitly register server functions
    server_fn::axum::register_explicit::<cf_dioxus::server_function::Multiply>();

    // Force generation of the Router
    LazyLock::force(&ROUTER);
}

#[event(fetch)]
async fn fetch(
    mut req: http::Request<worker::Body>,
    env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<axum::body::Body>> {
    // Extract request cookies and pass as extension
    let cookie_header = req.headers().get(http::header::COOKIE)
        .and_then(|h| h.to_str().ok());
    let request_cookies = RequestCookies::from_cookie_header(cookie_header);
    req.extensions_mut().insert(request_cookies);

    // Create a shared mutable container for response cookies
    let response_cookies: Arc<tokio::sync::Mutex<ResponseCookies>> =
        Arc::new(tokio::sync::Mutex::new(ResponseCookies::default()));
    req.extensions_mut().insert(Arc::clone(&response_cookies));

    // Insert the environment
    req.extensions_mut().insert(env);

    // Handle the request
    let mut response = ROUTER.clone().call(req).await?;

    // Extract response cookies and add them to the response headers
    let cookies = response_cookies.lock().await;
    for set_cookie_value in cookies.into_headers() {
        response.headers_mut().append(
            http::header::SET_COOKIE,
            set_cookie_value.parse().unwrap_or_else(|_| http::HeaderValue::from_static("")),
        );
    }

    Ok(response)
}

static ROUTER: LazyLock<axum::Router> = LazyLock::new(|| {
    let mut router = axum::Router::new();
    // Iterate through the registered server functions, adding each to the Axum router.
    for (path, method) in server_fn::axum::server_fn_paths() {
        console_log!("Adding {method} {path} to router");
        match routing::MethodFilter::try_from(method) {
            Ok(method_filter) => {
                router = router.route(
                    path,
                    routing::on(method_filter, |req| handle_server_fn_with_cookies(req)),
                );
            }
            Err(err) => {
                console_error!("Unsupported server function HTTP method: {err:?}");
            }
        }
    }
    // After all server function routes are handled any other calls to `/api/*`
    // return Not Found. All other routes return a static asset or `index.html`.
    router
        .nest(
            "/api",
            axum::Router::new().fallback(async || http::StatusCode::NOT_FOUND),
        )
        .fallback(static_asset_or_index_html)
});

// Handlers that extract `Extension<Env>` require `#[worker::send]`.
// See https://github.com/cloudflare/workers-rs/issues/485
#[worker::send]
async fn static_asset_or_index_html(
    uri: http::Uri,
    Extension(env): Extension<Env>,
) -> Result<http::Response<axum::body::Body>, (http::StatusCode, String)> {
    // Usually static resources will be returned without invoking the
    // worker. However, non-browser requests may invoke the worker.
    // This performs the `single-page-application` behavior of returning
    // the named static asset or, if not found, the `index.html` file.
    let response = env
        .assets("ASSETS")
        .map_err(|e| (http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .fetch(uri.to_string(), None)
        .await
        .map_err(|e| (http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Wrap the `worker::Body` in an `axum::body::Body`.
    Ok(response.map(axum::body::Body::new))
}

/// Handle server function with cookie support
#[worker::send]
async fn handle_server_fn_with_cookies(
    req: http::Request<axum::body::Body>,
) -> http::Response<axum::body::Body> {
    server_fn::axum::handle_server_fn(req).await
}
