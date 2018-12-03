//! Dynamic shared configuration
//! ============================
//! 
//! `dscfg` and associated crates allow several programs to share runtime-changeable configuration. One of the programs acts as a server - it accepts change requests and notifies other programs of changes. It should also store the configuration on disk.
//! 
//! This can be useful for example when changes in GUI should be immediately reflected by behavior of some background process.
//! 
//! The whole system is meant to be very modular and extensible. Basic implementations of Unix-socket-based server and client are available, but another alternatives (e.g. TCP-based) can be easily developed. This is also the reason these crate intentionally don't implement authentification nor authorization right now as they can be easily added on top.
//!
//! This crate is a facade for separate client and server crates.

extern crate dscfg_client;
extern crate dscfg_server;

/// Reexport of `dscfg-client` crate.
///
/// This crate is used by programs that use the shared configuration.
pub mod client {
    pub use dscfg_client::*;
}

/// Reexport of `dscfg-server` crate.
///
/// This crate is used by servers that store and serve the configuration.
pub mod server {
    pub use dscfg_server::*;
}
