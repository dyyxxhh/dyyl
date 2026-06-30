//! Command dispatch — thin router + per-family handler modules.

pub(crate) mod containers;
pub(crate) mod context;
pub(crate) mod dict_handlers;
pub(crate) mod dispatch;
pub(crate) mod file;
pub(crate) mod helpers;
pub(crate) mod io;
pub(crate) mod list_handlers;
pub(crate) mod list_query;
pub(crate) mod list_transform;
pub(crate) mod logic;
pub(crate) mod mcm;
pub(crate) mod math;
pub(crate) mod math_char;
pub(crate) mod math_hash;
pub mod net;
pub(crate) mod str;
pub(crate) mod str_basic;
pub(crate) mod str_convert;
pub(crate) mod str_modify;
pub(crate) mod str_regex;
pub(crate) mod str_split_join;
pub(crate) mod system;
pub(crate) mod time_cmd;
pub(crate) mod user;
pub(crate) mod vars;
