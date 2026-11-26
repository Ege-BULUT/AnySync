//! One transfer between two devices, on the bus under `transfers/<time>_<sender>_<receiver>/`: the offer (sealed to
//! the receiver), the picks (sealed to the sender), the stream key (sealed to the receiver), and each side's state.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::bus::{self, Answer, Bus, Party, Request, Session};
use crate::catalog::{CATALOG, FOLDERS, Item};
use crate::os::{Env, Os};
use crate::{seal, wire};

/// What the sender offers: its system (`macOS`, `Windows`, `Linux`) and home folder (for the brief's path rewrites) and the items it has.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Offer {
    pub os: String,
    pub home: String,
    pub items: Vec<Item>,
}

/// The item ids the receiver takes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Picks {
    pub ids: Vec<String>,
}

/// Everything this device could send: the catalogue's items found here, and each top-level file or folder of the
/// person's own folders (hidden ones left out).
pub fn offer(env: &Env) -> Offer {
    let exists = |t: &String| env.resolve(t).is_some_and(|p| std::fs::symlink_metadata(p).is_ok());
    let mut items: Vec<Item> = CATALOG.iter().map(Item::from).filter(|i| i.templates(env.os).iter().any(exists)).collect();
    for (id, ..) in FOLDERS {
        let Some(root) = Item::folder_child(id, "x").and_then(|i| env.resolve(&i.templates(env.os)[0])).map(|p| PathBuf::from(p).parent().unwrap().to_path_buf()) else { continue };
        let mut kids: Vec<String> = std::fs::read_dir(&root).map(|d| d.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| !n.starts_with('.')).collect()).unwrap_or_default();
        kids.sort();
        items.extend(kids.iter().filter_map(|k| Item::folder_child(id, k)));
    }
    items.extend(crate::catalog::LISTS.iter().filter(|l| l.on.contains(&env.os) && crate::catalog::on_path(l.run[0])).map(Item::from));
    Offer { os: env.os.name().into(), home: env.home.clone(), items }
}

/// Whether `item` has a place on `os`; a list goes to any system.
pub fn fits(item: &Item, os: Os) -> bool {
    !item.run.is_empty() || !item.templates(os).is_empty()
}

/// The ids a receiver on `os` takes: those under any of `only` (an id or its start, "editors"), else all, and
/// never one without a place on its system.
pub fn picks(offer: &Offer, os: Os, only: &[String]) -> Picks {
    let wanted = |id: &str| only.is_empty() || only.iter().any(|p| id == p || id.starts_with(&format!("{p}/")));
    Picks { ids: offer.items.iter().filter(|i| fits(i, os) && wanted(&i.id)).map(|i| i.id.clone()).collect() }
}

/// Sealed files are stored as base64 text: GitHub's contents API reads some binary files as text in a guessed
/// charset and hands back other bytes.
pub fn put_sealed<T: Serialize>(b: &Bus, path: &str, to: &[u8; 32], t: &T, msg: &str) -> Result<(), String> {
    b.put(path, bus::b64(&seal::seal(to, &serde_json::to_vec(t).unwrap())).as_bytes(), None, msg).map(|_| ())
}

pub fn get_sealed<T: DeserializeOwned>(b: &Bus, path: &str, secret: &[u8; 32]) -> Result<Option<T>, String> {
    let Some((text, _)) = b.get(path)? else { return Ok(None) };
    let bytes = bus::unb64(&String::from_utf8_lossy(&text)).ok_or(format!("{path} is not base64"))?;
    let plain = seal::open(secret, &bytes)?;
    serde_json::from_slice(&plain).map(Some).map_err(|e| format!("{path}: {e}"))
}

/// Approves a request: makes the transfer folder with its session.json and records the answer.
pub fn approve(b: &Bus, mut r: Request, sha: &str, names: &std::collections::HashMap<String, String>) -> Result<Request, String> {
    let (sender, receiver) = r.sides();
    let at = bus::now();
    let dir = bus::transfer_dir(at, sender, receiver);
    let party = |id: &str| Party { id: id.into(), name: names.get(id).cloned().unwrap_or_default() };
    let s = Session { sender: party(sender), receiver: party(receiver), state: "offering".into(), started: at, started_utc: bus::utc(at), updated: at, ..Session::default() };
    b.put_json(&format!("{dir}/session.json"), &s, None, &format!("transfer {sender} to {receiver}"))?;
    r.transfer = dir;
    r.answer = Answer::Accepted;
    b.put_json(&r.path(), &r, Some(sha), &format!("Accepted {}", r.path()))?;
    Ok(r)
}

