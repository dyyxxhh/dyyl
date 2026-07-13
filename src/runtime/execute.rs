use std::sync::Arc;

use crate::i18n::Lang;
use crate::parser::types::ParsedCommand;
use crate::runtime::cmd::context::ExecContext;
use crate::runtime::cmd::dispatch::dispatch_call;
use crate::runtime::env::Env;
use crate::runtime::error::{debug_diagnostic, error_to_sentinel, RuntimeError};
use crate::runtime::host_provider::HostProvider;
use crate::runtime::io_provider::{IoProvider, StdIoProvider};
use crate::runtime::value::Value;

#[path = "exec_block.rs"]
mod exec_block;

#[derive(Debug)]
pub struct ScriptOutput {
    pub values: Vec<Value>,
}

#[derive(Debug)]
pub struct ScriptOutputWithError {
    pub values: Vec<Value>,
    pub error: String,
}

pub fn run_script(source: &str, debug: bool) -> ScriptOutput {
    run_script_with_provider(source, debug, Arc::new(StdIoProvider))
}

pub fn run_script_with_lang(source: &str, debug: bool, lang: crate::i18n::Lang) -> ScriptOutput {
    let commands = match crate::parser::parse_source(source) {
        Ok(cmds) => cmds,
        Err(err) => {
            println!("-1");
            if debug {
                let source_lines: Vec<&str> = source.lines().collect();
                let cmd = match source_lines.get(err.line.saturating_sub(1)) {
                    Some(line) => *line,
                    None => "<end of file>",
                };
                eprintln!("line {}: {}", err.line, cmd);
                eprintln!("{}", err.message);
            }
            return ScriptOutput {
                values: vec![Value::Num(-1)],
            };
        }
    };
    let mut env = Env::new();
    env.set_lang(lang);
    let mut values = Vec::new();
    let provider: Arc<dyn IoProvider> = Arc::new(StdIoProvider);
    exec_commands_range(
        &commands,
        0,
        commands.len(),
        &mut env,
        &mut values,
        debug,
        &provider,
    );
    ScriptOutput { values }
}

pub fn run_script_with_lang_and_host(
    source: &str,
    debug: bool,
    lang: Lang,
    host: Option<Arc<dyn HostProvider>>,
) -> ScriptOutputWithError {
    let commands = match crate::parser::parse_source(source) {
        Ok(cmds) => cmds,
        Err(err) => {
            let error = if debug {
                let source_lines: Vec<&str> = source.lines().collect();
                let cmd = match source_lines.get(err.line.saturating_sub(1)) {
                    Some(line) => *line,
                    None => "<end of file>",
                };
                format!("line {}: {}\n{}", err.line, cmd, err.message)
            } else {
                err.message
            };
            return ScriptOutputWithError {
                values: vec![Value::Num(-1)],
                error,
            };
        }
    };
    let mut env = Env::new();
    env.set_lang(lang);
    if let Some(h) = host {
        env.set_host_provider(h);
    }
    let mut values = Vec::new();
    let provider: Arc<dyn IoProvider> = Arc::new(StdIoProvider);
    let mut error = String::new();
    exec_commands_range_with_error(
        &commands,
        0,
        commands.len(),
        &mut env,
        &mut values,
        debug,
        &provider,
        &mut error,
    );
    ScriptOutputWithError { values, error }
}

pub fn run_script_with_provider(
    source: &str,
    debug: bool,
    io_provider: Arc<dyn IoProvider>,
) -> ScriptOutput {
    let commands = match crate::parser::parse_source(source) {
        Ok(cmds) => cmds,
        Err(err) => {
            println!("-1");
            if debug {
                let source_lines: Vec<&str> = source.lines().collect();
                let cmd = match source_lines.get(err.line.saturating_sub(1)) {
                    Some(line) => *line,
                    None => "<end of file>",
                };
                eprintln!("line {}: {}", err.line, cmd);
                eprintln!("{}", err.message);
            }
            return ScriptOutput {
                values: vec![Value::Num(-1)],
            };
        }
    };
    run_commands_with_provider(&commands, debug, &io_provider)
}

pub fn run_commands(commands: &[ParsedCommand], debug: bool) -> ScriptOutput {
    let provider: Arc<dyn IoProvider> = Arc::new(StdIoProvider);
    run_commands_with_provider(commands, debug, &provider)
}

