Cached file storage for dscfg
=============================

Basic implementation of dscfg file storage using file to store configuration and
a hash map to cache it in memory.

About
-----

Dscfg doesn't dictate how the configuration is stored. Instead, it defines the
`Storage` trait which specifies required operations. This crate implements
`Storage` for a type by storing data in file as a Json map.

The file is updated atomically by writing to temp file first and moving it over
the old one. It's ensured that all data is written to file prior to moving, so
the file can never get corrupted - at worst it'll contain old configuration.

The whole configuration is cached in memory using hash map, so reading is fast.

License
-------

MITNFA
