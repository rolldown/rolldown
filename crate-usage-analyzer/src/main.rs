use anyhow::Result;
use clap::Parser;
use colored::*;
use std::path::PathBuf;

mod analyzer;
mod ast_parser;
mod parallel_analyzer;
mod re_export_detector;
mod report;
mod symbol_graph;
mod trait_impl_tracker;
mod usage_analyzer;
mod workspace_resolver;

use analyzer::{CrateUsageAnalyzer, AnalysisResult};

#[derive(Parser, Debug)]
#[command(name = "crate-usage-analyzer")]
#[command(about = "Analyze crate usage in a Cargo workspace")]
#[command(version)]
struct Args {
    #[arg(short, long, help = "Path to the workspace root")]
    workspace: Option<PathBuf>,

    #[arg(short, long, help = "Entry crate name to start analysis from")]
    entry: Option<String>,

    #[arg(
        short = 'o',
        long,
        default_value = "text",
        help = "Output format: text, json, markdown, html"
    )]
    output_format: String,

    #[arg(short, long, help = "Output file path (default: stdout)")]
    output: Option<PathBuf>,

    #[arg(short, long, help = "Verbose output")]
    verbose: bool,

    #[arg(long, help = "Show only unused items")]
    only_unused: bool,

    #[arg(long, value_delimiter = ',', help = "Comma-separated list of crate names to ignore in analysis")]
    ignore_crates: Option<Vec<String>>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let workspace_path = args
        .workspace
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    println!(
        "{}",
        format!("🔍 Analyzing workspace at: {}", workspace_path.display())
            .cyan()
            .bold()
    );

    // 使用分析器
    run_analyzer(args, workspace_path)
}

fn run_analyzer(args: Args, workspace_path: PathBuf) -> Result<()> {
    let entry_crate = if let Some(entry) = args.entry {
        entry
    } else {
        // 自动检测入口 crate
        let workspace_info = workspace_resolver::WorkspaceInfo::new(&workspace_path)?;
        let entry_points = workspace_info.get_entry_points();
        
        if entry_points.is_empty() {
            anyhow::bail!("No entry crate found. Please specify one with --entry");
        }
        
        println!(
            "{}",
            format!("📦 Auto-detected entry crate: {}", entry_points[0])
                .green()
                .bold()
        );
        entry_points[0].clone()
    };

    println!(
        "{}",
        format!("📦 Starting from entry crate: {}", entry_crate)
            .green()
            .bold()
    );

    let mut analyzer = CrateUsageAnalyzer::new(workspace_path, entry_crate)?;
    
    if args.verbose {
        analyzer.set_verbose(true);
    }
    
    if let Some(ignore_crates) = args.ignore_crates {
        analyzer.set_ignored_crates(ignore_crates);
    }
    
    let analysis_result = analyzer.analyze()?;

    let report = report::generate_report(
        &analysis_result,
        &args.output_format,
        args.only_unused,
    )?;

    if let Some(output_path) = args.output {
        std::fs::write(&output_path, report)?;
        println!(
            "{}",
            format!("✅ Report saved to: {}", output_path.display())
                .green()
                .bold()
        );
    } else {
        println!("\n{}", "=".repeat(80).yellow());
        println!("{}", report);
    }

    Ok(())
}

