#![cfg_attr(
    test,
    allow(
        clippy::all,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::panic,
        clippy::expect_used,
        clippy::todo,
        clippy::unimplemented,
        clippy::as_underscore,
        clippy::fn_to_numeric_cast_any,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::redundant_pub_crate,
        clippy::missing_const_for_fn,
    )
)]
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
use dyyl::runtime::execute::{
    run_script_with_lang_and_args, run_script_with_lang_and_host_and_args,
};
use dyyl::runtime::host_provider::StdioHostConnection;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Check for plugin management subcommands first.
    let mut lang = Lang::default();
    match dyyl::cli::try_handle_subcommand(&args, &mut lang) {
        dyyl::cli::CliResult::Handled(code) => process::exit(code),
        dyyl::cli::CliResult::NotASubcommand => {}
    }

    let mut debug = false;
    let mut host_json = false;
    let mut lang_explicit = false;
    let mut filename: Option<String> = None;

    let mut script_args: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        // 一旦看到 filename,后续所有 args 都转发给脚本
        if filename.is_some() {
            script_args.push(args[i].clone());
            i += 1;
            continue;
        }
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
                        eprintln!(
                            "Usage: dyyl [--debug] [--host-json] [--lang <en|zh>] <filename>"
                        );
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
                    installed_plugins: std::collections::HashMap::new(),
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

    // 预扫描：检测未填 ai.auto → 批量请求 → 回写源码。
    // 在 parser 之前执行，因为可能修改源文件。
    let prepass_path = std::path::PathBuf::from(&filename);
    if let Err(e) = dyyl::prepass::run(&prepass_path, lang) {
        eprintln!(
            "dyyl: {}",
            dyyl::i18n::t(lang, "ai.prepass_failed", &[("reason", &e.to_string())])
        );
        process::exit(2);
    }

    let source = match fs::read_to_string(&filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", dyyl::i18n::cannot_read_file(lang, &filename, &e));
            process::exit(1);
        }
    };

    // 保留原始 filename 用于 cli.script_name(basename 提取在 handler 端)
    let script_name = filename.clone();
    if host_json {
        let host = Arc::new(StdioHostConnection::new());
        let output = run_script_with_lang_and_host_and_args(
            &source,
            debug,
            lang,
            Some(host),
            script_args,
            script_name,
        );
        if !output.error.is_empty() {
            eprintln!("dyyl: {}", output.error);
            process::exit(1);
        }
    } else {
        run_script_with_lang_and_args(&source, debug, lang, script_args, script_name);
    }
}
