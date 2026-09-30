//! Writes editor-integration files for Claude Code, Codex, Cursor, and GitHub
//! Copilot. Every editor gets the same portable `SKILL.md` (agentskills.io
//! format) telling the agent to shell out to `logreduce`; editors whose
//! `UserPromptSubmit` hook can inject context (Claude Code, Codex, VS Code
//! Copilot) also get a deterministic hook entry — this binary doubles as the
//! hook via `logreduce hook`. No servers, no Node, no npm.
//!
//! Skill locations: Claude Code reads `.claude/skills/`; Codex, Cursor, and
//! VS Code Copilot all read `.agents/skills/`. Codex, Cursor, and Copilot
//! also read `AGENTS.md`, which gets a one-line pointer to the skill.

use std::fs;
use std::io;
use std::path::Path;

const HOOK_COMMAND: &str = "logreduce hook";
const SKILL_TEMPLATE: &str = include_str!("../templates/SKILL.md");
const AGENTS_MD_TEMPLATE: &str = include_str!("../templates/agents-md.md");
const AGENTS_MD_MARKER: &str = "Reducing large log files with logreduce";

pub const EDITORS: &str = "claude-code | codex | cursor | copilot | all";

pub fn run(editor: &str, project_root: &Path) -> io::Result<()> {
    match editor {
        "claude-code" => install_claude_code(project_root),
        "codex" => install_codex(project_root),
        "cursor" => install_cursor(project_root),
        "copilot" => install_copilot(project_root),
        "all" => {
            install_claude_code(project_root)?;
            install_codex(project_root)?;
            install_cursor(project_root)?;
            install_copilot(project_root)
        }
        other => {
            eprintln!("Error: unknown editor '{other}' (expected: {EDITORS})");
            std::process::exit(1);
        }
    }
}

fn install_claude_code(project_root: &Path) -> io::Result<()> {
    let settings_path = project_root.join(".claude").join("settings.json");
    merge_nested_hook(&settings_path)?;
    println!("✓ Hook merged: {}", settings_path.display());
    write_skill(&project_root.join(".claude"))?;

    println!("Claude Code: start a new session to activate. Log paths in your prompt are reduced");
    println!("by the hook; logs Claude discovers mid-session are covered by the skill.\n");
    Ok(())
}

fn install_codex(project_root: &Path) -> io::Result<()> {
    let hooks_path = project_root.join(".codex").join("hooks.json");
    merge_nested_hook(&hooks_path)?;
    println!("✓ Hook merged: {}", hooks_path.display());
    write_skill(&project_root.join(".agents"))?;
    write_agents_md(project_root)?;

    println!("Codex: run `/hooks` once to review and trust the logreduce hook.\n");
    Ok(())
}

fn install_cursor(project_root: &Path) -> io::Result<()> {
    write_skill(&project_root.join(".agents"))?;
    write_agents_md(project_root)?;

    // Cursor's `beforeSubmitPrompt` hook can only allow or block a prompt,
    // not add context, so Cursor gets the skill alone.
    println!(
        "Cursor: reload to pick up the skill (Cursor has no context-injecting prompt hook).\n"
    );
    Ok(())
}

fn install_copilot(project_root: &Path) -> io::Result<()> {
    let hooks_dir = project_root.join(".github").join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hooks_path = hooks_dir.join("logreduce.json");
    write_json(
        &hooks_path,
        &serde_json::json!({
            "hooks": {
                "UserPromptSubmit": [{ "type": "command", "command": HOOK_COMMAND }]
            }
        }),
    )?;
    println!("✓ Hook written: {}", hooks_path.display());
    write_skill(&project_root.join(".agents"))?;
    write_agents_md(project_root)?;

    println!("Copilot: use Agent mode in VS Code Copilot Chat.\n");
    Ok(())
}

/// Writes `<base>/skills/logreduce/SKILL.md`.
fn write_skill(base: &Path) -> io::Result<()> {
    let skill_dir = base.join("skills").join("logreduce");
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, SKILL_TEMPLATE)?;
    println!("✓ Skill written: {}", skill_path.display());
    Ok(())
}

