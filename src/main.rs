mod server;

use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;

use context_engine::VaultIndex;

#[tokio::main]
async fn main() {
    if let Err(error) = run(std::env::args_os()).await {
        eprintln!("context: {error}");
        std::process::exit(1);
    }
}

async fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let command = arguments.next();
    let vault = arguments.next();
    let extra = arguments.next();

    match (command.as_deref(), vault, extra) {
        (Some(command), Some(vault), None) if command == "serve" => {
            let index = VaultIndex::build(PathBuf::from(vault))?;
            server::serve(index).await?;
            Ok(())
        }
        _ => Err("usage: context serve <vault-dir>".into()),
    }
}
