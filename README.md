# Kivarro

Kivarro is a Rust/Tauri local model inference workstation for Windows, macOS, and Linux. The current baseline establishes the desktop shell, typed Rust command surface, model discovery foundation, and a dense command-center UI inspired by high-end instrumentation software.

## Current baseline

- Tauri v2 desktop app with SvelteKit and TypeScript.
- Custom Kivarro application shell with nav rail, contextual panel, workspace, inspector, and status bar.
- Implemented views for Command Center, Model Registry, Hardware Fit, Expert Tuning, RAG Knowledge Bases, Agents, Local API, Benchmarks, Logs, and Settings.
- Rust commands for CPU/RAM telemetry, local model discovery under `./models`, GGUF metadata indexing, API endpoint metadata, logs, and persisted benchmark result surfaces.
- Cross-platform accelerator discovery with NVIDIA SMI live utilization/VRAM telemetry and OS fallbacks for Windows, macOS, and Linux GPU inventory.
- Persistent `.kivarro.json` inference profiles stored in the app config directory.
- Profile-backed tuning controls for sampling, runtime, KV cache precision, context length, batching, mmap/mlock, and Flash Attention.
- Model load-plan estimator for RAM pressure, KV cache allocation, runtime overhead, and GPU/CPU layer split, using GGUF layer/context metadata when available.
- Engine supervisor for loading a selected local model through either `llama.cpp` `llama-server` or `mistral.rs` `mistralrs serve`, with OpenAI-compatible chat completions, live token streaming, and stop controls in Command Center.
- Tokens/sec benchmark runner for the loaded model, persisted under the app config directory with eval count, eval duration, tokens/sec, and load duration.
- Local RAG knowledge bases with persisted document metadata, UTF-8 text/Markdown/source ingestion, deterministic chunking, and retrieval test ranking.
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

Place local model files under `./models`. The scanner recognizes `.gguf`, `.safetensors`, `.bin`, and `.mlx` files. GGUF files are indexed directly from the file header and metadata block for architecture, quantization, tensor count, context length, and transformer block count without loading tensor payloads.

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

The Load Model action starts the selected backend with the active `.kivarro.json` profile. `llama.cpp` launches `llama-server` with model path, context length, CPU threads, batch/micro-batch, GPU layers, tensor split, KV cache precision, mmap/mlock, Flash Attention, and RoPE overrides. `mistral.rs` launches `mistralrs serve -m <model> --host 127.0.0.1 -p <port> --no-ui` and uses the single-model OpenAI-compatible request id `default`.

Prompt submission uses `POST /v1/chat/completions` on the local server with `stream: true`; the Rust backend parses the server-sent event stream and forwards token deltas to the UI over Tauri events. Active generations can be stopped from Command Center; Kivarro marks the stream as cancelled and closes the local SSE reader.

## Profiles

Kivarro seeds four default profiles on first launch:

- Balanced Engineer
- Strict JSON Extractor
- Local Code Reviewer
- Long Context Analyst

Profiles are saved as `.kivarro.json` files through the Tauri backend. The profile schema includes system prompt, sampling controls, runtime controls, and output constraints.

## Next engineering milestones

- Deeper `mistral.rs` runtime flag mapping for GPU, quantization, and context controls.
- GPU vendor-specific utilization adapters beyond NVIDIA SMI.
- Built-in OpenAI-compatible proxy/server for external clients.
- Embedding-backed vector indexing and citation injection for RAG.
- Benchmark CSV export and comparison filters.
