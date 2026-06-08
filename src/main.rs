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

fn main() {
    if dispatch_subcommand() {
        return;
    }

    let cli = Cli::parse();

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
