//! `gpg.*` commands — system gpg wrapper (13 commands).

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `gpg.detect` (arity 0): detect system gpg installation, return
/// `{installed, path, version}`.
pub fn detect(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.exec` (arity 1): execute raw gpg command with args string or list.
pub fn exec(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.encrypt` (arity 2): encrypt text using system gpg for a recipient.
pub fn gpg_encrypt(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.encrypt.file` (arity 3): encrypt a file using system gpg for a recipient.
pub fn gpg_encrypt_file(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.decrypt` (arity 1): decrypt an armored message using system gpg.
pub fn gpg_decrypt(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.decrypt.file` (arity 2): decrypt a file using system gpg.
pub fn gpg_decrypt_file(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.sign` (arity 2): sign text using system gpg (inline or detached).
pub fn gpg_sign(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.sign.file` (arity 3): sign a file using system gpg (inline or detached).
pub fn gpg_sign_file(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.verify` (arity 2): verify a signature using system gpg
/// (inline or detached).
pub fn gpg_verify(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.verify.file` (arity 2): verify a signature file using system gpg.
pub fn gpg_verify_file(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.key.list` (arity 0): list keys in system gpg keyring.
pub fn key_list(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.key.import` (arity 1): import armored key into system gpg keyring.
pub fn key_import(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}

/// `gpg.key.export` (arity 2): export key from system gpg keyring as
/// armored text.
pub fn key_export(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    Err(PluginError::runtime("not yet implemented"))
}
