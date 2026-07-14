//! `armor` and `dearmor` commands (2 commands).

use std::io::{Read, Write};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sequoia_openpgp::armor::{Kind, Reader as ArmorReader, ReaderMode, Writer as ArmorWriter};

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// `armor` (arity 1): convert base64-encoded binary to ASCII armor.
///
/// The input is a base64 string representing raw binary bytes. Those
/// bytes are wrapped in a generic PGP armor block (`Kind::File`).
pub fn armor(_state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let b64_input = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("armor expects (binary_b64)"))?;

    // Decode base64 to get the raw binary.
    let binary = B64
        .decode(b64_input)
        .map_err(|e| PluginError::parse_failed(format!("base64 decode: {e}")))?;

    // Wrap the binary in PGP armor (Kind::File for generic binary).
    let mut sink: Vec<u8> = Vec::new();
    let mut writer = ArmorWriter::new(&mut sink, Kind::File)
        .map_err(|e| PluginError::runtime(format!("create armor writer: {e}")))?;
    writer
        .write_all(&binary)
        .map_err(|e| PluginError::runtime(format!("write to armor: {e}")))?;
    writer
        .finalize()
        .map_err(|e| PluginError::runtime(format!("finalize armor: {e}")))?;

    let armored = String::from_utf8(sink)
        .map_err(|e| PluginError::runtime(format!("armor output not utf8: {e}")))?;
    Ok(DyylValue::Str(armored))
}

/// `dearmor` (arity 1): convert ASCII armor back to base64-encoded binary.
pub fn dearmor(_state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let armor_input = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("dearmor expects (armor)"))?;

    // Parse the armor and extract the raw binary. `from_reader` is
    // infallible; parsing errors surface when reading.
    let cursor = std::io::Cursor::new(armor_input.as_bytes());
    let mut reader = ArmorReader::from_reader(cursor, ReaderMode::Tolerant(None));

    let mut binary = Vec::new();
    reader
        .read_to_end(&mut binary)
        .map_err(|e| PluginError::parse_failed(format!("read armor binary: {e}")))?;

    let b64 = B64.encode(&binary);
    Ok(DyylValue::Str(b64))
}
