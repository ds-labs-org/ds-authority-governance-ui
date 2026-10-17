# End-to-end tests

Playwright drives the trunk-built console in Chromium against real
backends. The contract they test is
`governance/api-contract.md` (section 5 describes this harness).

```
Chromium -> proxy.mjs (gateway stand-in) --/api/issuer/*----> hub admin listener (+ admin token)
              | serves dist/ + configuration.json --/api/registry/*--> registry (+ admin token)
fake-holder.mjs --DCP POST /credentials (Self-Issued ID Token)--> hub public API
hub --Storage API POST /<holder>/credentials--> fake-holder.mjs
```

| File | What it does |
|---|---|
| `harness/stack.mjs` | Starts hub (`issuer-service --holder-approval manual`, admin listener), registry, fake holder and proxy on free ports; writes `.stack/stack.json`; backend logs in `.stack/*.log`. Run alone with `npm run stack`. |
| `harness/fake-holder.mjs` | DCP holder: P-256 key (WebCrypto), `did:web:127.0.0.1%3A<port>:<name>` document at `/<name>/did.json`, ES256 Self-Issued ID Tokens, Credential Requests, `GET /requests/{id}`, Storage API sink. JSON control API under `/_control` for the tests. |
| `harness/proxy.mjs` | Serves `dist/`, a generated `configuration.json`, forwards `/api/issuer` and `/api/registry` (prefix stripped, admin bearer injected, browser `Authorization` dropped). |
| `harness/backend.mjs` | Direct backend calls for setup/assertions only, plus the status-list decoder. |
| `tests/*.spec.mjs` | The specs. One worker: the stack is shared, so every test uses unique ids/DIDs. |

## Running

```bash
# 1. backends
(cd ../ds-identity-hub-rs && cargo build --bin identity-hub)
(cd ../ds-trusted-issuers-registry-rs && cargo build -p trusted-issuers-http --bin trusted-issuers-registry)
# 2. the console
trunk build                     # -> dist/
# 3. tests
cd e2e && npm ci
REGISTRY_BIN=../../ds-trusted-issuers-registry-rs/target/debug/trusted-issuers-registry npm test
npx playwright test tests/holders.spec.mjs --headed   # one file, visible browser
```

Never run `playwright install`; Chromium comes from `PLAYWRIGHT_BROWSERS_PATH`
(`/opt/pw-browsers` in the dev container). `@playwright/test` is pinned to
1.56.1 to match it.

| Env | Default | |
|---|---|---|
| `HUB_BIN` | `<repo>/../ds-identity-hub-rs/target/debug/identity-hub` | hub binary |
| `REGISTRY_BIN` | none (required) | `trusted-issuers-registry` binary |
| `UI_DIST` | `../dist` | trunk output (relative to `e2e/`) |
| `HUB_URL`, `HUB_ADMIN_URL`, `HUB_ADMIN_TOKEN` | | use a running hub instead of spawning one |
| `REGISTRY_URL`, `REGISTRY_ADMIN_TOKEN` | | use a running registry instead (started with `--lote-operator-street`, `-locality` and `-country`, or the LoTE answers 503) |
| `HUB_EXTRA_ARGS` | | extra hub flags |
| `E2E_STACK_DIR` | `e2e/.stack` | state file and logs |

## Accessibility contract (what the UI must render)

The specs only use roles, accessible names and visible text. Names are
exact (case-sensitive) unless noted.

**Every page**
- Routes are hash-based: `#/`, `#/credential-definitions`, `#/holders`,
  `#/credentials`, `#/trusted-issuers`, `#/identity-bootstrap`.
- A `navigation` landmark with links **Dashboard**, **Credential
  Definitions**, **Holders**, **Credentials**, **Trusted Issuers**,
  **Identity Bootstrap**.
- Exactly one `h1`, equal to the page's nav label.
- While loading: a spinner with `role="progressbar"`; none left once data
  (or an error) is shown.
- Errors: an element with `role="alert"` whose text includes the backend's
  `{"error": ...}` message (or any text for network failures). Mutation
  failures use the title **Could not \<verb\> \<thing\>** listed below. Never
  fail silently, never show an empty list for a failed load.
- Tables have an accessible name (`aria-label` or `<caption>`), real
  `<th>` column headers, and one `<tr>` per item whose text contains the
  full id/DID (truncate with CSS, not in the DOM); on Trusted Issuers the
  row's first cell is exactly its DID (tests key rows on it). Row actions are plain
  buttons in the row (no kebab menus).
