//! The MilkDrop application library: the CLI parser, the headless driver and
//! the `render` subcommand (F55: a library so integration tests can call it).
#![forbid(unsafe_code)]

pub mod cli;
pub mod headless;
pub mod render_cmd;
