# tokio-console Integration

[tokio-console](https://github.com/tokio-rs/console) provides deep async runtime inspection for debugging and profiling tokio-based applications.

The integration is **feature-gated** behind the `tokio-console` Cargo feature and is **disabled by default** in production builds.

## Building with tokio-console

```bash
cargo build -p hyperspot-server --features tokio-console
```

For a release build with console support (e.g. preprod):

```bash
cargo build -p hyperspot-server --release --features tokio-console
```

## Configuration

Add a `tokio_console` section to your YAML config file:

```yaml
tokio_console:
  server_addr: "127.0.0.1:6669"   # default
```

The `server_addr` field controls the bind address for the console-subscriber gRPC server.
Default is `127.0.0.1:6669` (localhost only).

Configuration can also be set via environment variables:

```bash
APP__TOKIO_CONSOLE__SERVER_ADDR=127.0.0.1:6669
```

### Enabling at runtime

The console layer is only activated when **both** conditions are met:

1. The binary was built with `--features tokio-console`.
2. The `tokio_console` section is present in the config file.

If the feature is compiled in but no `tokio_console` config section exists, the layer is not started.

## Connecting with tokio-console

### Local development

```bash
tokio-console http://localhost:6669
```

### Kubernetes (via port-forward)

```bash
kubectl port-forward pod/<pod-name> 6669:6669
tokio-console http://localhost:6669
```

The default `127.0.0.1` binding is compatible with `kubectl port-forward` — no `0.0.0.0` binding is needed.

## Disabling

- **Build time**: omit `--features tokio-console` (the default). The `console-subscriber` crate is not compiled at all.
- **Runtime**: remove or omit the `tokio_console` section from the config file.

## Security considerations

- Never expose the console gRPC port to untrusted networks.
- The default localhost binding ensures the port is only reachable via port-forward or local access.
- Do not enable this feature in production builds unless actively debugging.
