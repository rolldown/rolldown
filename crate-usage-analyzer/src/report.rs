use crate::analyzer::AnalysisResult;
use crate::symbol_graph::UnusedSymbol;
use anyhow::Result;
use colored::*;
use serde::Serialize;
use std::fmt::Write;

#[derive(Serialize)]
struct JsonReportV2 {
  summary: SummaryV2,
  unused_symbols: Vec<UnusedSymbolInfo>,
}

#[derive(Serialize)]
struct SummaryV2 {
  total_crates: usize,
  total_symbols: usize,
  used_symbols: usize,
  unused_symbols: usize,
  usage_percentage: f64,
  internal_only_usage: usize,
  external_only_usage: usize,
  re_exported: usize,
  pub_from_entry: usize,
}

#[derive(Serialize)]
struct UnusedSymbolInfo {
  symbol_key: String,
  crate_name: String,
  symbol_name: String,
  symbol_kind: String,
  file_path: String,
  line: usize,
  column: usize,
  is_public: bool,
  reason: String,
}

pub fn generate_report(result: &AnalysisResult, format: &str, only_unused: bool) -> Result<String> {
  match format {
    "json" => generate_json_report(result),
    "markdown" => generate_markdown_report(result, only_unused),
    "html" => generate_html_report(result, only_unused),
    _ => generate_text_report(result, only_unused),
  }
}

fn generate_json_report(result: &AnalysisResult) -> Result<String> {
  let stats = &result.statistics;
  let usage_percentage = if stats.total_symbols > 0 {
    (stats.used_symbols as f64 / stats.total_symbols as f64) * 100.0
  } else {
    100.0
  };

  let report = JsonReportV2 {
    summary: SummaryV2 {
      total_crates: result.total_crates,
      total_symbols: stats.total_symbols,
      used_symbols: stats.used_symbols,
      unused_symbols: stats.unused_symbols,
      usage_percentage,
      internal_only_usage: stats.internal_only,
      external_only_usage: stats.external_only,
      re_exported: stats.re_exported,
      pub_from_entry: stats.pub_from_entry,
    },
    unused_symbols: result
      .unused_symbols
      .iter()
      .map(|u| UnusedSymbolInfo {
        symbol_key: u.symbol_key.clone(),
        crate_name: u.crate_name.clone(),
        symbol_name: u.symbol.name.clone(),
        symbol_kind: format!("{:?}", u.symbol.kind),
        file_path: u.symbol.file_path.display().to_string(),
        line: u.symbol.line,
        column: u.symbol.column,
        is_public: u.symbol.is_public,
        reason: u.reason.clone(),
      })
      .collect(),
  };

  Ok(serde_json::to_string_pretty(&report)?)
}