/// Changes session.json (the sender's document).
pub fn update_session(b: &Bus, transfer: &str, f: impl FnOnce(&mut Session)) -> Result<(), String> {
    let path = format!("{transfer}/session.json");
    let (mut s, sha) = b.get_json::<Session>(&path)?.ok_or("the transfer has no session.json")?;
    f(&mut s);
    s.updated = bus::now();
    b.put_json(&path, &s, Some(&sha), &format!("session {}", s.state)).map(|_| ())
}

/// The receiver's folder for one transfer: `~/AnySync/<transfer>`.
pub fn local_dir(env: &Env, transfer: &str) -> PathBuf {
    PathBuf::from(env.resolve("~/AnySync").unwrap_or_default()).join(transfer.trim_start_matches("transfers/"))
}

/// GOAL.md for the agent finishing the setup: what landed where, what didn't, and what to rewrite.
pub fn brief(offer: &Offer, here: &Env, from: &str, placed: &[(Item, Vec<String>, Vec<String>)]) -> String {
    let mut g = format!("# GOAL: make this {} device work like {from} ({})\n\n", here.os.name(), offer.os);
    if offer.home != here.home || offer.os != here.os.name() {
        g += &format!("The old home folder was `{}` on {}, this one is `{}` on {}. Configs that name the old home or its paths need this device's \
            (separators too when the systems differ); keep a copy of each file before changing it.\n\n", offer.home, offer.os, here.home, here.os.name());
    }
    g += "## Items\n\n";
    for (item, to, problems) in placed {
        g += &format!("### {}{}\n\n- id: `{}`\n", item.name, if item.secret { " (secret)" } else { "" }, item.id);
        for t in to {
            g += &format!("- placed: `{t}`\n");
        }
        for p in problems {
            g += &format!("- PROBLEM: {p}\n");
        }
        if !item.note.is_empty() {
            g += &format!("- to finish: {}\n", item.note);
        }
        g += "\n";
    }
    g += "## Done means\n\n- Every item above is checked and works, or is in TODO-FOR-YOU.md with steps.\n- REPORT.md says what was done per item.\n";
    g
}

/// Places every unpacked item and writes the brief in `dir`; returns (placed paths, problems).
pub fn finish(offer: &Offer, picks: &Picks, here: &Env, from: &str, dir: &Path) -> (usize, usize) {
    let staging = dir.join("staging");
    let placed: Vec<(Item, Vec<String>, Vec<String>)> = offer.items.iter().filter(|i| picks.ids.contains(&i.id)).map(|i| {
        let (to, problems) = wire::place(i, &staging, here);
        (i.clone(), to, problems)
    }).collect();
    let _ = std::fs::write(dir.join("GOAL.md"), brief(offer, here, from, &placed));
    (placed.iter().map(|p| p.1.len()).sum(), placed.iter().map(|p| p.2.len()).sum())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::env;

    #[test]
    fn a_windows_receiver_takes_only_what_it_has_a_place_for() {
        let mac = env(Os::Mac, "/Users/ege");
        let zsh = Item::from(CATALOG.iter().find(|e| e.id == "shell/zsh").unwrap());
        let vscode = Item::from(CATALOG.iter().find(|e| e.id == "editors/vscode").unwrap());
        let doc = Item::folder_child("files/documents", "Notes 2026").unwrap();
        let o = Offer { os: "macOS".into(), home: mac.home.clone(), items: vec![zsh, vscode, doc] };
        assert_eq!(picks(&o, Os::Windows, &[]).ids, ["editors/vscode", "files/documents/notes-2026"]);
        assert_eq!(picks(&o, Os::Linux, &["shell".into()]).ids, ["shell/zsh"]);
        assert_eq!(picks(&o, Os::Linux, &["files/documents".into()]).ids, ["files/documents/notes-2026"]);
        let g = brief(&o, &env(Os::Windows, r"C:\Users\ege"), "Ege's MacBook", &[]);
        assert!(g.contains("`/Users/ege` on macOS") && g.contains(r"`C:\Users\ege` on Windows"));
    }
}
