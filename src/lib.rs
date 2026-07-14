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
/// dyyl — scripting language interpreter.
///
/// This crate provides a binary (`dyyl <filename>`) for executing dyyl scripts.
/// The library defines the CAS backend, lexer, parser, math/CAS layer, and runtime.
pub mod ai;
pub mod cas_backend;
pub mod cli;
pub mod config;
pub mod credentials;
pub mod i18n;
pub mod lexer;
pub mod math;
pub mod parser;
pub mod prepass;
pub mod runtime;

pub use runtime::host_provider::{
    GameChooseScope, HostError, HostProvider, McmArg, McmCommand, McmResponse, MockHostProvider,
    StdioHostConnection,
};
