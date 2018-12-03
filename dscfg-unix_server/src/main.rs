#[macro_use]
extern crate configure_me;
extern crate dscfg_server;
extern crate dscfg_cached_file_storage;
extern crate serde_json;
extern crate void;
extern crate tokio;
#[macro_use]
extern crate slog;
extern crate slog_term;

include_config!();

use dscfg_server::ServerParams;
use dscfg_cached_file_storage::CachedFileStorage;

fn main() {
    use tokio::prelude::Future;
    use std::sync::{Arc, Mutex};

    let (cfg, _) = Config::including_optional_config_files(std::iter::empty::<std::path::PathBuf>()).unwrap_or_exit();

    let storage = CachedFileStorage::load_or_create(cfg.file).unwrap();
    let listener = tokio::net::unix::UnixListener::bind(cfg.socket).unwrap();
    let logger = slog::Logger::root(slog::Fuse(Mutex::new(slog_term::term_full())), o!());

    let server_params = ServerParams {
        storage: Arc::new(Mutex::new(storage)),
        executor: tokio::executor::DefaultExecutor::current(),
        incoming_clients: listener.incoming(),
        logger: logger,
    };

    info!(server_params.logger, "Starting the server");

    let server = dscfg_server::serve(server_params).map_err(|err| {
        println!("Server failed: {:?}", err);
    });

    tokio::run(server);
}
