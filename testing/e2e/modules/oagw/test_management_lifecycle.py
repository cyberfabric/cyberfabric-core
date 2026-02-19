"""E2E tests for OAGW Management API lifecycle (upstream + route CRUD)."""
import httpx
import pytest

from .helpers import create_route, create_upstream, delete_upstream, unique_alias


@pytest.mark.asyncio
async def test_create_minimal_upstream_returns_201(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """POST /oagw/v1/upstreams with minimal fields returns 201."""
    _ = mock_upstream
    alias = unique_alias("mgmt-create")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )

        assert "id" in upstream
        assert upstream["id"].startswith("gts.x.core.oagw.upstream.v1~")
        assert upstream.get("enabled") is True
        assert upstream.get("alias") == alias

        # Cleanup.
        await delete_upstream(client, oagw_base_url, oagw_headers, upstream["id"])


@pytest.mark.asyncio
async def test_get_upstream_by_id(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """GET /oagw/v1/upstreams/{id} returns the created upstream."""
    _ = mock_upstream
    alias = unique_alias("mgmt-get")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]

        resp = await client.get(
            f"{oagw_base_url}/oagw/v1/upstreams/{uid}",
            headers=oagw_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["id"] == uid
        assert data["alias"] == alias

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_list_upstreams_includes_created(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """GET /oagw/v1/upstreams list includes the created upstream."""
    _ = mock_upstream
    alias = unique_alias("mgmt-list")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]

        resp = await client.get(
            f"{oagw_base_url}/oagw/v1/upstreams",
            headers=oagw_headers,
        )
        assert resp.status_code == 200
        items = resp.json()
        assert isinstance(items, list)
        assert any(u["id"] == uid for u in items)

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_update_upstream_alias(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """PATCH /oagw/v1/upstreams/{id} updates the alias."""
    _ = mock_upstream
    alias = unique_alias("mgmt-upd")
    new_alias = unique_alias("mgmt-upd-v2")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]

        resp = await client.patch(
            f"{oagw_base_url}/oagw/v1/upstreams/{uid}",
            headers={**oagw_headers, "content-type": "application/json"},
            json={"alias": new_alias},
        )
        assert resp.status_code == 200

        resp = await client.get(
            f"{oagw_base_url}/oagw/v1/upstreams/{uid}",
            headers=oagw_headers,
        )
        assert resp.json()["alias"] == new_alias

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_delete_upstream_returns_204(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """DELETE /oagw/v1/upstreams/{id} returns 204 and resource is gone."""
    _ = mock_upstream
    alias = unique_alias("mgmt-del")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]

        resp = await delete_upstream(client, oagw_base_url, oagw_headers, uid)
        assert resp.status_code == 204

        resp = await client.get(
            f"{oagw_base_url}/oagw/v1/upstreams/{uid}",
            headers=oagw_headers,
        )
        assert resp.status_code == 404


# ---------------------------------------------------------------------------
# Route lifecycle
# ---------------------------------------------------------------------------

@pytest.mark.asyncio
async def test_create_route_returns_201(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """POST /oagw/v1/routes returns 201 with GTS id."""
    _ = mock_upstream
    alias = unique_alias("mgmt-rte")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]

        route = await create_route(
            client, oagw_base_url, oagw_headers, uid, ["POST"], "/v1/chat/completions",
        )
        assert "id" in route
        assert route["id"].startswith("gts.x.core.oagw.route.v1~")

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_delete_upstream_cascades_routes(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Deleting an upstream cascades to its routes; proxy returns 404 gateway."""
    _ = mock_upstream
    alias = unique_alias("mgmt-cascade")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["GET"], "/test",
        )

        resp = await delete_upstream(client, oagw_base_url, oagw_headers, uid)
        assert resp.status_code == 204

        # Proxy to deleted alias should return 404 from gateway.
        resp = await client.get(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/test",
            headers=oagw_headers,
        )
        assert resp.status_code == 404
        assert resp.headers.get("x-oagw-error-source") == "gateway"


# ---------------------------------------------------------------------------
# Tags
# ---------------------------------------------------------------------------

@pytest.mark.asyncio
async def test_create_upstream_with_tags(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Upstream created with tags includes them in the response."""
    _ = mock_upstream
    alias = unique_alias("mgmt-tags")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url,
            alias=alias, tags=["openai", "llm"],
        )
        assert "openai" in upstream.get("tags", [])
        assert "llm" in upstream.get("tags", [])

        await delete_upstream(client, oagw_base_url, oagw_headers, upstream["id"])
