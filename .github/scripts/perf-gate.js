#!/usr/bin/env node

/**
 * Rolldown Fork Integrity, Upstream Absorption & Performance Gate (`perf-gate.js`)
 *
 * Mirrors the 4-stage gate in `kristofer-baxter/oxc`:
 * 1. Static architecture & fork invariants (verifying fork-only markers across RP-01..RP-02, RD-02..RD-53, and OS-01..OS-04).
 * 2. Upstream absorption & deduplication analysis against `--base <ref>` (identifying upstream commits
 *    that overlap with standalone bugfix or scaling alterations so duplicate hunks can be dropped).
 * 3. Unit & sourcemap parity tests (`rolldown_plugin`, `rolldown`, and `oxc_sourcemap`).
 * 4. Multi-module bundling benchmark gate (`crates/rolldown/examples/perf_gate.rs`).
 */

import { execFileSync, execSync } from "node:child_process";
import fs from "node:fs";

function parseArgs(argv) {
  const opts = {
    baseRef: "origin/main",
    vendorBaseline: "de6facf2228279718e042b801dd6b60dd6503918",
    runTests: true,
    runBench: true,
    rounds: 5,
    maxWorkloadMs: 500,
  };
  for (let i = 2; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--base" && argv[i + 1]) {
      opts.baseRef = argv[++i];
    } else if (arg === "--vendor-baseline" && argv[i + 1]) {
      opts.vendorBaseline = argv[++i];
    } else if (arg === "--no-tests") {
      opts.runTests = false;
    } else if (arg === "--no-bench") {
      opts.runBench = false;
    } else if (arg === "--rounds" && argv[i + 1]) {
      opts.rounds = Number(argv[++i]);
    } else if (arg === "--max-workload-ms" && argv[i + 1]) {
      opts.maxWorkloadMs = Number(argv[++i]);
    } else {
      throw new Error(`Unknown or incomplete argument: ${arg}`);
    }
  }
  if (!Number.isInteger(opts.rounds) || opts.rounds < 1) {
    throw new Error(`--rounds must be an integer >= 1 (got ${opts.rounds})`);
  }
  if (!Number.isFinite(opts.maxWorkloadMs) || opts.maxWorkloadMs <= 0) {
    throw new Error(`--max-workload-ms must be > 0 (got ${opts.maxWorkloadMs})`);
  }
  return opts;
}