fn generate_markdown_report(result: &AnalysisResult, _only_unused: bool) -> Result<String> {
  let mut output = String::new();
  let stats = &result.statistics;

  writeln!(output, "# Crate Usage Analysis Report (v2)")?;
  writeln!(output)?;

  writeln!(output, "## Summary")?;
  writeln!(output, "- **Total Crates Analyzed**: {}", result.total_crates)?;
  writeln!(output, "- **Total Symbols**: {}", stats.total_symbols)?;
  writeln!(
    output,
    "- **Used Symbols**: {} ({:.2}%)",
    stats.used_symbols,
    (stats.used_symbols as f64 / stats.total_symbols as f64) * 100.0
  )?;
  writeln!(
    output,
    "- **Unused Symbols**: {} ({:.2}%)",
    stats.unused_symbols,
    (stats.unused_symbols as f64 / stats.total_symbols as f64) * 100.0
  )?;
  writeln!(output)?;

  writeln!(output, "### Usage Breakdown")?;
  writeln!(output, "- **Internal Use Only**: {}", stats.internal_only)?;
  writeln!(output, "- **External Use Only**: {}", stats.external_only)?;
  writeln!(output, "- **Re-exported Symbols**: {}", stats.re_exported)?;
  writeln!(output, "- **Public from Entry**: {}", stats.pub_from_entry)?;
  writeln!(output)?;

  if !result.unused_symbols.is_empty() {
    writeln!(output, "## Unused Symbols")?;
    writeln!(output)?;

    // Group by crate
    let mut by_crate: rustc_hash::FxHashMap<String, Vec<&UnusedSymbol>> =
      rustc_hash::FxHashMap::default();

    for symbol in &result.unused_symbols {
      by_crate.entry(symbol.crate_name.clone()).or_default().push(symbol);
    }

    let mut crates: Vec<_> = by_crate.keys().cloned().collect();
    crates.sort();

    for crate_name in crates {
      if let Some(symbols) = by_crate.get(&crate_name) {
        writeln!(output, "### {}", crate_name)?;
        writeln!(output)?;

        for symbol in symbols {
          writeln!(output, "- **`{}`** ({:?})", symbol.symbol.name, symbol.symbol.kind)?;
          writeln!(
            output,
            "  - Location: `{}:{}:{}`",
            symbol.symbol.file_path.display(),
            symbol.symbol.line,
            symbol.symbol.column
          )?;
          writeln!(output, "  - Reason: {}", symbol.reason)?;
        }
        writeln!(output)?;
      }
    }
  } else {
    writeln!(output, "## ✅ No Unused Symbols Found!")?;
    writeln!(output)?;
    writeln!(output, "All symbols are either:")?;
    writeln!(output, "- Used internally within their crate")?;
    writeln!(output, "- Used by dependent crates")?;
    writeln!(output, "- Re-exported by other crates")?;
    writeln!(output, "- Part of the public API from the entry crate")?;
  }

  Ok(output)
}

