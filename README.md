# Kivarro

Kivarro is a Rust/Tauri local model inference workstation for Windows, macOS, and Linux. It is built for running, tuning, inspecting, and serving private local AI models from a desktop cockpit UI.

> License note: Kivarro is source-available for non-commercial use under the PolyForm Noncommercial License 1.0.0. This is not an OSI open-source license because commercial use is restricted.

## Status

- Tauri v2 desktop app with SvelteKit and TypeScript.
- Precision-instrument UI shell with dense nav rail, contextual panel, workspace, inspector, status bar, and strict design tokens.
- Implemented views for Command Center, Model Registry, Hardware Fit, Expert Tuning, RAG Knowledge Bases, Agents, Local API, Benchmarks, Logs, and Settings.
- Rust commands for CPU/RAM telemetry, local model discovery/import under `./models`, GGUF metadata indexing, API endpoint metadata, logs, and persisted benchmark result surfaces.
- Cross-platform accelerator discovery with NVIDIA SMI live utilization/VRAM telemetry and OS fallbacks for Windows, macOS, and Linux GPU inventory.
- Persistent `.kivarro.json` inference profiles stored in the app config directory.
- Profile-backed tuning controls for sampling, runtime, KV cache precision, context length, batching, mmap/mlock, and Flash Attention.
- Model load-plan estimator for RAM pressure, KV cache allocation, runtime overhead, and GPU/CPU layer split, using GGUF layer/context metadata when available.
- Engine supervisor for loading a selected local model through either `llama.cpp` `llama-server` or `mistral.rs` `mistralrs serve`, with OpenAI-compatible chat completions, live token streaming, and stop controls in Command Center.
- Persisted Local API host/port configuration with status-bar synchronization and copyable OpenAI-compatible base URL.
- Tokens/sec benchmark runner for the loaded model, persisted under the app config directory with eval count, eval duration, tokens/sec, and load duration.
- Local RAG knowledge bases with persisted document metadata, UTF-8 text/Markdown/source ingestion, deterministic chunking, and retrieval test ranking.
- Persisted System Logs for profile saves, model imports, engine lifecycle events, API endpoint changes, benchmark runs, and knowledge-base updates.
- Browser-preview fallbacks for UI smoke testing outside Tauri.
- Windows ARM64 release bundling verified with MSI and NSIS outputs.

## License

Kivarro is licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE). You may use, modify, and distribute it for permitted non-commercial purposes. For commercial use, contact the repository owner.

The license text is from the PolyForm Project: <https://polyformproject.org/licenses/noncommercial/1.0.0>.

## Prerequisites

- Node.js 20 or newer
- Rust stable
- Tauri v2 system prerequisites for your operating system
- Optional for real inference: `llama-server` from llama.cpp or `mistralrs`

## Development

```bash
npm install
npm run check:all
npm run tauri dev
```

For browser-only UI work:

```bash
npm run dev -- --host 127.0.0.1 --port 4173
```

## Verification

```bash
npm run check:all
npm run tauri build
```

The browser preview can be checked with:

```bash
npm run preview -- --host 127.0.0.1 --port 4173
```

Before publishing a release, verify the UI at:

- `1440x900`
- `1280x720`
- `900x720`

## Model files

Place local model files under `./models`, or paste a model file path into Model Registry to copy it into the local library. The scanner recognizes `.gguf`, `.safetensors`, `.bin`, and `.mlx` files. GGUF files are indexed directly from the file header and metadata block for architecture, quantization, tensor count, context length, and transformer block count without loading tensor payloads.

Model binaries are intentionally ignored by git. Do not commit local models, private prompts, imported documents, generated installers, logs, or `.env` files.

## Local engines

Kivarro can supervise a local OpenAI-compatible inference process. Choose `llama.cpp` or `mistral.rs` in Expert Tuning, then load the selected model.

For `llama.cpp`, install or build `llama.cpp`, then either put `llama-server` on `PATH` or point Kivarro at it. For `mistral.rs`, install `mistralrs`, then either put it on `PATH` or point Kivarro at it:

```powershell
$env:KIVARRO_LLAMA_SERVER = "C:\path\to\llama-server.exe"
$env:KIVARRO_MISTRALRS = "C:\path\to\mistralrs.exe"
$env:KIVARRO_API_PORT = "8080"
```

On macOS/Linux:

```bash
export KIVARRO_LLAMA_SERVER=/path/to/llama-server
export KIVARRO_MISTRALRS=/path/to/mistralrs
export KIVARRO_API_PORT=8080
```

The Local API view persists the host and port to the app config directory. `KIVARRO_API_PORT` is used as the initial/default port when no saved setting exists. Stop the loaded model before changing the endpoint, then load the model again to start the backend on the new host/port.

The Load Model action starts the selected backend with the active `.kivarro.json` profile. `llama.cpp` launches `llama-server` with model path, context length, CPU threads, batch/micro-batch, GPU layers, tensor split, KV cache precision, mmap/mlock, Flash Attention, and RoPE overrides. `mistral.rs` launches `mistralrs serve -m <model> --host <configured-host> -p <configured-port> --no-ui` and uses the single-model OpenAI-compatible request id `default`.

Prompt submission uses `POST /v1/chat/completions` on the local server with `stream: true`; the Rust backend parses the server-sent event stream and forwards token deltas to the UI over Tauri events. Active generations can be stopped from Command Center; Kivarro marks the stream as cancelled and closes the local SSE reader.

## Profiles

Kivarro seeds four default profiles on first launch:

- Balanced Engineer
- Strict JSON Extractor
- Local Code Reviewer
- Long Context Analyst

Profiles are saved as `.kivarro.json` files through the Tauri backend. The profile schema includes system prompt, sampling controls, runtime controls, and output constraints.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Security-sensitive reports should follow [SECURITY.md](SECURITY.md).

## Public Release Checklist

- `npm run check:all`
- `npm run tauri build`
- Confirm no local model files or generated bundles are staged.
- Confirm package, Cargo, README, and license metadata all match `PolyForm-Noncommercial-1.0.0`.
