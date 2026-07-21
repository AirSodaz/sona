# Contributing to Sona

Thanks for contributing to Sona. This repository moves quickly, so the goal of this guide is to keep day-to-day development predictable: small scoped changes, clear validation, and clean pull requests.

## Development flow

Use this default loop for almost every change:

1. Create a focused branch.
2. Implement one cohesive change.
3. Run the smallest useful local validation.
4. Commit only the files related to that change.
5. Open a pull request with clear notes about scope and verification.

Recommended branch names:

- `feat/...`
- `fix/...`
- `refactor/...`
- `docs/...`
- `chore/...`

Agent-assisted branches can also use a `codex/...` prefix, for example `codex/feat-workspace-search`.

## Local setup and validation

Follow the [development guide](docs/development.md) for prerequisites, installation, development commands, and source builds.

Package roles and host capability boundaries are reviewed contracts. **Do not
infer a crate’s architecture role from its folder name.** Read
`[package.metadata.sona] role` in that package’s `Cargo.toml`, the registry in
`scripts/architecture-policy.mjs`, and the [architecture guide](docs/architecture.md).

Known historical mismatches (path still under `adapters/`, role is not an adapter):

| Path | Package | Actual role |
| --- | --- | --- |
| `adapters/sync/` | `sona-sync` | application |
| `adapters/uniffi_bind/` | `sona-uniffi-bind` | host |

Each of those trees has a local `README.md` that restates the role. Physical
relocation is out of scope unless a dedicated slice owns the move.

When you change workspace crate roles, host Cargo dependencies, or host
production wiring for a matrix capability, run:

```bash
node --test scripts/crate-boundaries.test.js scripts/host-capability-matrix.test.js scripts/host-wiring-inventory.test.js
```

Do not add CLI Sync, or UniFFI model-downloads / media-detector / API-server wiring, without updating the reviewed host capability matrix and its tests.

Run the smallest validation that covers your change. Common frontend and script checks are:

```bash
pnpm test
pnpm run test:scripts
```

Run the CI-style frontend gates before opening broad PRs:

```bash
pnpm run lint:ci
pnpm run test:ci
pnpm run build:ci
```

Run Rust backend checks when you touch `platforms/desktop/`:

```bash
cargo test --manifest-path platforms/desktop/Cargo.toml --no-run
cargo test --manifest-path platforms/desktop/Cargo.toml --test desktop_entry
```

If your change affects shared Rust behavior, also run the focused lib selectors
that match the changed area. On Windows, the full Rust suite should now pass
because Rust test binaries embed the Common Controls v6 manifest needed by the
native dialog stack:

```bash
cargo test --manifest-path platforms/desktop/Cargo.toml
```

Security audit checks:

```bash
pnpm audit --prod
cargo audit --file Cargo.lock
```

`cargo audit` currently has no known-vulnerability findings for this repo, but
it can still report tracked RustSec warnings from transitive desktop/runtime
dependencies such as GTK3/Wry, `glib`, `unic`, `paste`, and
`proc-macro-error`. Treat those warning families as known dependency risk unless
your PR intentionally upgrades the Tauri/Wry/GTK stack.

## Commit message format

This repo uses Conventional Commits:

```text
type(scope): subject
```

`scope` is optional, but recommended when it improves clarity.

Allowed types:

- `feat`
- `fix`
- `refactor`
- `docs`
- `style`
- `test`
- `build`
- `ci`
- `chore`
- `perf`
- `revert`

Examples:

- `fix(workspace): close stale detail pane`
- `refactor(automation): offload runtime to Rust backend`
- `docs(user-guide): sync current support flows`

## Narrow commit discipline

Avoid mixing unrelated UI, docs, frontend, and backend work in the same commit.

Before every commit:

```bash
git status --short
```

Stage only the files for the change you are shipping:

```bash
git add <paths>
```

Then confirm the staged scope:

```bash
git diff --cached --name-only
```

If the staged file list is wider than the change you intend to ship, stop and fix it before committing.

## Pull request expectations

Before opening a PR:

- `pnpm run lint:ci`, `pnpm run test:ci`, and `pnpm run build:ci` should pass.
- Run focused tests that match the changed area.
- For Rust backend changes, run `cargo test --manifest-path platforms/desktop/Cargo.toml --no-run` and the focused Rust tests that match the change.
- For dependency or security-sensitive changes, run `pnpm audit --prod` and `cargo audit --file Cargo.lock`.
- Update repo docs for user-visible workflow or settings changes.
- Include screenshots or video for UI changes when they help reviewers.

The main repo doc surfaces to check are the bilingual root README, user guide,
CLI guide, and development guide.

If your branch intentionally leaves unrelated working tree changes out of the PR, say so clearly in the PR description.

## Hooks and validation

This repo installs lightweight local hooks with `husky`.

- `commit-msg` validates Conventional Commit format.
- `pre-commit` checks staged diffs for whitespace problems first.
- `lint-staged` runs a small staged-file safety check.

The staged safety check currently does two things:

- blocks unresolved merge conflict markers in staged text/config files
- blocks invalid staged JSON files

`pnpm run lint` is a hard requirement. The codebase maintains a clean ESLint baseline.
