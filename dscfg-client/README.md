Dscfg client
============

Client implementation for dynamic shared config.

About
-----

This crate implements client side of `dscfg` protocol. It exposes simple interface to manipulate configuration and listen for notifications. It supports communication over any kind of async byte stream (TCP/IP, Unix socket...) and allows one to provide their own encoding implementation or use the default (length-delimited Json messages).
