//! `key.*` commands — keyring management (5 commands).

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `key.generate` (arity 2): generate a new Ed25519/Curve25519 keypair,
/// store in keyring, return fingerprint.
pub fn generate(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `key.import` (arity 1): import armored public or private key into keyring.
pub fn import(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `key.export` (arity 2): export key from keyring as armored text
/// (secret flag exports private key).
pub fn export(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `key.list` (arity 0): list all keys in the keyring.
pub fn list(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `key.delete` (arity 1): delete a key from the keyring by fingerprint.
pub fn delete(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}
