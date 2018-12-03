Dscfg client
============

Server implementation for dynamic shared config.

About
-----

This crate implements server side of `dscfg` protocol. It exposes simple functions to listen for incoming clients using either default length delimited encoding or a custom encoding.

The crate doesn't implement storing of the configuration but defines `Storage` trait used for implementing it instead. Thanks to it, the code is more flexible. If you don't want to implement it yourself, but just use sensible default, you may use `dscfg-cached_file_storage` crate, which provides a basic implementation.

All this being said, if you're looking for a dscfg server, you might want to use `dscfg-unix_server`, which implements everything required to get `dscfg` running.
