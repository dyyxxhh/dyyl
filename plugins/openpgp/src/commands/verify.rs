//! `verify.*` commands (2 commands).

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `verify` (arity 1): verify a signed message (inline or detached with
/// second arg).
pub fn verify(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `verify.file` (arity 1): verify a signed file (inline or detached with
/// second arg).
pub fn verify_file(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}
