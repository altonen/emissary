// Emissary CLI library root.
//
// This crate is primarily a binary, but this lib.rs re-exports modules
// needed for integration testing of the i2pcontrol subsystem.

#[cfg(feature = "i2pcontrol")]
pub mod i2pcontrol;
