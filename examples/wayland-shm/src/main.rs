//! Exercises the host Wayland ABI (connect + shm pool + SCM_RIGHTS send).
//! A full libwayland-client port is the fidelity path; this proves the bridge.

#[link(wasm_import_module = "wawona_socket")]
extern "C" {
    fn wawona_wayland_connect(fd_out: *mut i32) -> i32;
    fn wawona_wayland_shm_create(size: i32, fd_out: *mut i32) -> i32;
    fn wawona_wayland_shm_send(wl_fd: i32, shm_fd: i32) -> i32;
}

fn main() {
    let mut wl = 0i32;
    let mut shm = 0i32;
    let rc = unsafe { wawona_wayland_connect(&mut wl) };
    if rc != 0 {
        eprintln!("wayland connect errno={rc} (is WAYLAND_DISPLAY set?)");
        std::process::exit(1);
    }
    let rc = unsafe { wawona_wayland_shm_create(64 * 64 * 4, &mut shm) };
    if rc != 0 {
        eprintln!("shm create errno={rc}");
        std::process::exit(1);
    }
    let rc = unsafe { wawona_wayland_shm_send(wl, shm) };
    println!("wayland-shm: connect fd={wl} shm fd={shm} send errno={rc}");
}
