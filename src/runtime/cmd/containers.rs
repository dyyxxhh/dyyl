//! Container command handlers — dict.* and list.* family routers.
//!
//! Shared helpers and the two routers (`handle_dict_command`,
//! `handle_list_command`) live here.  Per-command handlers are in
//! `dict_handlers` and `list_handlers` / `list_transform`.

use super::context::ExecContext;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

use super::dict_handlers;
use super::list_handlers;
use super::list_query;
use super::list_transform;

/// Route a `dict.*` call to the appropriate handler.
pub(crate) fn handle_dict_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["dict.".len()..];
    match sub {
        "create" => dict_handlers::handle_dict_create(call, env, ctx),
        "set" => dict_handlers::handle_dict_set(call, env, ctx),
        "get" => dict_handlers::handle_dict_get(call, env, ctx),
        "has" => dict_handlers::handle_dict_has(call, env, ctx),
        "del" => dict_handlers::handle_dict_del(call, env, ctx),
        "keys" => dict_handlers::handle_dict_keys(call, env, ctx),
        "vals" => dict_handlers::handle_dict_vals(call, env, ctx),
        "len" => dict_handlers::handle_dict_len(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "dict", sub),
        )),
    }
}

/// Route a `list.*` call to the appropriate handler.
pub(crate) fn handle_list_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["list.".len()..];
    match sub {
        "create" => list_handlers::handle_list_create(call, env, ctx),
        "get" => list_handlers::handle_list_get(call, env, ctx),
        "len" => list_handlers::handle_list_len(call, env, ctx),
        "append" => list_handlers::handle_list_append(call, env, ctx),
        "insert" => list_handlers::handle_list_insert(call, env, ctx),
        "remove" => list_handlers::handle_list_remove(call, env, ctx),
        "contains" => list_query::handle_list_contains(call, env, ctx),
        "index" => list_query::handle_list_index(call, env, ctx),
        "join" => list_transform::handle_list_join(call, env, ctx),
        "reverse" => list_transform::handle_list_reverse(call, env, ctx),
        "sort" => list_transform::handle_list_sort(call, env, ctx),
        "slice" => list_transform::handle_list_slice(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "list", sub),
        )),
    }
}
