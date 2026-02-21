# Web Search Gateway

Unified interface for web search across multiple providers using ModKit's **Gateway + Plugin** architecture. Provides seamless multi-provider support with intelligent routing, caching, quota management, and rate limiting.

## Overview

Web Search Gateway is a **Gateway Module** that exposes a unified search API while delegating to pluggable **Provider Plugins** (Tavily, Bing, Google, Serper, etc.). Consumers interact through a single interface without knowing which provider handles their request.

> **Plugin Isolation Rule:** Regular modules **cannot** depend on or consume plugin modules directly. All search functionality must be accessed through the Gateway's public API (`hub.get::<dyn WebSearchGatewayClient>()`). This ensures plugin implementations remain swappable, isolated, and decoupled from consumers.

**Key Architecture:**
- **Gateway Module** (`web_search_gateway-gw`) — Public API, routing, caching, quota, fallback
- **Plugin Modules** (`tavily_plugin`, `bing_plugin`, etc.) — Provider-specific implementations
- **SDK Crate** (`web_search_gateway-sdk`) — Shared traits (`WebSearchGatewayClient`, `WebSearchPluginClient`), models, GTS schemas

---

## Documentation

See [PRD](docs/PRD.md) for detailed specifications, architecture diagrams, and implementation roadmap.