fn run_commands_with_provider(
    commands: &[ParsedCommand],
    debug: bool,
    io_provider: &Arc<dyn IoProvider>,
) -> ScriptOutput {
    let mut env = Env::new();
    let mut values = Vec::new();
    exec_commands_range(
        commands,
        0,
        commands.len(),
        &mut env,
        &mut values,
        debug,
        io_provider,
    );
    ScriptOutput { values }
}

fn exec_commands_range(
    commands: &[ParsedCommand],
    start: usize,
    count: usize,
    env: &mut Env,
    values: &mut Vec<Value>,
    debug: bool,
    io_provider: &Arc<dyn IoProvider>,
) -> usize {
    let total = commands.len();
    let end = total.min(start + count);
    let mut i = start;
    let mut prev_if_was_false = false;

    while i < end {
        let cmd = &commands[i];
        let consumed = exec_one_command(
            cmd,
            env,
            values,
            &mut prev_if_was_false,
            commands,
            i,
            end,
            debug,
            io_provider,
        );
        i += consumed;
    }

    i - start
}

fn exec_one_command(
    cmd: &ParsedCommand,
    env: &mut Env,
    values: &mut Vec<Value>,
    prev_if_was_false: &mut bool,
    commands: &[ParsedCommand],
    i: usize,
    end: usize,
    debug: bool,
    io_provider: &Arc<dyn IoProvider>,
) -> usize {
    match cmd.call.command.as_str() {
        "logic.if" | "logic.else" | "logic.while" | "logic.for" => exec_block::exec_block_cmd(
            cmd,
            env,
            values,
            prev_if_was_false,
            commands,
            i,
            end,
            debug,
            io_provider,
        ),
        _ => {
            let ctx = ExecContext::from_command(cmd, debug, Arc::clone(io_provider), env.lang());
            let result = dispatch_call(&cmd.call, env, &ctx);
            push_result(result, cmd, values, debug, env.lang());
            1
        }
    }
}

fn push_result(
    result: Result<Value, RuntimeError>,
    cmd: &ParsedCommand,
    values: &mut Vec<Value>,
    debug: bool,
    lang: Lang,
) {
    match result {
        Ok(val) => values.push(val),
        Err(err) => {
            if debug {
                debug_diagnostic(&err, &cmd.text, lang);
            }
            let sentinel = error_to_sentinel(&err);
            println!("{sentinel}");
            values.push(sentinel);
        }
    }
}

fn exec_commands_range_with_error(
    commands: &[ParsedCommand],
    start: usize,
    count: usize,
    env: &mut Env,
    values: &mut Vec<Value>,
    debug: bool,
    io_provider: &Arc<dyn IoProvider>,
    error: &mut String,
) -> usize {
    let total = commands.len();
    let end = total.min(start + count);
    let mut i = start;
    let mut prev_if_was_false = false;

    while i < end {
        let cmd = &commands[i];
        let consumed = exec_one_command_with_error(
            cmd,
            env,
            values,
            &mut prev_if_was_false,
            commands,
            i,
            end,
            debug,
            io_provider,
            error,
        );
        i += consumed;
    }

    i - start
}

fn exec_one_command_with_error(
    cmd: &ParsedCommand,
    env: &mut Env,
    values: &mut Vec<Value>,
    prev_if_was_false: &mut bool,
    commands: &[ParsedCommand],
    i: usize,
    end: usize,
    debug: bool,
    io_provider: &Arc<dyn IoProvider>,
    error: &mut String,
) -> usize {
    match cmd.call.command.as_str() {
        "logic.if" | "logic.else" | "logic.while" | "logic.for" => exec_block::exec_block_cmd(
            cmd,
            env,
            values,
            prev_if_was_false,
            commands,
            i,
            end,
            debug,
            io_provider,
        ),
        _ => {
            let ctx = ExecContext::from_command(cmd, debug, Arc::clone(io_provider), env.lang());
            let result = dispatch_call(&cmd.call, env, &ctx);
            push_result_with_error(result, cmd, values, debug, env.lang(), error);
            1
        }
    }
}

fn push_result_with_error(
    result: Result<Value, RuntimeError>,
    cmd: &ParsedCommand,
    values: &mut Vec<Value>,
    debug: bool,
    lang: Lang,
    error: &mut String,
) {
    match result {
        Ok(val) => values.push(val),
        Err(err) => {
            if debug {
                debug_diagnostic(&err, &cmd.text, lang);
            }
            let sentinel = error_to_sentinel(&err);
            println!("{sentinel}");
            values.push(sentinel);
            if error.is_empty() {
                *error = err.to_string();
            }
        }
    }
}

#[cfg(test)]
#[path = "execute_tests.rs"]
mod tests;