/// Appends the logreduce pointer section to `AGENTS.md` unless present.
fn write_agents_md(project_root: &Path) -> io::Result<()> {
    let dest = project_root.join("AGENTS.md");
    let mut content = String::new();
    if dest.exists() {
        content = fs::read_to_string(&dest)?;
        if content.contains(AGENTS_MD_MARKER) {
            println!("✓ AGENTS.md pointer already present: {}", dest.display());
            return Ok(());
        }
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
    }
    content.push_str(AGENTS_MD_TEMPLATE);
    fs::write(&dest, content)?;
    println!("✓ AGENTS.md pointer written: {}", dest.display());
    Ok(())
}

/// Merges our `UserPromptSubmit` entry into a Claude-Code-style hooks file
/// (`{ "hooks": { "UserPromptSubmit": [{ "matcher", "hooks": [...] }] } }`),
/// the format shared by `.claude/settings.json` and `.codex/hooks.json`.
/// Preserves every other key and entry; idempotent.
fn merge_nested_hook(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut settings = read_json_object(path);

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

    if !entries_arr.iter().any(hook_entry_matches) {
        entries_arr.push(serde_json::json!({
            "matcher": "",
            "hooks": [{ "type": "command", "command": HOOK_COMMAND }],
        }));
    }

    write_json(path, &serde_json::Value::Object(settings))
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

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    fn assert_skill(path: &Path) {
        let content = fs::read_to_string(path).unwrap();
        assert!(content.starts_with("---\nname: logreduce"));
        assert!(content.contains("logreduce <path-to-file>"));
    }

    #[test]
    fn claude_code_install_creates_hook_entry() {
        let dir = temp_project_dir();
        install_claude_code(&dir).unwrap();

        let settings = read_json(&dir.join(".claude/settings.json"));
        let entries = settings["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["hooks"][0]["command"], HOOK_COMMAND);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn claude_code_install_writes_skill_file() {
        let dir = temp_project_dir();
        install_claude_code(&dir).unwrap();
        assert_skill(&dir.join(".claude/skills/logreduce/SKILL.md"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn claude_code_install_is_idempotent() {
        let dir = temp_project_dir();
        install_claude_code(&dir).unwrap();
        install_claude_code(&dir).unwrap();

        let settings = read_json(&dir.join(".claude/settings.json"));
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

        let settings = read_json(&claude_dir.join("settings.json"));
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
    fn codex_install_writes_hook_skill_and_agents_md() {
        let dir = temp_project_dir();
        install_codex(&dir).unwrap();
        install_codex(&dir).unwrap();

        let hooks = read_json(&dir.join(".codex/hooks.json"));
        let entries = hooks["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["hooks"][0]["command"], HOOK_COMMAND);
        assert_skill(&dir.join(".agents/skills/logreduce/SKILL.md"));
        assert!(fs::read_to_string(dir.join("AGENTS.md"))
            .unwrap()
            .contains(AGENTS_MD_MARKER));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cursor_install_writes_skill_without_hook() {
        let dir = temp_project_dir();
        install_cursor(&dir).unwrap();

        assert_skill(&dir.join(".agents/skills/logreduce/SKILL.md"));
        assert!(!dir.join(".cursor").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copilot_install_writes_flat_hook_file() {
        let dir = temp_project_dir();
        install_copilot(&dir).unwrap();

        let hooks = read_json(&dir.join(".github/hooks/logreduce.json"));
        assert_eq!(
            hooks["hooks"]["UserPromptSubmit"][0]["command"],
            HOOK_COMMAND
        );
        assert_skill(&dir.join(".agents/skills/logreduce/SKILL.md"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn agents_md_pointer_is_appended_once() {
        let dir = temp_project_dir();
        fs::write(dir.join("AGENTS.md"), "# Existing\nkeep me").unwrap();

        run("all", &dir).unwrap();

        let content = fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(content.starts_with("# Existing\nkeep me\n\n"));
        assert_eq!(content.matches(AGENTS_MD_MARKER).count(), 1);

        fs::remove_dir_all(&dir).ok();
    }
}
