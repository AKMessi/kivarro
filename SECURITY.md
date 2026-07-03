# Security Policy

Kivarro runs local models, starts local inference servers, reads local files when you import models or knowledge documents, and can expose a local OpenAI-compatible API endpoint. Treat bug reports and logs as potentially sensitive.

## Supported Versions

The public `main` branch is the supported development line until stable releases are published.

## Reporting a Vulnerability

Open a private security advisory on GitHub if available, or contact the maintainer through the repository owner account. Do not open public issues for vulnerabilities that include exploit details, local paths, private prompts, documents, logs, or model metadata.

Include:

- Kivarro version or commit SHA
- Operating system and CPU architecture
- Whether the issue requires Tauri desktop mode or browser preview mode
- Minimal reproduction steps
- Impact and expected behavior

Do not include:

- Proprietary model files
- API keys or tokens
- Private prompts, imported documents, chat logs, or local filesystem paths beyond what is necessary

## Security Boundaries

Kivarro is intended to run local inference tooling selected by the user. External binaries such as `llama-server` and `mistralrs` are outside this repository's trust boundary. Only run backends you trust.
