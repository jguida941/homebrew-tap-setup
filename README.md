# Homebrew Tap Setup

A CLI that creates a Homebrew tap repo, wires it to GitHub, and scaffolds a formula. It is designed to be step-based, resumable, and transparent.

## Requirements
- macOS with Homebrew installed
- `git`, `gh`, and `brew` on PATH
- `gh auth login` completed

## Usage
Build/run locally:
```bash
cargo run -- \
  --owner <github-owner> \
  --tap <tap-short-name> \
  --visibility public \
  --branch main \
  --formula-mode stub
```

Create a real formula from a tarball:
```bash
cargo run -- \
  --owner <github-owner> \
  --tap <tap-short-name> \
  --visibility public \
  --branch main \
  --formula-mode brew-create \
  --formula-url <tarball-url> \
  --formula-name <name>
```

Flags:
- `--dry-run`: skip apply steps but record state
- `--resume <run-id>`: resume a previous run using the stored inputs
- `--repo-name`: override the repo name (default: `homebrew-<tap>`)
- `--formula-mode`: `stub` or `brew-create`
- `--formula-url`: required for `brew-create`
- `--formula-name`: optional; if omitted we try to derive it from the URL

## State
Each run writes state to:
```
~/Library/Application Support/homebrew-tap-setup/runs/<run-id>/state.json
```

## Notes
- If your repo name does not follow `homebrew-<tap>`, the shorthand `brew tap owner/<tap>` will not work.
- The formula produced by `brew create` may still need edits (description, homepage, license, test).

## Current Status (2026-02-13)
- End-to-end flow implemented: preflight -> tap-new -> gh repo create -> add formula -> commit/push -> validate -> summary.
- Resume support via stored run state.
- CI and release workflows enabled on this repo.

## Next Steps
- Add optional validation modes: `brew audit --new`, `brew test`, or `brew install` for the created formula.
- Improve error messaging and remediation tips (especially for `gh` auth).
- Decide how to handle `brew create` editor flow more cleanly across platforms.
