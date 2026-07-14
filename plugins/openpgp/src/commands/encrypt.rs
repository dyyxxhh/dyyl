//! `encrypt.*` and `sym.encrypt` commands (3 commands).

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `encrypt` (arity 2): encrypt text for one or more recipients
/// (fingerprint or armored pubkey).
pub fn encrypt(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `encrypt.file` (arity 3): encrypt a file to an output file for one or
/// more recipients.
pub fn encrypt_file(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `sym.encrypt` (arity 2): symmetrically encrypt text with a passphrase.
pub fn sym_encrypt(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}
