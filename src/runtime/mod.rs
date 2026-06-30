//! dyyl runtime — value model, environment, execution engine.
//!
//! The runtime consumes the parser output and executes dyyl scripts.
//! Task 4 provides the minimal seams needed for the acceptance fixture;
//! full command families are added in later tasks.

pub mod cmd;
pub mod env;
pub mod error;
pub mod execute;
pub mod host_provider;
pub mod io_provider;
pub mod value;

pub use self::env::Env;
pub use self::error::{debug_diagnostic, error_to_sentinel, RuntimeError};
pub use self::execute::{run_script, run_script_with_provider, ScriptOutput, ScriptOutputWithError};
pub use self::host_provider::{
    GameChooseScope, HostError, HostProvider, McmArg, McmCommand, McmResponse, MockHostProvider,
    StdioHostConnection,
};
pub use self::io_provider::{IoError, IoProvider, MockIoProvider, StdIoProvider};
pub use self::value::Value;

pub use self::cmd::net::configure_agent_for_testing;
