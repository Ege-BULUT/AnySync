mod bus;
mod catalog;
mod me;
mod os;
mod seal;
mod wire;

use std::path::Path;

fn main() {
    let env = os::Env::here();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--scan") => scan(&env),
        _ => println!("AnySync on {} ({})\n\n  anysync --scan    what this machine could send", env.os.name(), env.home),
    }
}

/// The catalogue's items found here, and the person's folders.
fn scan(env: &os::Env) {
    println!("{} ({})", env.os.name(), env.home);
    for e in catalog::CATALOG {
        let here: Vec<String> = e.templates(env.os).iter().filter_map(|t| env.resolve(t)).filter(|p| Path::new(p).exists()).collect();
        if !here.is_empty() {
            println!("  {:<22} {}{}", e.id, e.name, if e.secret { " [secret]" } else { "" });
            for p in here {
                println!("      {p}");
            }
        }
    }
    for (id, mac, win, linux) in catalog::FOLDERS {
        let t = match env.os { os::Os::Mac => mac, os::Os::Windows => win, os::Os::Linux => linux };
        if let Some(p) = env.resolve(t) {
            let n = std::fs::read_dir(&p).map(|d| d.filter_map(Result::ok).filter(|f| !f.file_name().to_string_lossy().starts_with('.')).count()).unwrap_or(0);
            println!("  {id:<22} {n} items in {p}");
        }
    }
}
