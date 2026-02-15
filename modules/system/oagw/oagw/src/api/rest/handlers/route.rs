use axum::Json;
use axum::extract::{Extension, Path, Query};
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use modkit_security::SecurityContext;
use crate::domain::gts_helpers as gts;
use crate::domain::dto::Route;
use crate::api::rest::dto::{CreateRouteRequest, RouteResponse, UpdateRouteRequest};

use crate::api::rest::error::domain_error_response;
use crate::api::rest::extractors::{PaginationQuery, parse_gts_id};
use crate::module::AppState;

fn to_response(r: Route) -> RouteResponse {
    RouteResponse {
        id: gts::format_route_gts(r.id),
        tenant_id: r.tenant_id,
        upstream_id: r.upstream_id,
        match_rules: r.match_rules.into(),
        plugins: r.plugins.map(Into::into),
        rate_limit: r.rate_limit.map(Into::into),
        tags: r.tags,
        priority: r.priority,
        enabled: r.enabled,
    }
}

pub async fn create_route(
    Extension(state): Extension<AppState>,
    Extension(ctx): Extension<SecurityContext>,
    Json(req): Json<CreateRouteRequest>,
) -> Result<impl IntoResponse, Response> {
    let route = state
        .cp
        .create_route(&ctx, req.into())
        .await
        .map_err(|e| domain_error_response(e, "/oagw/v1/routes"))?;
    Ok((StatusCode::CREATED, Json(to_response(route))))
}

pub async fn get_route(
    Extension(state): Extension<AppState>,
    Extension(ctx): Extension<SecurityContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, Response> {
    let instance = format!("/oagw/v1/routes/{id}");
    let uuid = parse_gts_id(&id, &instance)?;
    let route = state
        .cp
        .get_route(&ctx, uuid)
        .await
        .map_err(|e| domain_error_response(e, &instance))?;
    Ok(Json(to_response(route)))
}

pub async fn list_routes(
    Extension(state): Extension<AppState>,
    Extension(ctx): Extension<SecurityContext>,
    Path(upstream_id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, Response> {
    let instance = format!("/oagw/v1/upstreams/{upstream_id}/routes");
    let upstream_uuid = parse_gts_id(&upstream_id, &instance)?;
    let query = pagination.to_list_query();
    let routes = state
        .cp
        .list_routes(&ctx, upstream_uuid, &query)
        .await
        .map_err(|e| domain_error_response(e, &instance))?;
    let response: Vec<RouteResponse> = routes.into_iter().map(to_response).collect();
    Ok(Json(response))
}

pub async fn update_route(
    Extension(state): Extension<AppState>,
    Extension(ctx): Extension<SecurityContext>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRouteRequest>,
) -> Result<impl IntoResponse, Response> {
    let instance = format!("/oagw/v1/routes/{id}");
    let uuid = parse_gts_id(&id, &instance)?;
    let route = state
        .cp
        .update_route(&ctx, uuid, req.into())
        .await
        .map_err(|e| domain_error_response(e, &instance))?;
    Ok(Json(to_response(route)))
}

pub async fn delete_route(
    Extension(state): Extension<AppState>,
    Extension(ctx): Extension<SecurityContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, Response> {
    let instance = format!("/oagw/v1/routes/{id}");
    let uuid = parse_gts_id(&id, &instance)?;
    state
        .cp
        .delete_route(&ctx, uuid)
        .await
        .map_err(|e| domain_error_response(e, &instance))?;
    Ok(StatusCode::NO_CONTENT)
}
