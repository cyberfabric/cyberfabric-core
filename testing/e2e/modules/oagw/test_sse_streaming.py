"""E2E tests for OAGW SSE streaming proxy."""
import json

import httpx
import pytest

from .helpers import create_route, create_upstream, delete_upstream, unique_alias


@pytest.mark.asyncio
async def test_sse_proxy_content_type_and_done(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """SSE proxy returns text/event-stream and ends with data: [DONE]."""
    alias = unique_alias("sse-ct")
    async with httpx.AsyncClient(timeout=15.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid,
            ["POST"], "/v1/chat/completions/stream",
        )

        resp = await client.post(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/chat/completions/stream",
            headers={**oagw_headers, "content-type": "application/json"},
            json={"model": "gpt-4", "stream": True},
        )
        assert resp.status_code == 200
        ct = resp.headers.get("content-type", "")
        assert "text/event-stream" in ct, f"Expected text/event-stream, got: {ct}"

        body = resp.text
        assert "data: [DONE]" in body, f"SSE stream missing 'data: [DONE]'. Body: {body[:500]}"

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)


@pytest.mark.asyncio
async def test_sse_proxy_contains_json_chunks(
    oagw_base_url, oagw_headers, mock_upstream_url, mock_upstream,
):
    """SSE data lines (except [DONE]) contain valid JSON with choices."""
    alias = unique_alias("sse-json")
    async with httpx.AsyncClient(timeout=15.0) as client:
        upstream = await create_upstream(
            client, oagw_base_url, oagw_headers, mock_upstream_url, alias=alias,
        )
        uid = upstream["id"]
        await create_route(
            client, oagw_base_url, oagw_headers, uid,
            ["POST"], "/v1/chat/completions/stream",
        )

        resp = await client.post(
            f"{oagw_base_url}/oagw/v1/proxy/{alias}/v1/chat/completions/stream",
            headers={**oagw_headers, "content-type": "application/json"},
            json={"model": "gpt-4", "stream": True},
        )
        assert resp.status_code == 200

        data_lines = [
            line[len("data: "):]
            for line in resp.text.splitlines()
            if line.startswith("data: ") and line.strip() != "data: [DONE]"
        ]
        assert len(data_lines) > 0, "No SSE data lines found"

        for dl in data_lines:
            chunk = json.loads(dl)
            assert "choices" in chunk, f"SSE chunk missing 'choices': {chunk}"

        await delete_upstream(client, oagw_base_url, oagw_headers, uid)
