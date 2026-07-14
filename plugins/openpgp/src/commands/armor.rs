//! `armor` and `dearmor` commands (2 commands).

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `armor` (arity 1): convert base64 binary to ASCII armor.
pub fn armor(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `dearmor` (arity 1): convert ASCII armor to base64 binary.
pub fn dearmor(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}
