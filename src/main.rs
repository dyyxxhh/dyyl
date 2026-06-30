//! dyyl — scripting language interpreter.
//!
//! Usage: `dyyl [--debug] [--host-json] [--lang <en|zh>] <filename>`
//!
//! Reads a dyyl script from `<filename>` (any extension).
//! With `--debug`, parser/runtime errors are printed to stderr
//! with line number, error kind, and the offending command text.
//! With `--host-json`, enables the streaming MCM host protocol:
//! `mcm.*` commands are emitted as NDJSON to stdout and the host
//! responds with NDJSON on stdin. Diagnostics still go to stderr.
//! With `--lang`, sets the display language for error messages.

use std::env;
use std::fs;
use std::process;
use std::sync::Arc;

use dyyl::i18n::Lang;
use dyyl::runtime::execute::{run_script_with_lang, run_script_with_lang_and_host};
use dyyl::runtime::host_provider::StdioHostConnection;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut debug = false;
    let mut host_json = false;
    let mut lang = Lang::default();
    let mut lang_explicit = false;
    let mut filename: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--debug" => debug = true,
            "--host-json" => host_json = true,
            "--lang" => {
                i += 1;
                if let Some(val) = args.get(i) {
                    if let Some(l) = Lang::from_name(val) {
                        lang = l;
                        lang_explicit = true;
                    } else {
                        eprintln!("dyyl: unknown language '{}'", val);
                        eprintln!("Usage: dyyl [--debug] [--host-json] [--lang <en|zh>] <filename>");
                        process::exit(1);
                    }
                } else {
                    eprintln!("dyyl: --lang requires a value");
                    process::exit(1);
                }
            }
            other if !other.starts_with('-') => filename = Some(other.to_string()),
            _ => {
                eprintln!("dyyl: unknown option '{}'", args[i]);
                eprintln!("Usage: dyyl [--debug] [--host-json] [--lang <en|zh>] <filename>");
                process::exit(1);
            }
        }
        i += 1;
    }

    // If --lang was not explicitly passed, try config file
    if !lang_explicit {
        if let Ok(config) = dyyl::config::load_config() {
            if let Some(ref config_lang) = config.lang {
                if let Some(l) = Lang::from_name(config_lang) {
                    lang = l;
                }
            }
        }
    }

    let filename = match filename {
        Some(f) => f,
        None => {
            if lang_explicit {
                let config = dyyl::config::DyylConfig {
                    lang: Some(lang.name().to_owned()),
                };
                if let Err(e) = dyyl::config::save_config(&config) {
                    eprintln!("dyyl: failed to save config: {e}");
                    process::exit(1);
                }
                eprintln!("dyyl: language set to '{}'", lang.name());
                process::exit(0);
            }
            eprintln!("dyyl 0.2.0 — script interpreter");
            eprintln!("Usage: dyyl [--debug] [--host-json] [--lang <en|zh>] <filename>");
            process::exit(1);
        }
    };

    let source = match fs::read_to_string(&filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", dyyl::i18n::cannot_read_file(lang, &filename, &e));
            process::exit(1);
        }
    };

    if host_json {
        let host = Arc::new(StdioHostConnection::new());
        let output = run_script_with_lang_and_host(&source, debug, lang, Some(host));
        if !output.error.is_empty() {
            eprintln!("dyyl: {}", output.error);
            process::exit(1);
        }
    } else {
        run_script_with_lang(&source, debug, lang);
    }
}
