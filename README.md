# DS Authority Governance UI

A [Yew](https://yew.rs) single-page app: the governance console for an
Eclipse EDC IdentityHub Issuer Service. It gives a dataspace authority a
single place to manage credential definitions, approve holder requests, and
track credential/issuance lifecycle, alongside the shared DCP identity
bootstrap flow (participant context, `did:web` publishing, keypair
lifecycle) used across ds-labs UIs.

It talks to two backends through same-origin paths (see
`configuration.json` below): the hub's Issuer Admin API (`v1alpha`, EDC
shaped) and the trusted issuers registry. The wire contract is
`governance/api-contract.md`; the typed client is `src/api/`.

| Route | View | Status |
|---|---|---|
| `/` | Dashboard: counts (credential definitions, pending holders, issuance processes in flight, issued credentials active/revoked), a **Review pending approvals** link when holders wait, and this issuer's trust status in the registry per credential type | done |
| `/credential-definitions` | Credential Definitions: table, create/edit dialog with validation, delete with confirmation | done |
| `/holders` | Holders: **Pending approval** queue (Approve / Reject with confirmation), all holders with a status filter, Register holder dialog | done |
| `/credentials` | Credentials: issued credentials (Revoke with confirmation) and issuance processes; Refresh button, auto-refresh every 3 s while a delivery is in flight (APPROVED), every 30 s while only SUBMITTED processes wait for approval | done |
| `/trusted-issuers` | Trusted Issuers: registry table with role/status filters, add/edit dialog, Suspend/Activate/Revoke, Delete with confirmation, Check trust form showing the accreditation chain (re-run after every write, out-of-order answers dropped); read-only banner and disabled write buttons when `GET {registry}/v1/admin` is not 2xx | done |
| `/identity-bootstrap` | mounts `ds_identity_bootstrap_ui::ParticipantContextPanel` | placeholder - see below |

UX conventions shared by every view: one `h1` per page (the nav label),
spinners (`role="progressbar"`) while loading, an empty state for empty
tables, errors in a danger alert (`role="alert"`) carrying the backend's
`{"error": ...}` message, success confirmations as inline success alerts
(`role="status"`), timestamps rendered as UTC, long DIDs/ids truncated with
CSS (full value in the DOM and in `title`), dialogs named by their title.
Dialogs take focus on open (first field, or the confirm button), trap Tab,
close on Escape and give focus back to the button that opened them. Below
1200px the nav sidebar starts collapsed behind the masthead toggle and
closes after each nav click. Row actions track every in-flight id, and a
finished action only closes the dialog it opened itself.
The e2e suite (`e2e/`, Playwright) relies on these accessible names; its
README lists them.

Code layout:

| Path | What |
|---|---|
| `src/api/` | models, QuerySpec, request builders, error messages (host unit-tested), thin reqwest client |
| `src/format.rs`, `src/validation.rs` | display formatting and form validation (host unit-tested) |
| `src/components.rs` | shared accessible PatternFly building blocks (alerts, dialog, form fields, table helpers) |
| `src/hooks.rs` | `use_config`, `use_load` (load + silent reload), `use_polling` |
| `src/views/` | one module per route |

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
  "issuer_admin_api_path": "/api/issuer",
  "registry_api_path": "/api/registry",
  "participant_context_id": "issuer",
  "bearer_token": null
}
```

- `identity_api_path` and `issuer_admin_api_path` are **same-origin,
  relative paths** - never full URLs with a host. A reverse proxy in front
  of the built app (apisix, in this project's deployment) is expected to
  forward each path to the real identity-api / issuer-admin-api. That's a
  deliberate choice to avoid any CORS dependency, handled by a separate
  integration/deployment stage, not by this app.
- `registry_api_path` (optional, default `/api/registry`) is the same-origin
  path the gateway forwards to the trusted issuers registry
  (`ds-trusted-issuers-registry-rs`), used by the Trusted Issuers view and
  the Dashboard's trust status.
- `participant_context_id` (optional, default `issuer`) is the hub's
  participant context id. It is sent base64-encoded in every Issuer Admin
  API path (`/v1alpha/participants/{base64(id)}/...`), as EDC does.
- `bearer_token` is optional (string or `null`/absent). This app is served
  behind an OIDC gate (apisix + Zitadel) that sets a same-origin session
  cookie, which the browser sends automatically on every same-origin API
  call - that's the common case and needs nothing from this field.
  `bearer_token`, when present, adds an *additional*
  `Authorization: Bearer <token>` header (see `Config::authorize` in
  `src/config.rs`) for scripted/service access; it never replaces the
  cookie.

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
blocks (including one for `registry_api_path`) (or add your own) to forward those paths to services reachable from
your machine while developing.

Trunk `0.22.0-beta.2` is a pinned devtool, installed separately - it is not
a Cargo dependency of this crate.

Building the actual app for the browser:

```bash
trunk build --release
```

`cargo check --target wasm32-unknown-unknown` and `cargo test` (host unit
tests for the API layer, formatting, validation and config) are the fast
sanity checks during development, without needing Trunk installed at all.
CI-equivalent lint: `cargo clippy --target wasm32-unknown-unknown -- -D warnings`.

## End-to-end tests

`e2e/` holds a Playwright suite that drives the built console in Chromium
against a real `ds-identity-hub-rs` issuer service, a real trusted issuers
registry, a fake DCP holder and a gateway stand-in that serves `dist/` and
injects the backend admin tokens. It also defines the accessibility
contract (headings, button names, table columns, dialog titles, alert texts)
the views are written against. See [`e2e/README.md`](e2e/README.md):

```bash
trunk build
cd e2e && npm ci && REGISTRY_BIN=/path/to/trusted-issuers-registry npm test
```

The `e2e` GitHub workflow runs this suite on pull requests and on `main`,
against `ds-identity-hub-rs` and `ds-trusted-issuers-registry-rs` at `main`
(other refs via a manual run's inputs).
