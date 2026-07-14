//! Build the credentials JSON object passed to `dyyl_plugin_set_credentials`.
//!
//! Resolves `type: "string"`/`"file"`/`"directory"` fields from the
//! manifest spec against `credentials.toml` and the filesystem.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::credentials::credentials_dir_for_plugin;
use crate::i18n::Lang;
use crate::runtime::plugin::manifest::{CredentialField, CredentialsSpec};

/// Build the JSON string to pass to `dyyl_plugin_set_credentials`.
///
/// - `string` fields: read from `toml_fields` `HashMap`.
/// - `file` fields: read file content from `<credentials_dir>/<field>`.
/// - `directory` fields: inject the absolute path of `<credentials_dir>/<field>`.
/// - `__credentials_dir` is auto-injected even if manifest doesn't declare it.
#[allow(clippy::implicit_hasher)]
pub fn build_credentials_json(
    spec: Option<&CredentialsSpec>,
    plugin_name: &str,
    toml_fields: &HashMap<String, String>,
    _lang: Lang,
) -> Result<String, String> {
    let creds_dir = credentials_dir_for_plugin(plugin_name);
    let mut map: HashMap<String, String> = HashMap::new();

    ensure_credentials_dir(&creds_dir)?;
    map.insert(
        "__credentials_dir".to_string(),
        creds_dir.to_string_lossy().into_owned(),
    );

    if let Some(spec) = spec {
        for field in &spec.fields {
            let value = resolve_field(field, plugin_name, &creds_dir, toml_fields)?;
            map.insert(field.name.clone(), value);
        }
    }

    serde_json::to_string(&map).map_err(|e| format!("serialize credentials json: {e}"))
}

#[allow(clippy::implicit_hasher)]
fn resolve_field(
    field: &CredentialField,
    plugin_name: &str,
    creds_dir: &Path,
    toml_fields: &HashMap<String, String>,
) -> Result<String, String> {
    match field.r#type.as_str() {
        "string" => toml_fields
            .get(&field.name)
            .cloned()
            .ok_or_else(|| format!("missing string credential '{}' for plugin '{}'", field.name, plugin_name)),
        "file" => {
            let path = creds_dir.join(&field.name);
            if path.exists() {
                fs::read_to_string(&path)
                    .map_err(|e| format!("read credential file {}: {e}", path.display()))
            } else {
                eprintln!(
                    "warning: credential file '{}' for plugin '{}' not found, injecting empty",
                    path.display(),
                    plugin_name
                );
                Ok(String::new())
            }
        }
        "directory" => Ok(creds_dir.join(&field.name).to_string_lossy().into_owned()),
        other => Err(format!(
            "unknown credential field type '{other}' for field '{}'",
            field.name
        )),
    }
}

fn ensure_credentials_dir(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| format!("create credentials dir {}: {e}", dir.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
    }
    Ok(())
}
