pub mod iface {
    include!(concat!(env!("OUT_DIR"), "/io_github_hepp3n_kdeconnect.rs"));
}

pub use iface::*;

/// XDG_RUNTIME_DIR socket path used by both service and clients
pub fn socket_address() -> String {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    format!("unix:{}/kdeconnect.varlink", runtime_dir)
}
