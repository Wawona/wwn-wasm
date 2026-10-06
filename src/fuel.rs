//! Guest fuel bursts.
//!
//! Fuel stays on so a pure wasm spin cannot hang the in-process shell.
//! Wasmtime does not burn fuel while a host call blocks, and it does not
//! refill when that call returns. A Wayland client therefore spends one
//! burst on everything between `socket_recv` returns: event dispatch and
//! the next SHM frame.
//!
//! `chess-wawona` logged `toplevel configure 0x0` (a normal first xdg
//! configure; the client ignores non-positive sizes and uses 640x800) and
//! then trapped `all fuel consumed by WebAssembly` inside `_start`, before
//! `committed`. The old 25_000_000 budget died in that first paint. A burst
//! has to cover one frame and one depth-3 search, which runs as pure wasm
//! between host calls.

/// Instructions available for one guest stretch between blocking receives.
pub const GUEST_BURST: u64 = 2_000_000_000;

#[cfg(test)]
mod tests {
    use super::GUEST_BURST;
    use wasmtime::{Instance, Module, Store};

    #[test]
    fn burst_covers_a_frame_sized_loop() {
        let engine = crate::engine().expect("engine");
        // ~7 operators per iteration. 8e6 iterations is past the old
        // 25_000_000 budget and inside one GUI burst.
        let wasm = wat::parse_str(
            r#"
            (module
              (func (export "burn")
                (local $i i32)
                (loop
                  local.get $i
                  i32.const 1
                  i32.add
                  local.tee $i
                  i32.const 8000000
                  i32.lt_u
                  br_if 0)))
            "#,
        )
        .unwrap();
        let module = Module::new(&engine, &wasm).unwrap();
        let mut store = Store::new(&engine, ());
        store.set_fuel(25_000_000).unwrap();
        let instance = Instance::new(&mut store, &module, &[]).unwrap();
        let burn = instance
            .get_typed_func::<(), ()>(&mut store, "burn")
            .unwrap();
        let err = burn.call(&mut store, ()).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.to_lowercase().contains("fuel"),
            "old 25e6 budget should trap a frame-sized loop: {msg}"
        );

        store.set_fuel(GUEST_BURST).unwrap();
        burn.call(&mut store, ())
            .expect("guest burst should finish the loop");
    }
}
