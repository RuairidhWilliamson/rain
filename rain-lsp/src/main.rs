#![allow(
    clippy::unwrap_used,
    clippy::dbg_macro,
    clippy::print_stderr,
    clippy::print_stdout
)]

mod comms;
mod json_rpc;
mod server;

use std::process::ExitCode;

use crate::comms::Comms;

fn main() -> ExitCode {
    let server = server::Server::new(Comms::new_stdin_stdout());
    server.run()
}
