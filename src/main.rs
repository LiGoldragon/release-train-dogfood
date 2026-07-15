//! Drive the synchronizer release-train library end-to-end against the real
//! six-crate language-family stack.
//!
//! Usage: `release-train-dogfood [intent.nota] [checkout-root] [output-dir]`.
//! Defaults drive the primary-authored slice-three intent, clone into
//! `./checkouts`, and write portable artifacts into `./integration`.

use std::path::PathBuf;
use std::process::ExitCode;

use release_train_dogfood::DogfoodInvocation;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let intent_path = PathBuf::from(arguments.next().unwrap_or_else(|| {
        "/home/li/primary/release-trains/language-family-slice-three.nota".to_string()
    }));
    let checkout_root = PathBuf::from(arguments.next().unwrap_or_else(|| "checkouts".to_string()));
    let output_directory = PathBuf::from(
        arguments
            .next()
            .unwrap_or_else(|| "integration".to_string()),
    );

    let invocation =
        DogfoodInvocation::new(intent_path, checkout_root, output_directory, "LiGoldragon");
    match invocation.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("release-train-dogfood: {error}");
            ExitCode::from(1)
        }
    }
}
