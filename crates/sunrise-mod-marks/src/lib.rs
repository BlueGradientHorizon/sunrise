//! The imports of the handler traits, for `sunrise-mod-vocab`'s hidden marks methods.
//!
//! A wasm mod's module must import every game method it can be called through, so the loader can
//! refuse it by name when the game's table changes. A handler trait (`EnergyHandler`, ...) has one
//! generated function here, generic over the implementor and calling an import of each of the
//! trait's methods; the trait's hidden `__sunrise_marks` method calls it, so only a mod whose module
//! holds an implementor imports them. The vocabulary forbids unsafe code, so the imports live
//! below it, in this crate, whose only unsafe code is that generated block.
//!
//! The functions are generated from the vocabulary's sources by `sunrise-mod-api-gen`, as the
//! vocabulary's own build is.

include!(concat!(env!("OUT_DIR"), "/handler_marks_generated.rs"));
