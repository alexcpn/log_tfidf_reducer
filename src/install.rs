//! Writes editor-integration files for Claude Code, Cursor, and GitHub
//! Copilot — a deterministic hook entry for Claude Code (this binary doubles
//! as the hook via `logreduce hook`), and static rule/instruction files for
//! Cursor and Copilot that tell the agent to shell out to `logreduce`
//! directly. No servers, no Node, no npm.

use std::fs;
use std::io;
use std::path::Path;

const HOOK_COMMAND: &str = "logreduce hook";
const CURSOR_RULES_TEMPLATE: &str = include_str!("../templates/cursor-rules.mdc");
const COPILOT_INSTRUCTIONS_TEMPLATE: &str = include_str!("../templates/copilot-instructions.md");

pub fn run(editor: &str, project_root: &Path) -> io::Result<()> {
    match editor {
        "claude-code" => install_claude_code(project_root),
        "cursor" => install_cursor(project_root),
        "copilot" => install_copilot(project_root),
        other => {
            eprintln!("Error: unknown editor '{other}' (expected: claude-code | cursor | copilot)");
            std::process::exit(1);
        }
    }
}

fn install_claude_code(project_root: &Path) -> io::Result<()> {
    let claude_dir = project_root.join(".claude");
    let settings_path = claude_dir.join("settings.json");
    fs::create_dir_all(&claude_dir)?;

    let mut settings = read_json_object(&settings_path);

    let hooks = settings
        .entry("hooks".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !hooks.is_object() {
        *hooks = serde_json::json!({});
    }
    let hooks_obj = hooks.as_object_mut().unwrap();

    let entries = hooks_obj
        .entry("UserPromptSubmit".to_string())
        .or_insert_with(|| serde_json::json!([]));
    if !entries.is_array() {
        *entries = serde_json::json!([]);
    }
    let entries_arr = entries.as_array_mut().unwrap();

    let already_present = entries_arr.iter().any(hook_entry_matches);
    if !already_present {
        entries_arr.push(serde_json::json!({
            "matcher": "",
            "hooks": [{ "type": "command", "command": HOOK_COMMAND }],
        }));
    }

    write_json(&settings_path, &serde_json::Value::Object(settings))?;
    println!("✓ Settings merged: {}", settings_path.display());
    println!("\nClaude Code integration installed. Start a new session to activate.");
    println!("Any prompt with a log path or large inline log is now silently reduced before Claude reads it.");
    Ok(())
}

/// Matches an existing hook entry that already references our command, in
/// either the old (top-level `command`) or new (`hooks` array) settings format.
fn hook_entry_matches(entry: &serde_json::Value) -> bool {
    if entry.get("command").and_then(|c| c.as_str()) == Some(HOOK_COMMAND) {
        return true;
    }
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|nested| {
            nested
                .iter()
                .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(HOOK_COMMAND))
        })
        .unwrap_or(false)
}

fn install_cursor(project_root: &Path) -> io::Result<()> {
    let rules_dir = project_root.join(".cursor").join("rules");
    fs::create_dir_all(&rules_dir)?;
    let dest = rules_dir.join("logreduce.mdc");
    fs::write(&dest, CURSOR_RULES_TEMPLATE)?;
    println!("✓ Cursor rule written: {}", dest.display());
    println!("Reload Cursor — the agent will run `logreduce` on large logs before reading them.");
    Ok(())
}

fn install_copilot(project_root: &Path) -> io::Result<()> {
    let github_dir = project_root.join(".github");
    fs::create_dir_all(&github_dir)?;
    let dest = github_dir.join("copilot-instructions.md");

    let mut content = String::new();
    if dest.exists() {
        content = fs::read_to_string(&dest)?;
        if content.contains("Reducing large log files with logreduce") {
            println!("✓ Copilot instructions already present: {}", dest.display());
            println!("Switch to Agent mode in Copilot Chat to use it.");
            return Ok(());
        }
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
    }
    content.push_str(COPILOT_INSTRUCTIONS_TEMPLATE);
    fs::write(&dest, content)?;

    println!("✓ Copilot instructions written: {}", dest.display());
    println!("Switch to Agent mode in Copilot Chat to use it.");
    Ok(())
}

/// Reads a JSON object from `path`, returning an empty object if the file is
/// missing or not a valid JSON object (warning on the latter, to avoid
/// silently discarding a user's existing config).
fn read_json_object(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    if !path.exists() {
        return serde_json::Map::new();
    }
    match fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        Some(serde_json::Value::Object(map)) => map,
        Some(_) | None => {
            eprintln!(
                "Warning: existing {} is not a valid JSON object — overwriting",
                path.display()
            );
            serde_json::Map::new()
        }
    }
}

fn write_json(path: &Path, value: &serde_json::Value) -> io::Result<()> {
    let pretty = serde_json::to_string_pretty(value).expect("JSON serialization cannot fail here");
    fs::write(path, format!("{pretty}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_project_dir() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("logreduce-install-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn claude_code_install_creates_hook_entry() {
        let dir = temp_project_dir();
        install_claude_code(&dir).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join(".claude/settings.json")).unwrap())
                .unwrap();
        let entries = settings["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["hooks"][0]["command"], HOOK_COMMAND);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn claude_code_install_is_idempotent() {
        let dir = temp_project_dir();
        install_claude_code(&dir).unwrap();
        install_claude_code(&dir).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join(".claude/settings.json")).unwrap())
                .unwrap();
        let entries = settings["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(
            entries.len(),
            1,
            "running install twice must not duplicate the hook entry"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn claude_code_install_preserves_existing_settings() {
        let dir = temp_project_dir();
        let claude_dir = dir.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("settings.json"),
            r#"{"otherSetting": "keepme", "hooks": {"UserPromptSubmit": [{"matcher": "", "hooks": [{"type": "command", "command": "echo unrelated"}]}]}}"#,
        )
        .unwrap();

        install_claude_code(&dir).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(claude_dir.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["otherSetting"], "keepme");
        let entries = settings["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(
            entries.len(),
            2,
            "existing unrelated hook entries must be preserved"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cursor_install_writes_rules_file() {
        let dir = temp_project_dir();
        install_cursor(&dir).unwrap();

        let content = fs::read_to_string(dir.join(".cursor/rules/logreduce.mdc")).unwrap();
        assert!(content.contains("logreduce <path-to-file>"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copilot_install_writes_instructions_file() {
        let dir = temp_project_dir();
        install_copilot(&dir).unwrap();

        let content = fs::read_to_string(dir.join(".github/copilot-instructions.md")).unwrap();
        assert!(content.contains("Reducing large log files with logreduce"));

        // Re-running must not duplicate the section.
        install_copilot(&dir).unwrap();
        let content2 = fs::read_to_string(dir.join(".github/copilot-instructions.md")).unwrap();
        assert_eq!(
            content2
                .matches("Reducing large log files with logreduce")
                .count(),
            1
        );

        fs::remove_dir_all(&dir).ok();
    }
}
