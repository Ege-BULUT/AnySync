mod bus;
mod catalog;
mod cli;
mod me;
mod os;
mod seal;
mod session;
mod wire;

const HELP: &str = "AnySync: move your setup between macOS, Windows and Linux

  anysync --scan                        what this device could send
  anysync --give <id> [--only a,b]      ask device <id> to receive, then send
  anysync --accept [--only a,b]         approve the request waiting here, then do this side
  anysync --id                          this device's id, name and look";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let only: Vec<String> = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let r = match args.first().map(String::as_str) {
        Some("--scan") => {
            let o = session::offer(&os::Env::here());
            println!("{} ({})", o.os, o.home);
            for i in &o.items {
                println!("  {:<40} {}{}", i.id, i.name, if i.secret { " [secret]" } else { "" });
            }
            Ok(())
        }
        Some("--give") => match args.get(1) {
            Some(peer) => cli::give(peer, only),
            None => Err("--give needs a device id".into()),
        },
        Some("--accept") => cli::accept(only),
        Some("--id") => {
            let c = me::Config::load();
            println!("{} #{} {:?}", c.name, c.id, c.look);
            Ok(())
        }
        _ => {
            println!("{HELP}");
            Ok(())
        }
    };
    if let Err(e) = r {
        eprintln!("anysync: {e}");
        std::process::exit(1);
    }
}
