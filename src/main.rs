use clap::Parser;
use logreduce::{hook, install, reduce, BlendWeights, LogFormat, ReduceConfig};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

/// Dispatches `hook` and `install` subcommands before falling through to the
/// default `reduce` behavior — kept outside clap's derive so that a log file
/// literally named `hook` or `install` still works as a positional `input`.
fn dispatch_subcommand() -> bool {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("hook") => {
            hook::run();
            true
        }
        Some("install") => {
            run_install(&args[2..]);
            true
        }
        _ => false,
    }
}

fn run_install(args: &[String]) {
    let mut editor: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(value) = arg.strip_prefix("--editor=") {
            editor = Some(value.to_string());
        } else if arg == "--editor" {
            i += 1;
            editor = args.get(i).cloned();
        } else {
            eprintln!("Error: unknown argument to `install`: {arg}");
            process::exit(1);
        }
        i += 1;
    }

    let Some(editor) = editor else {
        eprintln!("Error: `install` requires --editor=<claude-code|cursor|copilot>");
        process::exit(1);
    };

    let project_root = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("Error: cannot determine current directory: {e}");
        process::exit(2);
    });

    if let Err(e) = install::run(&editor, &project_root) {
        eprintln!("Error: {e}");
        process::exit(2);
    }
}

#[derive(Parser)]
#[command(
    name = "logreduce",
    version,
    about = "Reduce noisy logs to an LLM-ready file",
    after_help = "Editor integration (no Node/npm — single static binary):\n  \
                  logreduce hook                                 Claude Code UserPromptSubmit hook (reads/writes JSON on stdio)\n  \
                  logreduce install --editor=<claude-code|cursor|copilot>   Write the integration files for an editor"
)]
struct Cli {
    /// Input log file (omit or use - for stdin)
    input: Option<PathBuf>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    out: Option<PathBuf>,

    /// Token budget ceiling (approximate; tiktoken cl100k_base)
    #[arg(short, long, default_value = "8000")]
    budget: u32,

    /// Alternative line-count cap (tightest of budget/max-lines wins)
    #[arg(long)]
    max_lines: Option<u32>,

    /// Context lines to keep around each kept line (0 = disable)
    #[arg(short, long, default_value = "2")]
    context: u8,

    /// Disable secret/PII redaction (redaction is ON by default)
    #[arg(long, default_value_t = false)]
    no_redact: bool,

    /// Force log format: auto | json | logfmt | syslog | klog | plain
    #[arg(long, default_value = "auto")]
    format: String,

    /// Blended-score weights as r,s,f (default: 0.4,0.5,0.1)
    #[arg(long, default_value = "0.4,0.5,0.1")]
    weights: String,

    /// Print reduction stats to stderr
    #[arg(long, default_value_t = false)]
    stats: bool,
}

fn parse_weights(s: &str) -> Result<BlendWeights, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err("weights must be r,s,f (three floats)".to_string());
    }
    let parse =
        |p: &str| -> Result<f32, String> { p.trim().parse::<f32>().map_err(|e| e.to_string()) };
    Ok(BlendWeights {
        rarity: parse(parts[0])?,
        severity: parse(parts[1])?,
        burst: parse(parts[2])?,
    })
}

fn parse_format(s: &str) -> LogFormat {
    match s.to_lowercase().as_str() {
        "json" => LogFormat::Json,
        "logfmt" => LogFormat::Logfmt,
        "syslog" => LogFormat::Syslog,
        "klog" | "glog" => LogFormat::Klog,
        "plain" => LogFormat::Plain,
        _ => LogFormat::Auto,
    }
}

/// Flags that consume the following token as their value (when not given as
/// `--flag=value`). Needed so unquoted-path recovery doesn't swallow flag
/// values into the joined path.
const VALUE_FLAGS: &[&str] = &[
    "-o",
    "--out",
    "-b",
    "--budget",
    "--max-lines",
    "-c",
    "--context",
    "--format",
    "--weights",
];

