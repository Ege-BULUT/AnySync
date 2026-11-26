//! What AnySync knows how to move, by logical id, with each part's place on each system. A part is one slot: the
//! sender's path in slot i lands at the receiver's path in slot i. A system with no place for an item has an empty
//! list, and the item isn't offered to it.

use crate::os::{Env, Os};

pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub mac: &'static [&'static str],
    pub win: &'static [&'static str],
    pub linux: &'static [&'static str],
    pub secret: bool,
    /// For the agent finishing the setup.
    pub note: &'static str,
}

impl Entry {
    pub fn templates(&self, os: Os) -> &'static [&'static str] {
        match os {
            Os::Mac => self.mac,
            Os::Windows => self.win,
            Os::Linux => self.linux,
        }
    }

    /// (sender's path, receiver's path) per slot both systems have.
    pub fn pairs(&self, from: &Env, to: &Env) -> Vec<(String, String)> {
        let (a, b) = (self.templates(from.os), self.templates(to.os));
        a.iter().zip(b).filter_map(|(x, y)| Some((from.resolve(x)?, to.resolve(y)?))).collect()
    }
}

const fn e(id: &'static str, name: &'static str, mac: &'static [&'static str], win: &'static [&'static str], linux: &'static [&'static str], note: &'static str) -> Entry {
    Entry { id, name, mac, win, linux, secret: false, note }
}

const fn s(id: &'static str, name: &'static str, mac: &'static [&'static str], win: &'static [&'static str], linux: &'static [&'static str], note: &'static str) -> Entry {
    Entry { id, name, mac, win, linux, secret: true, note }
}

const VSCODE_NOTE: &str = "Extensions reinstall from the list in the transfer folder (`code --install-extension <id>`); settings that name paths of the old system need the new ones.";

pub const CATALOG: &[Entry] = &[
    // Shell and git
    e("dev/git", "Git config", &["~/.gitconfig", "~/.gitignore_global"], &["~/.gitconfig", "~/.gitignore_global"], &["~/.gitconfig", "~/.gitignore_global"],
        "Check `core.autocrlf`, `credential.helper` and any path in it: they differ between systems."),
    s("secrets/ssh", "SSH keys and config", &["~/.ssh"], &["~/.ssh"], &["~/.ssh"],
        "On macOS and Linux keep ~/.ssh at 700 and private keys at 600. `UseKeychain` in ssh_config is macOS only."),
    s("secrets/gnupg", "GnuPG", &["~/.gnupg"], &["%APPDATA%/gnupg"], &["~/.gnupg"], "On macOS and Linux keep ~/.gnupg at 700."),
    e("shell/zsh", "zsh config", &["~/.zshrc", "~/.zprofile"], &[], &["~/.zshrc", "~/.zprofile"], "Lines for tools of the old system (brew shellenv, paths) need the new system's."),
    e("shell/bash", "bash config", &["~/.bashrc", "~/.bash_profile"], &[], &["~/.bashrc", "~/.bash_profile"], ""),
    e("shell/powershell", "PowerShell profile", &["~/.config/powershell/Microsoft.PowerShell_profile.ps1"],
        &["~/Documents/PowerShell/Microsoft.PowerShell_profile.ps1"], &["$XDG_CONFIG_HOME/powershell/Microsoft.PowerShell_profile.ps1"], ""),
    // Editors
    e("editors/vscode", "VS Code settings", &["~/Library/Application Support/Code/User/settings.json", "~/Library/Application Support/Code/User/keybindings.json", "~/Library/Application Support/Code/User/snippets"],
        &["%APPDATA%/Code/User/settings.json", "%APPDATA%/Code/User/keybindings.json", "%APPDATA%/Code/User/snippets"],
        &["$XDG_CONFIG_HOME/Code/User/settings.json", "$XDG_CONFIG_HOME/Code/User/keybindings.json", "$XDG_CONFIG_HOME/Code/User/snippets"], VSCODE_NOTE),
    e("editors/cursor", "Cursor settings", &["~/Library/Application Support/Cursor/User/settings.json", "~/Library/Application Support/Cursor/User/keybindings.json", "~/Library/Application Support/Cursor/User/snippets"],
        &["%APPDATA%/Cursor/User/settings.json", "%APPDATA%/Cursor/User/keybindings.json", "%APPDATA%/Cursor/User/snippets"],
        &["$XDG_CONFIG_HOME/Cursor/User/settings.json", "$XDG_CONFIG_HOME/Cursor/User/keybindings.json", "$XDG_CONFIG_HOME/Cursor/User/snippets"], VSCODE_NOTE),
    e("editors/zed", "Zed settings", &["~/.config/zed/settings.json", "~/.config/zed/keymap.json"], &["%APPDATA%/Zed/settings.json", "%APPDATA%/Zed/keymap.json"],
        &["$XDG_CONFIG_HOME/zed/settings.json", "$XDG_CONFIG_HOME/zed/keymap.json"], ""),
    // Terminals
    e("terminals/wave", "Wave config", &["~/.config/waveterm"], &["~/.config/waveterm"], &["~/.config/waveterm"],
        "Widget commands and connections that name the old system's shells or paths need the new ones."),
    // Notes
    e("notes/obsidian", "Obsidian vault list", &["~/Library/Application Support/obsidian/obsidian.json"], &["%APPDATA%/obsidian/obsidian.json"],
        &["$XDG_CONFIG_HOME/obsidian/obsidian.json"], "Lists the vaults by absolute path: rewrite them to where the vault folders landed, and check each opens."),
    // Browsers: bookmarks only; saved passwords and cookies are locked to the old system's key store
    e("browsers/chrome", "Chrome bookmarks", &["~/Library/Application Support/Google/Chrome/Default/Bookmarks"],
        &["%LOCALAPPDATA%/Google/Chrome/User Data/Default/Bookmarks"], &["$XDG_CONFIG_HOME/google-chrome/Default/Bookmarks"], BROWSER_NOTE),
    e("browsers/brave", "Brave bookmarks", &["~/Library/Application Support/BraveSoftware/Brave-Browser/Default/Bookmarks"],
        &["%LOCALAPPDATA%/BraveSoftware/Brave-Browser/User Data/Default/Bookmarks"], &["$XDG_CONFIG_HOME/BraveSoftware/Brave-Browser/Default/Bookmarks"], BROWSER_NOTE),
    e("browsers/edge", "Edge bookmarks", &["~/Library/Application Support/Microsoft Edge/Default/Bookmarks"],
        &["%LOCALAPPDATA%/Microsoft/Edge/User Data/Default/Bookmarks"], &["$XDG_CONFIG_HOME/microsoft-edge/Default/Bookmarks"], BROWSER_NOTE),
    e("browsers/chromium", "Chromium bookmarks", &["~/Library/Application Support/Chromium/Default/Bookmarks"],
        &["%LOCALAPPDATA%/Chromium/User Data/Default/Bookmarks"], &["$XDG_CONFIG_HOME/chromium/Default/Bookmarks"], BROWSER_NOTE),
    // AI tools: settings, never the logins
    e("ai/claude-code", "Claude Code settings, skills, agents", &["~/.claude/settings.json", "~/.claude/CLAUDE.md", "~/.claude/skills", "~/.claude/agents", "~/.claude/commands"],
        &["~/.claude/settings.json", "~/.claude/CLAUDE.md", "~/.claude/skills", "~/.claude/agents", "~/.claude/commands"],
        &["~/.claude/settings.json", "~/.claude/CLAUDE.md", "~/.claude/skills", "~/.claude/agents", "~/.claude/commands"],
        "Install Claude Code, run `claude` and sign in. Hooks and statusline commands that call the old system's shell or paths need rewriting."),
    e("ai/codex", "Codex config", &["~/.codex/config.toml", "~/.codex/AGENTS.md"], &["~/.codex/config.toml", "~/.codex/AGENTS.md"], &["~/.codex/config.toml", "~/.codex/AGENTS.md"],
        "Install Codex and run `codex login`."),
];

