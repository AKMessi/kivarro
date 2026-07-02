# Kivarro

Kivarro is a Rust/Tauri local model inference workstation for Windows, macOS, and Linux. The current baseline establishes the desktop shell, typed Rust command surface, model discovery foundation, and a dense command-center UI inspired by high-end instrumentation software.

## Current baseline

- Tauri v2 desktop app with SvelteKit and TypeScript.
- Custom Kivarro application shell with nav rail, contextual panel, workspace, inspector, and status bar.
- Implemented views for Command Center, Model Registry, Hardware Fit, Expert Tuning, RAG Knowledge Bases, Agents, Local API, Benchmarks, Logs, and Settings.
- Rust commands for CPU/RAM telemetry, local model discovery under `./models`, API endpoint metadata, logs, and benchmark result surfaces.
- Browser-preview fallbacks for UI smoke testing outside Tauri.
- Windows ARM64 release bundling verified with MSI and NSIS outputs.

## Development

```bash
npm install
npm run check
npm run build
npm run tauri dev
```

## Verification

```bash
npm run check
cargo check --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri build
```

The browser preview can be checked with:

```bash
npm run preview -- --host 127.0.0.1 --port 4173
```

## Model files

Place local model files under `./models`. The baseline scanner currently recognizes `.gguf`, `.safetensors`, `.bin`, and `.mlx` files.

## Next engineering milestones

- Engine adapter layer for `llama.cpp` and `mistral.rs`.
- Real GPU/accelerator discovery and VRAM telemetry.
- OpenAI-compatible local server implementation.
- Profile persistence for `.kivarro.json`.
- RAG ingestion, vector indexing, retrieval testing, and citations.
- Benchmark runner with CSV export.
