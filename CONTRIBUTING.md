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

## Local setup

Install dependencies:

```bash
npm install
```

Start the app in development:

```bash
npm run tauri dev
```

Run frontend tests:

```bash
npm test
```

Build the app:

```bash
npm run build
```

Verify the packaged CLI bundle when you touch packaging or CLI behavior:

```bash
npm run verify:cli-bundle
```

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

- `npm run build` should pass.
- Run focused tests that match the changed area.
- Update repo docs for user-visible workflow or settings changes.
- Include screenshots or video for UI changes when they help reviewers.

The main repo doc surfaces to check are:

- `README.md`
- `README.zh-CN.md`
- `docs/user-guide.md`
- `docs/user-guide.zh-CN.md`

If your branch intentionally leaves unrelated working tree changes out of the PR, say so clearly in the PR description.

## Hooks and validation

This repo installs lightweight local hooks with `husky`.

- `commit-msg` validates Conventional Commit format.
- `pre-commit` checks staged diffs for whitespace problems first.
- `lint-staged` runs a small staged-file safety check.

The staged safety check currently does two things:

- blocks unresolved merge conflict markers in staged text/config files
- blocks invalid staged JSON files

`npm run lint` is still a manual quality task for now. The current lint baseline is not clean enough to make full ESLint a hard commit gate yet.