fn generate_html_report(result: &AnalysisResult, _only_unused: bool) -> Result<String> {
  let mut output = String::new();
  let stats = &result.statistics;

  writeln!(output, "<!DOCTYPE html>")?;
  writeln!(output, "<html>")?;
  writeln!(output, "<head>")?;
  writeln!(output, "  <title>Crate Usage Analysis Report (v2)</title>")?;
  writeln!(output, "  <style>")?;
  writeln!(
    output,
    "    body {{ font-family: 'Segoe UI', Tahoma, Arial, sans-serif; margin: 20px; background: #f5f5f5; }}"
  )?;
  writeln!(
    output,
    "    .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 10px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}"
  )?;
  writeln!(
    output,
    "    h1 {{ color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 10px; }}"
  )?;
  writeln!(output, "    h2 {{ color: #34495e; margin-top: 30px; }}")?;
  writeln!(output, "    h3 {{ color: #7f8c8d; }}")?;
  writeln!(
    output,
    "    .summary {{ background: #ecf0f1; padding: 20px; border-radius: 8px; margin: 20px 0; }}"
  )?;
  writeln!(
    output,
    "    .stats-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; margin: 20px 0; }}"
  )?;
  writeln!(
    output,
    "    .stat-card {{ background: #fff; padding: 15px; border-radius: 5px; border-left: 4px solid #3498db; }}"
  )?;
  writeln!(output, "    .stat-value {{ font-size: 24px; font-weight: bold; color: #2c3e50; }}")?;
  writeln!(output, "    .stat-label {{ color: #7f8c8d; margin-top: 5px; }}")?;
  writeln!(
    output,
    "    .unused-symbol {{ background: #fff3cd; padding: 15px; margin: 10px 0; border-radius: 5px; border-left: 4px solid #ffc107; }}"
  )?;
  writeln!(output, "    .symbol-name {{ font-weight: bold; color: #d9534f; }}")?;
  writeln!(output, "    .symbol-details {{ margin-top: 8px; color: #666; font-size: 14px; }}")?;
  writeln!(
    output,
    "    .success {{ background: #d4edda; color: #155724; padding: 20px; border-radius: 5px; }}"
  )?;
  writeln!(output, "  </style>")?;
  writeln!(output, "</head>")?;
  writeln!(output, "<body>")?;
  writeln!(output, "  <div class='container'>")?;

  writeln!(output, "    <h1>🔍 Crate Usage Analysis Report (v2)</h1>")?;

  let usage_percentage = if stats.total_symbols > 0 {
    (stats.used_symbols as f64 / stats.total_symbols as f64) * 100.0
  } else {
    100.0
  };

  writeln!(output, "    <div class='summary'>")?;
  writeln!(output, "      <h2>📊 Summary</h2>")?;
  writeln!(output, "      <div class='stats-grid'>")?;

  writeln!(output, "        <div class='stat-card'>")?;
  writeln!(output, "          <div class='stat-value'>{}</div>", result.total_crates)?;
  writeln!(output, "          <div class='stat-label'>Total Crates</div>")?;
  writeln!(output, "        </div>")?;

  writeln!(output, "        <div class='stat-card'>")?;
  writeln!(output, "          <div class='stat-value'>{}</div>", stats.total_symbols)?;
  writeln!(output, "          <div class='stat-label'>Total Symbols</div>")?;
  writeln!(output, "        </div>")?;

  writeln!(output, "        <div class='stat-card'>")?;
  writeln!(output, "          <div class='stat-value'>{:.1}%</div>", usage_percentage)?;
  writeln!(output, "          <div class='stat-label'>Usage Rate</div>")?;
  writeln!(output, "        </div>")?;

  writeln!(output, "        <div class='stat-card'>")?;
  writeln!(output, "          <div class='stat-value'>{}</div>", stats.unused_symbols)?;
  writeln!(output, "          <div class='stat-label'>Unused Symbols</div>")?;
  writeln!(output, "        </div>")?;

  writeln!(output, "        <div class='stat-card'>")?;
  writeln!(output, "          <div class='stat-value'>{}</div>", stats.re_exported)?;
  writeln!(output, "          <div class='stat-label'>Re-exported</div>")?;
  writeln!(output, "        </div>")?;

  writeln!(output, "        <div class='stat-card'>")?;
  writeln!(output, "          <div class='stat-value'>{}</div>", stats.pub_from_entry)?;
  writeln!(output, "          <div class='stat-label'>Public from Entry</div>")?;
  writeln!(output, "        </div>")?;

  writeln!(output, "      </div>")?;
  writeln!(output, "    </div>")?;

  if !result.unused_symbols.is_empty() {
    writeln!(output, "    <h2>⚠️ Unused Symbols</h2>")?;

    let mut by_crate: rustc_hash::FxHashMap<String, Vec<&UnusedSymbol>> =
      rustc_hash::FxHashMap::default();

    for symbol in &result.unused_symbols {
      by_crate.entry(symbol.crate_name.clone()).or_default().push(symbol);
    }

    let mut crates: Vec<_> = by_crate.keys().cloned().collect();
    crates.sort();

    for crate_name in crates {
      if let Some(symbols) = by_crate.get(&crate_name) {
        writeln!(output, "    <h3>📦 {}</h3>", crate_name)?;

        for symbol in symbols {
          writeln!(output, "    <div class='unused-symbol'>")?;
          writeln!(
            output,
            "      <div class='symbol-name'>{} ({:?})</div>",
            symbol.symbol.name, symbol.symbol.kind
          )?;
          writeln!(output, "      <div class='symbol-details'>")?;
          writeln!(
            output,
            "        <strong>Location:</strong> <code>{}:{}:{}</code><br>",
            symbol.symbol.file_path.display(),
            symbol.symbol.line,
            symbol.symbol.column
          )?;
          writeln!(output, "        <strong>Reason:</strong> {}", symbol.reason)?;
          writeln!(output, "      </div>")?;
          writeln!(output, "    </div>")?;
        }
      }
    }
  } else {
    writeln!(output, "    <div class='success'>")?;
    writeln!(output, "      <h2>✅ Excellent! No Unused Symbols Found!</h2>")?;
    writeln!(output, "      <p>All symbols are properly utilized in your codebase.</p>")?;
    writeln!(output, "    </div>")?;
  }

  writeln!(output, "  </div>")?;
  writeln!(output, "</body>")?;
  writeln!(output, "</html>")?;

  Ok(output)
}

