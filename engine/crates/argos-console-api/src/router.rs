//! Public router — Argos Console exposes a *complete* HTTP surface that any
//! container can mount as a peer to other axum routers.  In the v1.0
//! bundled binary, `argos-server` brings up this router alongside the
//! engine's ES-compat / native routers via `Router::merge` so they
//! share one TCP listener.  When Argos Console is extracted post-v1.0 it ships
//! with this same router as the binary's main entry point.
//!
//! Surface owned by this crate:
//!
//! ```text
//! /_argos-console                 redirect → /_argos-console/
//! /_argos-console/                bundled SPA index.html
//! /_argos-console/setup           bundled setup.html (magic-link claim)
//! /_argos-console/login           bundled login.html (returning user)
//! /_argos-console/{path*}         bundled SPA assets (CSS, JS, images, fonts)
//! /_argos-console/api/v1/...      API endpoints (auth, prefs, dashboards, …)
//! ```
//!
//! ## Why one router, not two?
//!
//! Mounting the SPA assets and the API as one Router ensures Argos Console
//! owns the full `/_argos-console/*` namespace exclusively — no risk that the
//! engine's ES-compat router accidentally serves a 404 page over an
//! asset request, or vice versa.  The engine layer enforces its own
//! `Authorization: Bearer` policy on its routes only; this router
//! enforces its session-cookie policy on its routes only.

use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::{
    auth, cluster, dashboards, data_sources, graph, knowledge, prefs, spa, state::ConsoleState,
    views,
};

/// Build the full Argos Console router. Mount at the root of an axum Router
/// (it owns the `/_argos-console/*` prefix internally):
///
/// ```ignore
/// let merged = engine_router.merge(argos_console_api::argos_console_router(state));
/// axum::serve(listener, merged).await?;
/// ```
pub fn argos_console_router(state: ConsoleState) -> Router {
    Router::new()
        // ── Bundled SPA assets (index, setup, login, css, js, fonts) ───────
        .route("/_argos-console", get(spa::argos_console_redirect))
        .route("/_argos-console/", get(spa::argos_console_index))
        .route("/_argos-console/*rest", get(spa::argos_console_asset))
        // ── API: cluster awareness ─────────────────────────────────────────
        .route("/_argos-console/api/v1/cluster/info", get(cluster::info))
        // ── API: auth bootstrap (unauthenticated) ──────────────────────────
        .route(
            "/_argos-console/api/v1/auth/magic/redeem",
            post(auth::magic::redeem),
        )
        // ── API: invite issue (owner/admin passkey session required) ───────
        // A bearer API token is 403. See `PasskeySession`.
        .route(
            "/_argos-console/api/v1/auth/magic/issue",
            post(auth::magic::issue),
        )
        .route(
            "/_argos-console/api/v1/auth/passkey/begin",
            post(auth::passkey::begin),
        )
        .route(
            "/_argos-console/api/v1/auth/passkey/finish",
            post(auth::passkey::finish),
        )
        .route(
            "/_argos-console/api/v1/auth/login/begin",
            post(auth::login::begin),
        )
        .route(
            "/_argos-console/api/v1/auth/login/finish",
            post(auth::login::finish),
        )
        // ── API: session-protected ─────────────────────────────────────────
        // `PasskeySession` (bearer token → 403): logout, passkey delete,
        // api-token create and revoke. List routes and every data route
        // below accept a bearer token. See `auth::sessions::PasskeySession`.
        .route(
            "/_argos-console/api/v1/auth/logout",
            post(auth::login::logout),
        )
        .route("/_argos-console/api/v1/me", get(auth::me::me))
        .route(
            "/_argos-console/api/v1/auth/passkeys",
            get(auth::me::list_passkeys),
        )
        .route(
            "/_argos-console/api/v1/auth/passkeys/:id",
            delete(auth::me::delete_passkey),
        )
        .route(
            "/_argos-console/api/v1/auth/api-tokens",
            get(auth::tokens::list).post(auth::tokens::create),
        )
        .route(
            "/_argos-console/api/v1/auth/api-tokens/:id",
            delete(auth::tokens::revoke),
        )
        .route(
            "/_argos-console/api/v1/prefs",
            get(prefs::get).put(prefs::put),
        )
        .route(
            "/_argos-console/api/v1/dashboards",
            get(dashboards::list).post(dashboards::create),
        )
        // Static `_bulk` sits beside the `:id` param route below; matchit
        // gives the static segment priority, so a POST to `_bulk` never
        // lands on the `:id` handler.
        .route(
            "/_argos-console/api/v1/dashboards/_bulk",
            post(dashboards::bulk),
        )
        .route(
            "/_argos-console/api/v1/dashboards/:id",
            get(dashboards::get_one)
                .put(dashboards::replace)
                .delete(dashboards::delete),
        )
        .route(
            "/_argos-console/api/v1/dashboards/:id",
            patch(dashboards::patch),
        )
        .route(
            "/_argos-console/api/v1/views",
            get(views::list).post(views::create),
        )
        .route(
            "/_argos-console/api/v1/views/:id",
            get(views::get_one).delete(views::delete),
        )
        .route(
            "/_argos-console/api/v1/data-sources/connections",
            get(data_sources::list_connections),
        )
        .route(
            "/_argos-console/api/v1/data-sources/connections/:id/indices",
            get(data_sources::list_indices),
        )
        .route(
            "/_argos-console/api/v1/data-sources/connections/:id/indices/:name/fields",
            get(data_sources::list_fields),
        )
        .route(
            "/_argos-console/api/v1/data-sources/connections/:id/indices/:name/search",
            post(data_sources::search),
        )
        // ── API: the knowledge surface (session-authorized) ─
        // One read that answers what a person asks the moment indexing
        // finishes: how large, which data, what can I do now. See
        // src/knowledge.rs.
        .route("/_argos-console/api/v1/knowledge", get(knowledge::knowledge))
        // ── API: Second-Brain graph reads (session-authorized, issue #936) ─
        // The data plane's /_graph/* routes stay credential-only; these are
        // the console's own role-gated read paths over the same engine
        // traversals. See src/graph.rs for the authorization rule.
        .route("/_argos-console/api/v1/graph/brains", get(graph::brains))
        .route("/_argos-console/api/v1/graph/:brain/ego", get(graph::ego))
        .route(
            "/_argos-console/api/v1/graph/:brain/overview",
            get(graph::overview),
        )
        .route(
            "/_argos-console/api/v1/graph/:brain/edges/_search",
            post(graph::edges_search),
        )
        .route(
            "/_argos-console/api/v1/graph/:brain/nodes/_search",
            post(graph::nodes_search),
        )
        .with_state(state)
}

