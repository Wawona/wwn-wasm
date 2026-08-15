# WASI demos for Wawona

Compile on a desktop, copy the `.wasm` into the Wawona Documents folder
(Files / File Sharing / `scp`), then in the on-device shell:

```text
wasm ./tool.wasm hello
./tool.wasm hello
```

| Dir | Target | Notes |
|---|---|---|
| `rust/` | `wasm32-wasip1` | hello, fs-*, tcp-client (host `wawona_socket`) |
| `go/` | `GOOS=wasip1` | hello, fs-* |
| `swift/` | `wasm32-unknown-wasip1` | hello only; **no Foundation** |
| `wasip2/` | `wasm32-wasip2` | `wasi:cli` hello |
| `wayland-shm/` | `wasm32-wasip1` | host Wayland connect + SHM fd-bridge |

Native ports stay first-class. Prefer `weston-simple-shm` / `foot` when we
have a recipe. See `Wawona/docs/wasm-wasi.md`.
