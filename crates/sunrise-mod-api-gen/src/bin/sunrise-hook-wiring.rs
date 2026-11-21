//! Run the catalog's read-only engine wiring check on a checkout.
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: sunrise-hook-wiring CHECKOUT");
        return ExitCode::FAILURE;
    };
    if let Err(error) = sunrise_mod_api_gen::check_hook_wiring(&root) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    println!("Native hook wiring check passed.");
    ExitCode::SUCCESS
}