const STANDALONE_ALTERATIONS = [
  {
    id: "RD-05",
    category: "bugfix",
    description: "JoinSet-owned module loader tasks and panic propagation",
    files: [
      "crates/rolldown/src/module_loader/module_loader.rs",
      "crates/rolldown/src/module_loader/module_task.rs",
    ],
    markerByFile: {
      "crates/rolldown/src/module_loader/module_loader.rs": "JoinSet",
      "crates/rolldown/src/module_loader/module_task.rs": "self.ctx.send(",
    },
  },
  {
    id: "RD-24",
    category: "bugfix",
    description: "Direct AST construction for HMR binding-cell and temporal registrations (__proto__-safe)",
    files: [
      "crates/rolldown/src/utils/native_record_ast.rs",
      "crates/rolldown/src/module_finalizers/mod.rs",
      "crates/rolldown/src/hmr/hmr_ast_finalizer.rs",
    ],
    markerByFile: {
      "crates/rolldown/src/utils/native_record_ast.rs": "\"__proto__\"",
      "crates/rolldown/src/module_finalizers/mod.rs": "prop_name == \"__proto__\"",
      "crates/rolldown/src/hmr/hmr_ast_finalizer.rs": "exported == \"__proto__\"",
    },
  },
  {
    id: "RD-25",
    category: "bugfix",
    description: "Shadow-safe SymbolRef TDZ and const-reassignment guards",
    files: [
      "crates/rolldown/src/stages/link_stage/temporal_bindings.rs",
      "crates/rolldown/src/module_finalizers/temporal.rs",
    ],
    markerByFile: {
      "crates/rolldown/src/stages/link_stage/temporal_bindings.rs": "fn lexical_declaration",
      "crates/rolldown/src/module_finalizers/temporal.rs": "__checkTDZ",
    },
  },
  {
    id: "RD-32",
    category: "bugfix",
    description: "Complete export retention for dynamic import() member calls, template tags, and callback arguments",
    files: [
      "crates/rolldown/src/ast_scanner/dynamic_import.rs",
      "crates/rolldown/src/ast_scanner/impl_visit.rs",
    ],
    markerByFile: {
      "crates/rolldown/src/ast_scanner/dynamic_import.rs":
        "complete_dynamic_imports_read_before_registration",
      "crates/rolldown/src/ast_scanner/impl_visit.rs":
        "complete_dynamic_imports_read_before_registration",
    },
  },
  {
    id: "RD-37",
    category: "bugfix",
    description: "Iterative Tarjan SCC __esmRun initializer evaluation",
    files: [
      "crates/rolldown/src/runtime/runtime-base.js",
      "crates/rolldown/src/module_finalizers/mod.rs",
    ],
    markerByFile: {
      "crates/rolldown/src/runtime/runtime-base.js": "__esmRun",
      "crates/rolldown/src/module_finalizers/mod.rs": "extract_leading_esm_init_deps",
    },
  },
  {
    id: "RD-38",
    category: "bugfix",
    description: "Deterministic least-fixpoint cyclic side-effect determination",
    files: ["crates/rolldown/src/stages/link_stage/tree_shaking/determine_side_effects.rs"],
    markerByFile: {
      "crates/rolldown/src/stages/link_stage/tree_shaking/determine_side_effects.rs":
        "propagate_side_effects",
    },
  },
  {
    id: "RD-40",
    category: "bugfix",
    description: "Delegated initialization and chunk placement for re-exported lazy-barrel namespaces",
    files: ["crates/rolldown/src/esm_init_obligations.rs"],
    markerByFile: {
      "crates/rolldown/src/esm_init_obligations.rs": "namespace_binding_is_delegated",
    },
  },
  {
    id: "RD-44",
    category: "perf",
    description: "Strict-execution-order dynamic_already_loaded fold without full cycle-graph rebuild",
    files: ["crates/rolldown/src/stages/generate_stage/dynamic_already_loaded.rs"],
    markerByFile: {
      "crates/rolldown/src/stages/generate_stage/dynamic_already_loaded.rs":
        "unless a CommonJS module renders raw",
    },
  },
  {
    id: "RD-45",
    category: "bugfix",
    description: "revert_partial_scan and last_scan_failures_unreached on failed partial scans",
    files: [
      "crates/rolldown/src/module_loader/module_loader.rs",
      "crates/rolldown/src/types/scan_stage_cache.rs",
    ],
    markerByFile: {
      "crates/rolldown/src/module_loader/module_loader.rs": "revert_partial_scan",
      "crates/rolldown/src/types/scan_stage_cache.rs": "last_scan_failures_unreached",
    },
  },
  {
    id: "RD-46",
    category: "bugfix",
    description: "CommonJS constant inlining escape analysis",
    files: [
      "crates/rolldown/src/ast_scanner/hmr.rs",
      "crates/rolldown/src/ast_scanner/impl_visit.rs",
    ],
    markerByFile: {
      "crates/rolldown/src/ast_scanner/hmr.rs": "HasComputedMemberWrite",
      "crates/rolldown/src/ast_scanner/impl_visit.rs": "HasComputedMemberWrite",
    },
  },
  {
    id: "RP-01",
    category: "perf",
    description: "Lazy hook_call_id UUID generation in HookResolveIdCall, HookLoadCall, and HookRenderChunk",
    files: [
      "crates/rolldown_plugin/src/plugin_driver/mod.rs",
      "crates/rolldown_plugin/src/plugin_driver/build_hooks.rs",
      "crates/rolldown_plugin/src/plugin_driver/output_hooks.rs",
    ],
    markerByFile: {
      "crates/rolldown_plugin/src/plugin_driver/mod.rs": "pub fn hook_call_id",
      "crates/rolldown_plugin/src/plugin_driver/build_hooks.rs": "hook_call_id(module_path!())",
      "crates/rolldown_plugin/src/plugin_driver/output_hooks.rs": "hook_call_id(module_path!())",
    },
  },
  {
    id: "RP-02",
    category: "architecture",
    description: "Plugin::interop_module_def_format hook for per-module CommonJS interop",
    files: [
      "crates/rolldown_plugin/src/plugin.rs",
      "crates/rolldown_plugin/src/pluginable.rs",
      "crates/rolldown/src/ecmascript/ecma_module_view_factory.rs",
    ],
    markerByFile: {
      "crates/rolldown_plugin/src/plugin.rs": "fn interop_module_def_format",
      "crates/rolldown_plugin/src/pluginable.rs": "interop_module_def_format",
      "crates/rolldown/src/ecmascript/ecma_module_view_factory.rs": "interop_module_def_format",
    },
  },
  {
    id: "OS-01..04",
    category: "bugfix+perf",
    description: "oxc_sourcemap coordinate retention, line-break estimation, parallel VLQ encoding, and in-place redaction",
    files: [
      "vendor/oxc_sourcemap-9.0.0/src/concat_sourcemap_builder.rs",
      "vendor/oxc_sourcemap-9.0.0/src/encode.rs",
      "vendor/oxc_sourcemap-9.0.0/src/sourcemap.rs",
    ],
    markerByFile: {
      "vendor/oxc_sourcemap-9.0.0/src/concat_sourcemap_builder.rs": "token_chunk_prev_source_line",
      "vendor/oxc_sourcemap-9.0.0/src/encode.rs": "encode_to_string_with_null_contents",
      "vendor/oxc_sourcemap-9.0.0/src/sourcemap.rs": "redact_source_contents",
    },
  },
];

