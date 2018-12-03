Dynamic shared configuration
============================

These crates allow several programs to share runtime-changeable configuration. One of the programs acts as a server - it accepts change requests and notifies other programs of changes. It should also store the configuration on disk.

This can be useful for example when changes in GUI should be immediately reflected by behavior of some background process.

The whole system is meant to be very modular and extensible. Basic implementations of Unix-socket-based server and client are available, but another alternatives (e.g. TCP-based) can be easily developed. This is also the reason these crate intentionally don't implement authentification nor authorization right now as they can be easily added on top.

Note that this is currently proof-of-concept. I plan to improve it in the future as new requirements come.

In order to understand how you can use it, I recommend looking at Unix server and util implementations.

The crates
==========

DSCFG involves several crates that can be composed together to achieve desired results:

* `dscfg-proto`               - Common crate that defines basic communication protocol. It's shared by the client and the server.
                                All programs involved must use the same version of `dscfg-proto` in order to be compatible.
* `dscfg-server`              - The heart of `dscfg`. It allows serving the configuration to connected clients and storing it on disk.
* `dscfg-client`              - Implements client side of `dscfg` protocol and exposes simple interface to manipulate configuration and
                                listen for notifications.
* `dscfg`                     - A facade for client and server crates. Useful when one wants to write bridges/extensions or to type
                                `dscfg::client` instead of `dscfg_client`, which some people find nicer/more idiomatic.
* `dscfg-cached_file_storage` - An implementation of `Storage` trait defined by `dscfg-server` using file and a hash map to store data.
* `dscfg-unix_server`         - Full server implementation using Unix socket for communication.
* `dscfg-unix_util`           - Simple client that works with server. It can be used for debugging server, other clients
                                (via notifications), or in shell scripts.