/// Agents (and shells with careless quoting) sometimes pass a path containing
/// spaces as several bare arguments, e.g. `logreduce logs/foo 12:30:00.log`
/// instead of `logreduce "logs/foo 12:30:00.log"`. clap then rejects the
/// extra positional as `unexpected argument`. Recover by joining each run of
/// consecutive non-flag tokens with spaces and using it as a single argument
/// if — and only if — that joined string is an existing path; this can't
/// misfire on legitimate multi-positional usage since the CLI has exactly one
/// positional argument.
fn recover_unquoted_input_path(args: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(args.len());
    result.push(args[0].clone());

    let mut run_start: Option<usize> = None;
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with('-') {
            if let Some(start) = run_start.take() {
                join_run_if_existing_path(&mut result, start);
            }
            result.push(arg.clone());
            if VALUE_FLAGS.contains(&arg.as_str()) && !arg.contains('=') {
                i += 1;
                if i < args.len() {
                    result.push(args[i].clone());
                }
            }
        } else {
            if run_start.is_none() {
                run_start = Some(result.len());
            }
            result.push(arg.clone());
        }
        i += 1;
    }
    if let Some(start) = run_start {
        join_run_if_existing_path(&mut result, start);
    }
    result
}

fn join_run_if_existing_path(result: &mut Vec<String>, start: usize) {
    if result.len() - start < 2 {
        return;
    }
    let joined = result[start..].join(" ");
    if std::path::Path::new(&joined).exists() {
        result.truncate(start);
        result.push(joined);
    }
}

fn main() {
    if dispatch_subcommand() {
        return;
    }

    let cli = Cli::parse_from(recover_unquoted_input_path(std::env::args().collect()));

    let weights = match parse_weights(&cli.weights) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error: invalid --weights: {e}");
            process::exit(1);
        }
    };

    let config = ReduceConfig {
        input: cli.input.clone(),
        output: cli.out.clone(),
        budget_tokens: cli.budget,
        max_lines: cli.max_lines,
        context_window: cli.context,
        weights,
        redact: !cli.no_redact,
        format: parse_format(&cli.format),
        show_stats: cli.stats,
    };

    // Read input
    let raw = if cli
        .input
        .as_deref()
        .map(|p| p.to_str() == Some("-"))
        .unwrap_or(true)
        && cli.input.is_none()
    {
        let mut buf = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buf) {
            eprintln!("Error reading stdin: {e}");
            process::exit(2);
        }
        buf
    } else {
        match std::fs::read_to_string(cli.input.as_ref().unwrap()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading input file: {e}");
                process::exit(2);
            }
        }
    };

    let result = reduce(&config, &raw);

    // Write output
    let output = format!("{}\n{}", result.header, result.body);
    if let Some(ref path) = cli.out {
        if let Err(e) = std::fs::write(path, &output) {
            eprintln!("Error writing output: {e}");
            process::exit(2);
        }
    } else {
        print!("{output}");
    }

    if cli.stats {
        eprintln!(
            "[logreduce] {inp} lines → {kept} kept | tokens {it} → {ot} ({ratio:.1}%)",
            inp = result.input_lines,
            kept = result.kept_lines,
            it = result.input_tokens,
            ot = result.output_tokens,
            ratio = if result.input_tokens > 0 {
                result.output_tokens as f64 / result.input_tokens as f64 * 100.0
            } else {
                0.0
            },
        );
        eprintln!("[logreduce] Note: token counts use tiktoken cl100k_base — slightly conservative for Claude.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file_with_space(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, "log content").unwrap();
        path
    }

    #[test]
    fn joins_unquoted_path_with_spaces_when_it_exists() {
        let name = format!("logreduce-recover-test-{} 12:30:00.log", std::process::id());
        let path = temp_file_with_space(&name);
        let dir = path.parent().unwrap().display().to_string();
        let stem = format!("logreduce-recover-test-{} 12:30:00.log", std::process::id());

        let args: Vec<String> = vec!["logreduce".into(), format!("{dir}/{stem}")]
            .iter()
            .flat_map(|s| s.split(' ').map(String::from))
            .collect();
        assert!(args.len() > 2, "test setup must produce a split path");

        let recovered = recover_unquoted_input_path(args);
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[1], format!("{dir}/{stem}"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn leaves_args_untouched_when_joined_path_does_not_exist() {
        let args: Vec<String> = vec![
            "logreduce".to_string(),
            "no".to_string(),
            "such".to_string(),
            "file.log".to_string(),
        ];
        let recovered = recover_unquoted_input_path(args.clone());
        assert_eq!(recovered, args);
    }

    #[test]
    fn does_not_swallow_flag_values_into_joined_path() {
        let args: Vec<String> = vec![
            "logreduce".to_string(),
            "app.log".to_string(),
            "--budget".to_string(),
            "32000".to_string(),
            "--stats".to_string(),
        ];
        let recovered = recover_unquoted_input_path(args.clone());
        assert_eq!(recovered, args);
    }
}