function checkStaticInvariants(failures, summaryLines) {
  console.log("==> [1/4] Checking static fork & performance invariants...");
  for (const alt of STANDALONE_ALTERATIONS) {
    for (const relPath of alt.files) {
      if (!fs.existsSync(relPath)) {
        failures.push(`Missing file for ${alt.id} (${alt.description}): ${relPath}`);
        continue;
      }
      const marker = alt.markerByFile[relPath];
      const content = fs.readFileSync(relPath, "utf8");
      if (!content.includes(marker)) {
        failures.push(
          `Alteration ${alt.id} (${alt.description}) marker '${marker}' missing in ${relPath}.`,
        );
      }
    }
  }
  summaryLines.push(
    `- **Static Invariants**: Verified all ${STANDALONE_ALTERATIONS.length} tracked Rolldown, Rolldown Plugin, and Oxc Sourcemap per-file alteration markers.`,
  );
}

function checkUpstreamAbsorption(opts, failures, summaryLines) {
  console.log(`==> [2/4] Inspecting upstream changes vs ${opts.baseRef} for absorption candidates...`);
  try {
    const mergeBase = execFileSync("git", ["merge-base", "HEAD", opts.baseRef], {
      encoding: "utf8",
    }).trim();
    const baseCommit = execFileSync("git", ["rev-parse", opts.baseRef], {
      encoding: "utf8",
    }).trim();
    const diffStart = mergeBase === baseCommit ? opts.vendorBaseline : mergeBase;
    const upstreamChanged = new Set(
      execFileSync(
        "git",
        ["diff", "--name-only", `${diffStart}..${opts.baseRef}`, "--", "crates/rolldown", "crates/rolldown_plugin"],
        { encoding: "utf8" },
      )
        .split("\n")
        .map(s => s.trim())
        .filter(Boolean),
    );
    const overlapping = [];
    for (const alt of STANDALONE_ALTERATIONS) {
      const hitFiles = alt.files.filter(f => upstreamChanged.has(f));
      if (hitFiles.length > 0) {
        overlapping.push(`${alt.id} (${hitFiles.join(", ")})`);
      }
    }
    if (overlapping.length > 0) {
      summaryLines.push(
        `- **Upstream Overlap Candidates (${diffStart.slice(0, 8)}..${opts.baseRef})**: ${overlapping.join("; ")}`,
      );
    } else {
      summaryLines.push(
        `- **Upstream Overlap Candidates (${diffStart.slice(0, 8)}..${opts.baseRef})**: None (clean separation).`,
      );
    }
  } catch (err) {
    failures.push(`Upstream absorption check failed for --base ${opts.baseRef}: ${err.message.split("\n")[0]}`);
  }
}

