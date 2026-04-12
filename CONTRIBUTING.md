# Contributing to Hellox

Thanks for considering a contribution to Hellox.

## Before you start

- Read [README.md](./README.md) or [README_CN.md](./README_CN.md)
- Review the current local-first boundary in `docs/HELLOX_LOCAL_FIRST_BOUNDARIES.md`
- Check current implementation status in `docs/HELLOX_LOCAL_FEATURE_AUDIT.md`
- Follow [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)

## Recommended contribution flow

1. Fork the repository
2. Create a branch, for example `feature/workflow-panel`
3. Make focused changes
4. Run relevant tests
5. Commit with a clear message
6. Open a Pull Request

## Development tips

- Prefer small, focused patches
- Keep local-first / remote-capable boundaries intact
- Avoid introducing cloud-only assumptions into the primary path
- Keep documentation aligned with code and tests

## Validation

At minimum, run the checks relevant to your change. Typical commands:

```powershell
cargo fmt --all
cargo test --workspace
```

For smaller changes, targeted checks are also welcome:

```powershell
cargo test -p hellox-cli
cargo test -p hellox-agent
```

## Commit style

Simple commit messages are fine. Conventional commit style is recommended:

- `feat:` new functionality
- `fix:` bug fixes
- `docs:` documentation
- `refactor:` structural cleanup
- `test:` test-only updates
- `chore:` maintenance work

## Good first contribution areas

- richer `hellox-tui` interaction
- workflow authoring and visual surfaces
- tmux / iTerm fixture capture and replay
- local memory, telemetry, and diagnostics improvements

## Questions

If something is unclear, open an issue or discussion in the repository.

## Security

For sensitive security issues, please follow [SECURITY.md](./SECURITY.md) instead of
opening a public issue.

---

This repository is licensed under the [Apache-2.0 License](./LICENSE).
