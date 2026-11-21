# Stable native release fixture

`source.smod` pins `26.3-0.1.0`. `valid.binaries.json` describes a rebuild for `26.3-0.1.1`, with the Linux client and dedicated server host names as SDK metadata names them. The archives contain inert placeholder bytes, not loadable libraries. The other signed descriptors omit a required host, substitute the server archive for the client, or carry a signature from a different key. The source variants exercise the lockfile requirement and the builder's Cargo feature refusal.

`schema-1.json` shows the same archive records as the registry emits, alongside an unrelated source-only project and a project that depends on the native mod's source version. It is also used to test the unmodified schema-1 reader.

These packages and tar.xz archives were assembled independently of `cargo sunrise-mod`. The `.minisig` files and embedded archive signatures were made with disposable fixture keys; only the project public key is retained.
