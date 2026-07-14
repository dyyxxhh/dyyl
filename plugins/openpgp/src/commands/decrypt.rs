//! `decrypt.*` and `sym.decrypt` commands (3 commands).

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `decrypt` (arity 1): decrypt an armored message with optional
/// passphrase override.
pub fn decrypt(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `decrypt.file` (arity 2): decrypt an armored file to an output file.
pub fn decrypt_file(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `sym.decrypt` (arity 2): symmetrically decrypt an armored message
/// with a passphrase.
pub fn sym_decrypt(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}
