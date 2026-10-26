//! vroxgw — шлюз vrox.vpn без UI: поднимает тоннель hysteria2 по
//! подписке и пускает через него трафик клиентов LAN (см. docs/GATEWAY.md).

mod cli;
mod daemon;
mod net;
mod settings;
mod shutdown;

use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("run") => daemon::run().await,
        Some("status") => cli::status(),
        Some("nodes") => cli::nodes(),
        Some("switch") => cli::switch(args.get(1).map(String::as_str)),
        _ => {
            println!("{}", cli::USAGE);
            return ExitCode::SUCCESS;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vroxgw: {e}");
            ExitCode::FAILURE
        }
    }
}