- Dialogs have `role="dialog"` **and an accessible name equal to their
  title**. Note: patternfly-yew 0.8.2's `Modal` sets
  `aria-labelledby="modal-title"` but renders the title with a different
  id, so it has no name; set `aria-label`/fix the id. Every dialog has a
  **Cancel** button; form fields have `<label for>`.
- Config load failure: alert **Could not load configuration.json**, no nav.

**Dashboard** - regions (`<section aria-label>`) **Credential
definitions**, **Pending holders**, **Issued credentials** (all records,
any status), each containing its count as a number; a link **Review
pending approvals** to `#/holders`.

**Credential Definitions** - table **Credential definitions**: ID,
Credential type, Format, Validity (seconds as a number). Button **Create
credential definition** -> dialog **Create credential definition** with
fields **ID**, **Credential type**, **Validity (seconds)**, button
**Create**; failure alert **Could not create credential definition**
(dialog stays open). Row **Edit** -> dialog **Edit credential definition**
(**ID** shown read-only, **Validity (seconds)**, **Save**). Row **Delete**
-> dialog **Delete credential definition** naming the id, button
**Delete**. Empty: **No credential definitions**.

**Holders** - table **Pending approval**: DID, Name, Requested; row
buttons **Approve** (immediate) and **Reject** -> dialog **Reject holder**
naming the DID, button **Reject**. Table **All holders**: DID, Name, Status
(`pending`/`approved`/`rejected`, any case). Button **Register holder** ->
dialog **Register holder** with **DID**, **Name**, button **Register**;
failure alert **Could not register holder**. Button **Refresh**. Empty:
**No pending approvals**, **No holders**.

**Credentials** - table **Issued credentials**: ID, Type, Holder (DID),
Issued, Expires, Status (`active`/`revoked`); row **Revoke** (absent or
disabled once revoked) -> dialog **Revoke credential**, button **Revoke**.
Table **Issuance processes**: ID (the full process id), Holder (DID),
Credential definitions, State (`SUBMITTED`, `APPROVED`, `DELIVERED`,
`ERRORED`, `REJECTED` verbatim). Button **Refresh**. Empty: **No issued
credentials**, **No issuance processes**.

**Trusted Issuers** - table **Trusted issuers**: Issuer id (a DID for DCP
issuers, an https credential issuer URL for OID4VCI ones), Name, Role
(**Root TAO** / **TAO** / **Trusted issuer**), Credential types, Accredited
by, Status (`active`/`suspended`/`revoked`). Selects **Filter by role**,
**Filter by status**, **Filter by identifier** (option values `did`, `url`).
Button **Add trusted issuer** -> dialog **Add trusted issuer** with **Issuer
id**, **Name**, **Role** (native `<select>`, option values `root_tao`, `tao`,
`ti`), **Credential types** (comma-separated), **Accredited by**, **X.509
trust anchors** (textarea, one base64 DER certificate per line), button
**Save**; failure alert **Could not save trusted issuer**. Row **Suspend** /
**Activate** (immediate), **Delete** -> dialog **Delete trusted issuer**
naming the id, button **Delete**; failure alert **Could not delete trusted
issuer**; and **Accreditations** (also read-only) -> dialog
**Accreditations** with the text "N accreditation VCs" and button **Close**,
or alert **Could not issue accreditations**. A `<form aria-label="Check
trust">` with **Issuer id**, **Credential type**, button **Check**; result
inside it in `role="status"`, text **Trusted** or **Not trusted** (+ reason),
and an `<ol aria-label="Trust chain">` with one item per id, issuer first.
The issuer dialog also has a **Postal address** group: **Street** (which
shows the address error), **Locality**, **Postal code**, **Region** and
**Country**, all optional together; a partial address or a country that is
not two letters is refused in the dialog.
When the console may write: table **Audit log** (Seq, Time, Actor, Action,
Issuer id, Hash), button **Load more** while older pages remain. Links
**LoTE (JSON)** and **signed LoTE (JWS)**. When `GET /api/registry/v1/admin`
is not 2xx: visible text containing "read-only" (any case), no enabled
Add/Suspend/Activate/Delete buttons and no audit log; the table and check
form still work. Empty: **No trusted issuers**.

The console polls nothing on its own as far as the tests care: they click
**Refresh** until the expected state appears.
