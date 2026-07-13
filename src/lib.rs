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
