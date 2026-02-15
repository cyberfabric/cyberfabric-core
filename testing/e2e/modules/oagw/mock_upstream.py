"""Standalone mock upstream HTTP server for OAGW E2E tests.

Provides endpoints that simulate an upstream service (OpenAI-compatible JSON,
SSE streaming, echo, configurable errors). Started as a session-scoped pytest
fixture so the OAGW service under test can proxy to it.
"""
import asyncio
import json
import time

from aiohttp import web


# ---------------------------------------------------------------------------
# Handlers
# ---------------------------------------------------------------------------

async def handle_health(request: web.Request) -> web.Response:
    return web.json_response({"status": "ok"})


async def handle_echo(request: web.Request) -> web.Response:
    """Return received headers and body as JSON."""
    body = await request.read()
    headers = {k.lower(): v for k, v in request.headers.items()}
    return web.json_response({
        "headers": headers,
        "body": body.decode("utf-8", errors="replace"),
    })


async def handle_chat_completions(request: web.Request) -> web.Response:
    """OpenAI-compatible chat completion response."""
    return web.json_response({
        "id": "chatcmpl-mock-123",
        "object": "chat.completion",
        "created": 1_234_567_890,
        "model": "gpt-4-mock",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello from mock server"},
            "finish_reason": "stop",
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30},
    })


async def handle_models(request: web.Request) -> web.Response:
    """OpenAI-compatible model list."""
    return web.json_response({
        "object": "list",
        "data": [
            {"id": "gpt-4", "object": "model", "created": 1_234_567_890, "owned_by": "openai"},
            {"id": "gpt-3.5-turbo", "object": "model", "created": 1_234_567_890, "owned_by": "openai"},
        ],
    })


async def handle_stream_chat_completions(request: web.Request) -> web.StreamResponse:
    """SSE streaming chat completion."""
    resp = web.StreamResponse(
        status=200,
        headers={"Content-Type": "text/event-stream", "Cache-Control": "no-cache"},
    )
    await resp.prepare(request)

    words = ["Hello", " from", " mock", " server"]
    for i, word in enumerate(words):
        delta = {}
        if i == 0:
            delta["role"] = "assistant"
        delta["content"] = word
        chunk = {
            "id": "chatcmpl-mock-stream",
            "object": "chat.completion.chunk",
            "created": 1_234_567_890,
            "model": "gpt-4-mock",
            "choices": [{"index": 0, "delta": delta, "finish_reason": None}],
        }
        await resp.write(f"data: {json.dumps(chunk)}\n\n".encode())
        await asyncio.sleep(0.01)

    # Final chunk with finish_reason
    final = {
        "id": "chatcmpl-mock-stream",
        "object": "chat.completion.chunk",
        "created": 1_234_567_890,
        "model": "gpt-4-mock",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
    }
    await resp.write(f"data: {json.dumps(final)}\n\n".encode())
    await resp.write(b"data: [DONE]\n\n")
    await resp.write_eof()
    return resp


async def handle_error_code(request: web.Request) -> web.Response:
    """Return a configurable HTTP error status."""
    code = int(request.match_info["code"])
    return web.json_response(
        {"error": {"message": f"Simulated error {code}", "type": "server_error", "code": f"error_{code}"}},
        status=code,
    )


async def handle_error_timeout(request: web.Request) -> web.Response:
    """Sleep long enough for the gateway proxy timeout to fire."""
    await asyncio.sleep(30)
    return web.Response(text="should not reach here")


async def handle_status(request: web.Request) -> web.Response:
    """Return configurable status code."""
    code = int(request.match_info["code"])
    return web.json_response({"status": code, "description": f"Status {code}"}, status=code)


# ---------------------------------------------------------------------------
# App factory
# ---------------------------------------------------------------------------

def create_app() -> web.Application:
    app = web.Application()
    app.router.add_get("/health", handle_health)
    app.router.add_post("/echo", handle_echo)
    app.router.add_post("/v1/chat/completions", handle_chat_completions)
    app.router.add_get("/v1/models", handle_models)
    app.router.add_post("/v1/chat/completions/stream", handle_stream_chat_completions)
    app.router.add_get("/error/{code}", handle_error_code)
    app.router.add_get("/error/timeout", handle_error_timeout)
    app.router.add_get("/status/{code}", handle_status)
    return app


# ---------------------------------------------------------------------------
# Server lifecycle (used by conftest.py fixture)
# ---------------------------------------------------------------------------

class MockUpstreamServer:
    """Manages the mock upstream lifecycle for pytest fixtures."""

    def __init__(self, host: str = "0.0.0.0", port: int = 19876):
        self.host = host
        self.port = port
        self._runner: web.AppRunner | None = None

    async def start(self) -> None:
        app = create_app()
        self._runner = web.AppRunner(app)
        await self._runner.setup()
        site = web.TCPSite(self._runner, self.host, self.port)
        await site.start()

    async def stop(self) -> None:
        if self._runner:
            await self._runner.cleanup()
            self._runner = None

    @property
    def base_url(self) -> str:
        return f"http://127.0.0.1:{self.port}"


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="OAGW mock upstream server")
    parser.add_argument("--port", type=int, default=19876)
    parser.add_argument("--host", default="0.0.0.0")
    args = parser.parse_args()

    app = create_app()
    web.run_app(app, host=args.host, port=args.port)