fn generate_text_report(result: &AnalysisResult, _only_unused: bool) -> Result<String> {
  let mut output = String::new();
  let stats = &result.statistics;

  writeln!(output, "{}", "=".repeat(80).yellow())?;
  writeln!(output, "{}", "CRATE USAGE ANALYSIS REPORT (v2)".cyan().bold())?;
  writeln!(output, "{}", "=".repeat(80).yellow())?;
  writeln!(output)?;

  writeln!(output, "{}", "📊 Summary".green().bold())?;
  writeln!(output, "  Total Crates: {}", result.total_crates.to_string().cyan())?;
  writeln!(output, "  Total Symbols: {}", stats.total_symbols.to_string().cyan())?;
  writeln!(output, "  Used Symbols: {}", stats.used_symbols.to_string().green())?;
  writeln!(output, "  Unused Symbols: {}", stats.unused_symbols.to_string().red())?;

  let usage_percentage = if stats.total_symbols > 0 {
    (stats.used_symbols as f64 / stats.total_symbols as f64) * 100.0
  } else {
    100.0
  };

  let usage_color = if usage_percentage >= 80.0 {
    format!("{:.2}%", usage_percentage).green()
  } else if usage_percentage >= 50.0 {
    format!("{:.2}%", usage_percentage).yellow()
  } else {
    format!("{:.2}%", usage_percentage).red()
  };

  writeln!(output, "  Usage Rate: {}", usage_color)?;
  writeln!(output)?;

  writeln!(output, "{}", "📈 Usage Breakdown".blue().bold())?;
  writeln!(output, "  Internal Use Only: {}", stats.internal_only.to_string().cyan())?;
  writeln!(output, "  External Use Only: {}", stats.external_only.to_string().cyan())?;
  writeln!(output, "  Re-exported: {}", stats.re_exported.to_string().yellow())?;
  writeln!(output, "  Public from Entry: {}", stats.pub_from_entry.to_string().green())?;
  writeln!(output)?;

  if !result.unused_symbols.is_empty() {
    writeln!(output, "{}", "⚠️  Unused Symbols".red().bold())?;
    writeln!(output)?;

    let mut by_crate: rustc_hash::FxHashMap<String, Vec<&UnusedSymbol>> =
      rustc_hash::FxHashMap::default();

    for symbol in &result.unused_symbols {
      by_crate.entry(symbol.crate_name.clone()).or_default().push(symbol);
    }

    let mut crates: Vec<_> = by_crate.keys().cloned().collect();
    crates.sort();

    for crate_name in crates {
      if let Some(symbols) = by_crate.get(&crate_name) {
        writeln!(output, "  📦 {}", crate_name.yellow().bold())?;

        for symbol in symbols {
          let kind_str = format!("{:?}", symbol.symbol.kind);
          writeln!(output, "     • {} {} ", kind_str.dimmed(), symbol.symbol.name.red())?;
          writeln!(
            output,
            "       {}",
            format!(
              "{}:{}:{}",
              symbol.symbol.file_path.display(),
              symbol.symbol.line,
              symbol.symbol.column
            )
            .dimmed()
          )?;
          writeln!(output, "       {}", format!("Reason: {}", symbol.reason).italic().dimmed())?;
        }
        writeln!(output)?;
      }
    }
  } else {
    writeln!(output, "{}", "✅ All symbols are being used!".green().bold())?;
  }

  writeln!(output, "{}", "=".repeat(80).yellow())?;

  Ok(output)
}
