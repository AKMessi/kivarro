# Changelog

## v0.1.0-alpha

Initial public alpha for Kivarro.

### Added

- Rust/Tauri desktop app for Windows, macOS, and Linux.
- Svelte command-center UI for local inference workflows.
- Local model registry with GGUF metadata indexing.
- llama.cpp and mistral.rs engine supervision.
- OpenAI-compatible local chat completions with streaming and cancellation.
- Prompt profiles for balanced inference, strict JSON extraction, code review, and long-context analysis.
- Runtime controls for context length, batching, CPU threads, GPU layers, KV cache precision, mmap/mlock, Flash Attention, tensor split, and RoPE settings.
- Hardware, memory, context, API, benchmark, logs, and RAG workspace surfaces.
- GitHub Actions release pipeline for Linux x64/ARM64, macOS Intel/Apple Silicon, Windows x64, and Windows ARM64.

### Notes

- Alpha builds are unsigned; Windows SmartScreen and macOS Gatekeeper may warn on first launch.
- Kivarro does not bundle model weights or inference backends. Install `llama-server` or `mistralrs` separately and keep model files local.
- Report reproducible crashes with OS, architecture, backend, model format, and sanitized logs.