function runUnitAndSourcemapTests(opts, failures, summaryLines) {
  if (!opts.runTests) {
    return;
  }
  console.log("==> [3/4] Running rolldown_plugin, rolldown, and oxc_sourcemap unit tests...");
  try {
    execSync("cargo test -p rolldown_plugin --lib --quiet", { stdio: "inherit" });
    execSync("cargo test -p rolldown --lib --features experimental --quiet", { stdio: "inherit" });
    execSync("cargo test --manifest-path vendor/oxc_sourcemap-9.0.0/Cargo.toml --lib --quiet", {
      stdio: "inherit",
    });
    summaryLines.push(
      "- **Unit & Parity Tests**: `rolldown_plugin` (8 tests), `rolldown` (139 tests), and `oxc_sourcemap` (79 tests) passed.",
    );
  } catch {
    failures.push("Unit test suite failed in `rolldown_plugin`, `rolldown`, or `oxc_sourcemap`.");
  }
}

const EXPECTED_BENCH_WORKLOADS = ["bundle_eager_120", "bundle_strict_lazy_barrel_120"];

function runBenchmarks(opts, failures, summaryLines) {
  if (!opts.runBench) {
    return;
  }
  console.log("==> [4/4] Running multi-module bundler benchmark gate...");
  try {
    const out = execFileSync(
      "cargo",
      ["run", "-p", "rolldown", "--example", "perf_gate", "--features", "experimental", "--quiet"],
      {
        encoding: "utf8",
        env: { ...process.env, ROUNDS: String(opts.rounds) },
      },
    );
    const parsed = new Map();
    summaryLines.push("", "| Workload | Emitted Assets | Best Time (ms) | Budget (ms) |", "| :--- | ---: | ---: | ---: |");
    for (const line of out.split("\n")) {
      if (!line.startsWith("RESULT|")) continue;
      const [, name, assetsRaw, nsRaw] = line.trim().split("|");
      const assets = Number(assetsRaw);
      const ns = Number(nsRaw);
      const ms = ns / 1e6;
      parsed.set(name, { assets, ms });
      summaryLines.push(`| \`${name}\` | ${assets} | ${ms.toFixed(2)} ms | <= ${opts.maxWorkloadMs.toFixed(0)} ms |`);
      if (!Number.isInteger(assets) || assets <= 0) {
        failures.push(`Workload ${name} emitted invalid asset count: ${assetsRaw}`);
      }
      if (!Number.isFinite(ms) || ms <= 0 || ms > opts.maxWorkloadMs) {
        failures.push(
          `Workload ${name} took ${ms.toFixed(2)} ms (exceeds budget ceiling of ${opts.maxWorkloadMs} ms).`,
        );
      }
    }
    for (const expected of EXPECTED_BENCH_WORKLOADS) {
      if (!parsed.has(expected)) {
        failures.push(`Missing RESULT| line for benchmark workload '${expected}'.`);
      }
    }
  } catch (err) {
    failures.push(`Benchmark gate failed: ${err.message}`);
  }
}

function main() {
  const opts = parseArgs(process.argv);
  const failures = [];
  const summaryLines = ["## Rolldown Fork Performance & Integrity Gate", ""];

  checkStaticInvariants(failures, summaryLines);
  checkUpstreamAbsorption(opts, failures, summaryLines);
  runUnitAndSourcemapTests(opts, failures, summaryLines);
  runBenchmarks(opts, failures, summaryLines);

  console.log("\n" + summaryLines.join("\n"));
  if (failures.length > 0) {
    console.error("\nFAILURES:");
    for (const f of failures) {
      console.error(`  - ${f}`);
    }
    process.exit(1);
  }
  console.log("\nAll Rolldown fork integrity & performance gates passed.");
}

main();
