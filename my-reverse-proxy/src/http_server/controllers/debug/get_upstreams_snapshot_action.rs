use my_http_server::{macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

#[http_route(
    method: "GET",
    route: "/api/debug/upstreams-snapshot",
    summary: "Upstream pools / connections diagnostic snapshot",
    description: "Returns the exact same data as the get_proxy_state_snapshot MCP tool: every upstream pool across the 6 registries (h1/h2 × tcp/tls/uds) with per-entry connection state (dead / rented / last_success / idle_secs), per-pool last_status (what the proxy believes about the upstream — 'ok'/'error'/'unknown'), per-pool live_disposables (on-demand connections in flight, h1 only), the process-wide on-demand budget (live_disposables_global / max_disposables_global), all configured locations, and orphan/naked correlation. Use this REST endpoint when MCP is unreachable to inspect, per upstream, which connections are pooled vs on-demand and whether the proxy has detected the upstream as down.",
    controller: "Debug",
    result:[
        {status_code: 200, description: "Proxy state snapshot (JSON)"},
    ]
)]
pub struct GetUpstreamsSnapshotAction;

async fn handle_request(
    _action: &GetUpstreamsSnapshotAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let snapshot = crate::mcp::build_proxy_state_snapshot().await;
    HttpOutput::as_json(snapshot).into_ok_result(true).into()
}