const BROWSER_NOTE: &str = "Only the bookmarks cross systems: saved passwords and cookies are encrypted with a key the old system's key store holds. Quit the browser before placing; for passwords use the browser's own sync, or its password export and import.";

/// The person's own folders: each top-level item in them can be picked; the folder itself lands in the same
/// named place (macOS's Movies is Videos elsewhere).
pub const FOLDERS: &[(&str, &str, &str, &str)] = &[
    ("files/desktop", "~/Desktop", "~/Desktop", "~/Desktop"),
    ("files/documents", "~/Documents", "~/Documents", "~/Documents"),
    ("files/downloads", "~/Downloads", "~/Downloads", "~/Downloads"),
    ("files/pictures", "~/Pictures", "~/Pictures", "~/Pictures"),
    ("files/music", "~/Music", "~/Music", "~/Music"),
    ("files/videos", "~/Movies", "~/Videos", "~/Videos"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::env;
    use std::collections::HashSet;

    #[test]
    fn every_entry_is_whole() {
        let mut ids = HashSet::new();
        for e in CATALOG {
            assert!(ids.insert(e.id), "{} twice", e.id);
            let lens: Vec<usize> = [e.mac, e.win, e.linux].iter().map(|l| l.len()).filter(|n| *n > 0).collect();
            assert!(lens.windows(2).all(|w| w[0] == w[1]), "{}: systems with a place need the same slots", e.id);
            for os in [Os::Mac, Os::Windows, Os::Linux] {
                let here = env(os, "/h");
                for t in e.templates(os) {
                    assert!(here.resolve(t).is_some(), "{}: {t} names a root {} doesn't have", e.id, os.name());
                }
            }
            assert!(!e.note.contains('\u{2014}') && !e.note.contains('\u{2013}'));
        }
    }

    #[test]
    fn a_mac_item_lands_in_windows_own_place() {
        let mac = env(Os::Mac, "/Users/ege");
        let win = env(Os::Windows, r"C:\Users\ege");
        let lin = env(Os::Linux, "/home/ege");
        let vscode = CATALOG.iter().find(|e| e.id == "editors/vscode").unwrap();
        assert_eq!(vscode.pairs(&mac, &win)[0], ("/Users/ege/Library/Application Support/Code/User/settings.json".into(),
            r"C:\Users\ege\AppData\Roaming\Code\User\settings.json".into()));
        assert_eq!(vscode.pairs(&win, &lin)[2].1, "/home/ege/.config/Code/User/snippets");
        // zsh has no place on Windows: nothing to pair.
        let zsh = CATALOG.iter().find(|e| e.id == "shell/zsh").unwrap();
        assert!(zsh.pairs(&mac, &win).is_empty());
        assert_eq!(zsh.pairs(&mac, &lin).len(), 2);
    }
}