/// Routes this crate currently exposes. Used by tests so an accidental
/// drop is caught at CI.
#[doc(hidden)]
pub fn known_routes() -> &'static [&'static str] {
    &[
        "/_argos-console",
        "/_argos-console/",
        "/_argos-console/*rest",
        "/_argos-console/api/v1/cluster/info",
        "/_argos-console/api/v1/auth/magic/redeem",
        "/_argos-console/api/v1/auth/magic/issue",
        "/_argos-console/api/v1/auth/passkey/begin",
        "/_argos-console/api/v1/auth/passkey/finish",
        "/_argos-console/api/v1/auth/login/begin",
        "/_argos-console/api/v1/auth/login/finish",
        "/_argos-console/api/v1/auth/logout",
        "/_argos-console/api/v1/me",
        "/_argos-console/api/v1/auth/passkeys",
        "/_argos-console/api/v1/auth/passkeys/:id",
        "/_argos-console/api/v1/auth/api-tokens",
        "/_argos-console/api/v1/auth/api-tokens/:id",
        "/_argos-console/api/v1/prefs",
        "/_argos-console/api/v1/dashboards",
        "/_argos-console/api/v1/dashboards/_bulk",
        "/_argos-console/api/v1/dashboards/:id",
        "/_argos-console/api/v1/views",
        "/_argos-console/api/v1/views/:id",
        "/_argos-console/api/v1/data-sources/connections",
        "/_argos-console/api/v1/data-sources/connections/:id/indices",
        "/_argos-console/api/v1/data-sources/connections/:id/indices/:name/fields",
        "/_argos-console/api/v1/data-sources/connections/:id/indices/:name/search",
        "/_argos-console/api/v1/knowledge",
        "/_argos-console/api/v1/graph/brains",
        "/_argos-console/api/v1/graph/:brain/ego",
        "/_argos-console/api/v1/graph/:brain/overview",
        "/_argos-console/api/v1/graph/:brain/edges/_search",
        "/_argos-console/api/v1/graph/:brain/nodes/_search",
    ]
}
