#![cfg(test)]
//! The GPU tests' adapter and device helpers: the client render crate's own (`gfx::test_gpu`),
//! which already hands out a test lock, an adapter when the machine has one, and a device. Both
//! crates' GPU tests take the same lock, so they serialize even when the whole workspace runs
//! its tests at once.
// Some helpers are the client's tests' (DX12 compilers, depth-load support): this crate includes
// the same file, and `dead_code` would flag what it does not call.
#![allow(dead_code)]

include!("../../sunrise-client-render/src/gfx/test_gpu.rs");
