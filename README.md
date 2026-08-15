# wwn-wasm

In-process **WASI Preview 1 + Preview 2** runtime for Wawona.

Apple mobile uses Wasmtime **Pulley** (interpreter: WASM → Pulley IR → interpret;
no native ARM, no `MAP_JIT`). macOS may use Cranelift. `.wasm` files are user
documents — Apple does not sign them.

This is an L3′ repo: depends on `wwn-toolchain` only. See
[wwn-repo-dag.md](https://github.com/Wawona/Wawona/blob/development/docs/wwn-repo-dag.md)
and milestone
[Support WASI P1 P2 WASM!](https://github.com/Wawona/Wawona/milestone/2).

C ABI (`include/wawona_wasm.h`):

- `wawona_wasm_can_run(path)`
- `wawona_wasm_run(argc, argv)`
- `wawona_terminal_raw_enabled()`

Host extras (P1 import module `wawona_socket` / `wawona_terminal`):

- POSIX TCP/UDP/unix + `connect_host`
- `wawona_wayland_connect` / `shm_create` / `shm_send` (SCM_RIGHTS into the
  existing compositor)
- TTY raw/cooked bit

P2 guests use `wasi:cli` / `filesystem` / `sockets` / `clocks` / `random`.
`wasi:http` is not linked yet (size); use sockets or a native port.

Native ports stay first-class. WASM is the long-tail escape hatch.

Demos: [`examples/`](examples/README.md).
