## Summary

- 

## Verification

- [ ] `npm run check`
- [ ] `npm run build`
- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `npm run test:ui`

## Risk

- [ ] No model files, private prompts, private documents, local logs, generated installers, or `.env` files are included.
- [ ] UI changes were checked at 1440x900, 1280x720, and 900x720 when relevant.
