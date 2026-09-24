# DS Authority Governance UI

A [Yew](https://yew.rs) single-page app: the governance console for an
Eclipse EDC IdentityHub Issuer Service. It gives a dataspace authority a
single place to manage credential definitions, approve holder requests, and
track credential/issuance lifecycle, alongside the shared DCP identity
bootstrap flow (participant context, `did:web` publishing, keypair
lifecycle) used across ds-labs UIs.

This repository currently holds the **app shell**: routing, runtime
configuration, and shared state. The 5 routes below are wired up and each
renders a one-line placeholder - the real views are filled in separately, as
follow-on work.

| Route | Component | Status |
|---|---|---|
| `/` | Dashboard | placeholder |
| `/credential-definitions` | Credential Definitions | placeholder |
| `/holders` | Holders approval queue | placeholder |
| `/credentials` | Credentials & Issuance | placeholder |
| `/identity-bootstrap` | mounts `ds_identity_bootstrap_ui::ParticipantContextPanel` | placeholder - see below |

Routing is **hash-based** (`yew_router::HashRouter`, URLs like
`/#/holders`), not history/browser mode, so the built `dist/` bundle can be
dropped on any static host or CDN behind a reverse proxy with zero
server-side fallback-rule configuration.

## `ds-identity-bootstrap-ui` is not wired in yet

`Cargo.toml` depends on `ds-identity-bootstrap-ui` (git, branch `main`), but
that crate currently only declares stub `mod` items in its `lib.rs` with no
component files behind them - it does not compile *at all* yet, even as an
unused dependency (Cargo builds every crate in the dependency graph
regardless of whether its symbols are imported). To keep this crate building
in the meantime without dropping the dependency, it's marked `optional`,
gated behind an off-by-default `identity-bootstrap-ui` feature (see
`Cargo.toml`) - the git ref itself is unchanged, and this is the one
deviation from the original build instructions (which asked to leave it a
plain mandatory dependency; that isn't possible while the upstream crate
fails to compile). `src/views/identity_bootstrap.rs` keeps the real
`use ds_identity_bootstrap_ui::ParticipantContextPanel;` import and
`<ParticipantContextPanel />` usage commented out (search that file for
`TODO: uncomment`), with the same one-line placeholder shown in the
meantime. The route and the nav entry are already in place; once that crate
ships real components, uncomment those two lines and drop `optional`/the
feature gate from `Cargo.toml`.

## `configuration.json`

Fetched at runtime from the app's own origin, same pattern as
[`ds-catalog-browser-ui`](https://github.com/ds-labs-org/ds-catalog-browser-ui)'s
`configuration.json`:

```json
{
  "identity_api_path": "/api/identity",
  "issuer_admin_api_path": "/api/issuer"
}
```

- `identity_api_path` and `issuer_admin_api_path` are **same-origin,
  relative paths** - never full URLs with a host. A reverse proxy in front
  of the built app (apisix, in this project's deployment) is expected to
  forward each path to the real identity-api / issuer-admin-api. That's a
  deliberate choice to avoid any CORS dependency, handled by a separate
  integration/deployment stage, not by this app.
- There is deliberately no API-key/bearer-token field here. EDC's
  identity-api/issuer-admin-api require their own `x-api-key` header
  underneath apisix's Zitadel OIDC gate; that key is injected server-side
  by apisix (`proxy-rewrite`, dslabs-infra's `roles/edc_issuer`) after the
  gate authenticates the human. `configuration.json` is served by this
  app's own unauthenticated static hosting, so shipping any such key here
  would leak it to anyone who loads the page, logged in or not - the
  browser never sees or sends it.

`src/config.rs` fetches and parses this file once at startup (before the
routed content renders - see `src/main.rs`), and the result is stored in the
shared `yewdux` store (`src/store.rs`) as `AppState::config`. Nothing
downstream of startup should re-fetch it: read it via
`yewdux::prelude::use_store::<AppState>()` instead.

## Running locally

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.22.0-beta.2 --locked
trunk serve
```

Then open `http://localhost:8080`. Without a proxy in front of it,
`configuration.json`'s `identity_api_path` / `issuer_admin_api_path` won't
resolve to real backends; edit `Trunk.toml`'s commented-out `[[proxy]]`
blocks (or add your own) to forward those paths to services reachable from
your machine while developing.

Trunk `0.22.0-beta.2` is a pinned devtool, installed separately - it is not
a Cargo dependency of this crate.

Building the actual app for the browser:

```bash
trunk build --release
```

`cargo check --target wasm32-unknown-unknown` (and `cargo test` for the
host-testable unit tests in `src/config.rs`) are the fast sanity checks
during development, without needing Trunk installed at all.
