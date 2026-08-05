# Persistent generic Studio service

The `rusty-studio.service` user unit serves one generic Rusty Engine Studio independently of
`den-serve`. It is rolling-development infrastructure for a trusted machine, not an exact downstream
consumer certification path. Root-local `.rusty-studio.json` files continue to select project-owned
adapters; the service does not acquire project or gameplay meaning.

## Install and operate

From the Engine checkout:

```bash
pnpm --dir studio run service -- install
systemctl --user status rusty-studio.service
curl http://127.0.0.1:4310/health
journalctl --user -u rusty-studio.service -f
```

Installation creates immutable commit-keyed releases under `/home/system/rusty-studio/releases`,
keeps the selected release in the atomic `current` symlink, and writes configuration only when
`/home/system/rusty-studio/config/service.env` does not exist. A restart admits the current committed
Engine revision without fetching the network:

```bash
systemctl --user restart rusty-studio.service
```

The service builds a clean archive of `HEAD`, installs the locked Studio workspace, builds it, and
starts a temporary loopback host whose `/health` identity must match that commit before promotion.
Failed candidate installation, build, or smoke leaves `current` unchanged. Runtime health reports
the exact rolling Engine source commit but never claims an exact downstream consumer certification.

## Update and rollback

```bash
pnpm --dir studio run service -- update
pnpm --dir studio run service -- rollback
```

Update refuses a dirty checkout, fetches its configured upstream, permits only a fast-forward,
builds and smokes the immutable candidate, then restarts and health-checks the service. A failed
post-restart health check restores the previous release. Rollback swaps `current` and `previous`,
restarts, and verifies health.

To uninstall the unit while preserving releases, settings, and operator configuration:

```bash
pnpm --dir studio run service -- uninstall
```

Remove `/home/system/rusty-studio` separately only when its settings and releases are no longer
needed.

## Binding and security

The generated configuration binds `127.0.0.1:4310` by default. Studio can read explicitly selected
host directories and project resources and can start commands declared by trusted root-local
`.rusty-studio.json` files. Do not expose it to an untrusted network. A trusted-LAN deployment may
set `RUSTY_STUDIO_HOST=0.0.0.0` deliberately in `service.env`, but the operator then owns network
access control and must treat the endpoint as host-file and process-launch authority.

`pnpm run serve:den` remains the managed exact-consumer certification workflow. This service never
replaces or weakens its immutable consumer identity checks.
