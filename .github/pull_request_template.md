## What changed

-

## Why

-

## Validation

- [ ] `pnpm run lint:ci`
- [ ] `pnpm run test:ci`
- [ ] `pnpm run build:ci`
- [ ] Focused tests for the changed area
- [ ] Rust backend checks if `src-tauri/` changed (`cargo test --manifest-path src-tauri/Cargo.toml --no-run` plus focused Rust tests)
- [ ] Security audits if dependencies/security posture changed (`pnpm audit --prod` and `cargo audit --file Cargo.lock`)

## Docs updated

- [ ] Not needed
- [ ] `README.md`
- [ ] `README.zh-CN.md`
- [ ] `docs/user-guide.md`
- [ ] `docs/user-guide.zh-CN.md`
- [ ] Other:

## Screenshots / video if UI changed

- Not applicable

## Known risk / rollback note

-

## Scope note

- [ ] This PR intentionally excludes unrelated local changes
- [ ] No unrelated local changes were present while preparing this PR
