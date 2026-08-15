# wayland-shm — same client, three languages

A **real Wayland client**: `wl_compositor` + `xdg_wm_base` + `wl_shm`.
Same request order as a Linux `weston-simple-shm` (port fidelity). No custom
draw API. The host only supplies unix connect + SCM_RIGHTS (WASI has no fd
passing).

Compile with **cargo / go / swift** — not Nix. See each `build.sh`.

```text
wasm ./wayland-shm-rust.wasm
```

## Sequence

1. `wawona_wayland_connect` → `XDG_RUNTIME_DIR/$WAYLAND_DISPLAY`
2. `wl_display.get_registry` + `sync`
3. Bind `wl_compositor` (v4), `wl_shm` (v1), `xdg_wm_base` (v2)
4. `create_surface` → `get_xdg_surface` → `get_toplevel` → `set_title` → `commit`
5. Host SHM file (`shm_create` / `shm_write`) + `wl_shm.create_pool` via
   `wawona_wayland_sendmsg` (protocol bytes + `SCM_RIGHTS`)
6. `create_buffer` (XRGB8888), wait `xdg_surface.configure`, `ack_configure`
7. `attach` + `damage` + `commit`
8. Event loop: `xdg_wm_base.ping` → `pong`; exit on `xdg_toplevel.close`

Window is 256×256, solid `0xFF3366CC` (XRGB8888 LE).
