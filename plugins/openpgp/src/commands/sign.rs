//! `sign.*` commands (2 commands).

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `sign` (arity 2): sign text with a key (inline or detached).
pub fn sign(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `sign.file` (arity 3): sign a file to an output file (inline or detached).
pub fn sign_file(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}
