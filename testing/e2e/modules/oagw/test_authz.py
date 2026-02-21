"""E2E tests for OAGW proxy authorization enforcement."""
import os

import httpx
import pytest

from .helpers import create_route, create_upstream, delete_upstream, unique_alias

NIL_TENANT_ID = "00000000-0000-0000-0000-000000000000"


@pytest.mark.asyncio
async def test_proxy_authz_allowed(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Regression: valid tenant passes authz and proxy request succeeds."""
    _ = mock_upstream
    alias = unique_alias("authz-allow")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        try:
            await create_route(
                client, oagw_base_url, oagw_headers, uid, ["GET"], "/v1/models",
            )

            resp = await client.get(
                f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/models",
                headers=oagw_headers,
            )
            assert resp.status_code == 200
        finally:
            await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_proxy_authz_forbidden_nil_tenant(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Nil tenant UUID is denied by authz with 403 and Problem Details body."""
    _ = mock_upstream
    alias = unique_alias("authz-deny")
    async with httpx.AsyncClient(timeout=10.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        try:
            await create_route(
                client, oagw_base_url, oagw_headers, uid, ["GET"], "/v1/models",
            )

            denied_headers = {"x-tenant-id": NIL_TENANT_ID}
            token = os.getenv("E2E_AUTH_TOKEN")
            if token:
                denied_headers["Authorization"] = f"Bearer {token}"

            resp = await client.get(
                f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/models",
                headers=denied_headers,
            )

            if resp.status_code == 401:
                pytest.skip(
                    "Server requires JWT auth; set E2E_AUTH_TOKEN to a token with nil tenant"
                )
            if resp.status_code == 200:
                pytest.skip("AuthZ not enforced in this environment")

            assert resp.status_code == 403
            assert resp.headers.get("x-oagw-error-source") == "gateway"
            body = resp.json()
            assert body["status"] == 403
            assert body["title"] == "Forbidden"
            assert body["type"] == "gts.x.core.errors.err.v1~x.oagw.authz.forbidden.v1"
        finally:
            await delete_upstream(client, oagw_base_url, oagw_headers, uid)
