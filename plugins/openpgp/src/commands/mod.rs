//! Command dispatch — routes `<cmd>` strings to per-domain submodules.
//!
//! 30 commands total: 17 sequoia-based + 13 gpg-wrapper.

pub mod armor;
pub mod decrypt;
pub mod encrypt;
pub mod gpg;
pub mod key;
pub mod sign;
pub mod verify;

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// Dispatch a command by name.
///
/// Returns the encoded result on success, or a `PluginError` on failure.
pub fn dispatch(
    state: &mut PluginState,
    cmd: &str,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    match cmd {
        // key.* (5 commands)
        "key.generate" => key::generate(state, args),
        "key.import" => key::import(state, args),
        "key.export" => key::export(state, args),
        "key.list" => key::list(state, args),
        "key.delete" => key::delete(state, args),
        // encrypt.* (2 commands)
        "encrypt" => encrypt::encrypt(state, args),
        "encrypt.file" => encrypt::encrypt_file(state, args),
        // decrypt.* (2 commands)
        "decrypt" => decrypt::decrypt(state, args),
        "decrypt.file" => decrypt::decrypt_file(state, args),
        // sign.* (2 commands)
        "sign" => sign::sign(state, args),
        "sign.file" => sign::sign_file(state, args),
        // verify.* (2 commands)
        "verify" => verify::verify(state, args),
        "verify.file" => verify::verify_file(state, args),
        // sym.* (2 commands)
        "sym.encrypt" => encrypt::sym_encrypt(state, args),
        "sym.decrypt" => decrypt::sym_decrypt(state, args),
        // armor.* (2 commands)
        "armor" => armor::armor(state, args),
        "dearmor" => armor::dearmor(state, args),
        // gpg.* (13 commands)
        "gpg.detect" => gpg::detect(state, args),
        "gpg.exec" => gpg::exec(state, args),
        "gpg.encrypt" => gpg::gpg_encrypt(state, args),
        "gpg.encrypt.file" => gpg::gpg_encrypt_file(state, args),
        "gpg.decrypt" => gpg::gpg_decrypt(state, args),
        "gpg.decrypt.file" => gpg::gpg_decrypt_file(state, args),
        "gpg.sign" => gpg::gpg_sign(state, args),
        "gpg.sign.file" => gpg::gpg_sign_file(state, args),
        "gpg.verify" => gpg::gpg_verify(state, args),
        "gpg.verify.file" => gpg::gpg_verify_file(state, args),
        "gpg.key.list" => gpg::key_list(state, args),
        "gpg.key.import" => gpg::key_import(state, args),
        "gpg.key.export" => gpg::key_export(state, args),
        _ => Err(PluginError::unknown_command(cmd)),
    }
}
