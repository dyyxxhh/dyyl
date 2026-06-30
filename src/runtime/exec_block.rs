use std::sync::Arc;

use crate::i18n;
use crate::parser::types::Expr;
use crate::parser::types::ParsedCommand;
use crate::runtime::cmd::context::ExecContext;
use crate::runtime::cmd::dispatch::eval_expr;
use crate::runtime::env::Env;
use crate::runtime::io_provider::IoProvider;
use crate::runtime::value::Value;

pub(super) fn exec_block_cmd(
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
    let cmd_name = &cmd.call.command;
    let body_lines = match cmd.call.args.get(1) {
        Some(Expr::Num(n)) if *n >= 0 => *n as usize,
        _ => 0,
    };
    let available = end.saturating_sub(i + 1);
    let underdeclared = body_lines > available;
    let execute_count = body_lines.min(available);
    let mut body_consumed: usize = 0;

    let cond_val = eval_first_arg(cmd, env, debug, io_provider);
    let cmd_result = match cmd_name.as_str() {
        "logic.if" => {
            let cond_truthy = cond_val != 0;
            *prev_if_was_false = !cond_truthy;
            if cond_truthy && !underdeclared && execute_count > 0 {
                body_consumed = super::exec_commands_range(
                    commands,
                    i + 1,
                    execute_count,
                    env,
                    values,
                    debug,
                    io_provider,
                );
            }
            Value::Num(if cond_truthy && !underdeclared && execute_count > 0 {
                1
            } else {
                0
            })
        }
        "logic.else" => {
            let cond_truthy = cond_val != 0;
            let fire = *prev_if_was_false && cond_truthy && !underdeclared && execute_count > 0;
            if fire {
                body_consumed = super::exec_commands_range(
                    commands,
                    i + 1,
                    execute_count,
                    env,
                    values,
                    debug,
                    io_provider,
                );
            }
            Value::Num(if fire { 1 } else { 0 })
        }
        "logic.while" => {
            let mut iterations: i64 = 0;
            if !underdeclared && execute_count > 0 {
                loop {
                    let cond = eval_first_arg(cmd, env, debug, io_provider);
                    if cond == 0 {
                        break;
                    }
                    let iter_consumed = super::exec_commands_range(
                        commands,
                        i + 1,
                        execute_count,
                        env,
                        values,
                        debug,
                        io_provider,
                    );
                    body_consumed = body_consumed.max(iter_consumed);
                    iterations += 1;
                }
            }
            Value::Num(iterations)
        }
        "logic.for" => {
            let loop_count = if cond_val > 0 { cond_val } else { 0 };
            let mut iterations: i64 = 0;
            if !underdeclared && execute_count > 0 {
                for _ in 0..loop_count {
                    let iter_consumed = super::exec_commands_range(
                        commands,
                        i + 1,
                        execute_count,
                        env,
                        values,
                        debug,
                        io_provider,
                    );
                    body_consumed = body_consumed.max(iter_consumed);
                    iterations += 1;
                }
            }
            Value::Num(iterations)
        }
        _ => Value::Empty,
    };

    if underdeclared {
        println!("{}", Value::Num(0));
        if debug {
            let lang = env.lang();
            eprintln!("line {}: {}", cmd.line, cmd.text);
            eprintln!(
                "{}{}",
                i18n::reason_prefix(lang),
                i18n::warn_block_underdeclared(lang, body_lines, available)
            );
        }
    }

    values.push(cmd_result);

    let skip = if underdeclared {
        body_lines
    } else if body_consumed > 0 {
        body_consumed
    } else {
        execute_count
    };
    1 + skip
}

fn eval_first_arg(
    cmd: &ParsedCommand,
    env: &mut Env,
    debug: bool,
    io_provider: &Arc<dyn IoProvider>,
) -> i64 {
    let ctx = ExecContext::from_command(cmd, debug, Arc::clone(io_provider), env.lang());
    match cmd.call.args.first() {
        Some(expr) => match eval_expr(expr, env, &ctx) {
            Ok(Value::Num(n)) => n,
            Ok(Value::Expr(e)) => {
                if e.is_zero() {
                    0
                } else {
                    e.to_f64() as i64
                }
            }
            _ => 0,
        },
        None => 0,
    }
}
