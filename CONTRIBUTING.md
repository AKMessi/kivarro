# Contributing to Kivarro

Kivarro is an open-source local inference workstation. Contributions are welcome under the MIT License.

## Development Setup

Required tooling:

- Node.js 20 or newer
- Rust stable
- Tauri v2 prerequisites for your OS
- A local inference backend only when testing real model runs: `llama-server` from llama.cpp or `mistralrs`

Install and verify:

```bash
npm install
npm run check:all
```

Run the desktop app:

```bash
npm run tauri dev
```

Run a browser-only UI preview:

```bash
npm run dev -- --host 127.0.0.1 --port 4173
```

## Pull Request Checklist

Before opening a PR:

- Run `npm run check`.
- Run `npm run build`.
- Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- Run `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`.
- Run `cargo check --manifest-path src-tauri/Cargo.toml`.
- Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- Run `npm run test:ui`.
- Do not commit local model files, generated installers, private logs, or `.env` files.
- Keep UI changes aligned with the instrument-panel design system in `src/app.css` and `src/routes/+page.svelte`.
- Keep Tauri/Rust IPC behavior backward compatible unless the PR explicitly documents a breaking change.

## Model Files and Private Data

Do not upload model binaries, prompts, documents, logs, or screenshots containing private data. Local model files belong in ignored paths such as `models/` or `src-tauri/models/`.

## Licensing

By contributing, you agree that your contribution is licensed under the same license as the project: MIT.
