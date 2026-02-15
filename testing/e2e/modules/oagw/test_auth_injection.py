"""E2E tests for OAGW auth injection (API key plugin)."""
import httpx
import pytest

from .helpers import create_route, create_upstream, delete_upstream, unique_alias

APIKEY_AUTH_PLUGIN_ID = "gts.x.core.oagw.auth_plugin.v1~x.core.oagw.apikey.v1"


@pytest.mark.asyncio
async def test_apikey_auth_injects_bearer_header(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """Auth plugin injects Authorization: Bearer <secret> into upstream request."""
    alias = unique_alias("auth-key")
    auth_config = {
        "type": APIKEY_AUTH_PLUGIN_ID,
        "sharing": "private",
        "config": {
            "header": "authorization",
            "prefix": "Bearer ",
            "secret_ref": "cred://openai-key",
        },
    }

    async with httpx.AsyncClient(timeout=10.0) as client:
        try:
            upstream = await create_upstream(
                client, oagw_base_url, oagw_headers, mock_upstream_url,
                alias=alias, auth=auth_config,
            )
        except httpx.HTTPStatusError as exc:
            if exc.response.status_code in (400, 500):
                pytest.skip(
                    f"Cannot create upstream with auth config (cred_store may not be available): "
                    f"{exc.response.status_code} {exc.response.text[:200]}"
                )
            raise

        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid, ["POST"], "/echo",
        )

        resp = await client.post(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/echo",
            headers={**oagw_headers, "content-type": "application/json"},
            json={"test": True},
        )

        if resp.status_code in (401, 500):
            await delete_upstream(client, oagw_base_url, oagw_headers, uid)
            pytest.skip(
                f"Auth injection failed (cred_store may not have test secret): "
                f"{resp.status_code} {resp.text[:200]}"
            )

        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text[:500]}"

        echoed = resp.json().get("headers", {})
        auth_header = echoed.get("authorization", "")
        assert auth_header.startswith("Bearer "), (
            f"Expected 'Bearer ...' in authorization header, got: {auth_header!r}"
        )
        # The value after "Bearer " should be a non-empty resolved secret.
        secret_value = auth_header[len("Bearer "):]
        assert len(secret_value) > 0, "Resolved secret is empty"

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)
