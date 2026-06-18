#!/usr/bin/env node
// Encode a set of source files into a rolldown REPL share URL.
//
// Inverse of decode.mjs. Builds the hash payload as
// base64(zlib.deflate(JSON({v: version, f: { filename: {n, c, e?} }}))).
//
// Usage:
//   node encode.mjs <dir> [--entry <file>]... [--version <v>] [--base <repl-url>]
//                         [--variant <name>] [--no-config]
//
// All non-dotfile files under <dir> are included. Use --entry to mark a file
// as a REPL entry; repeat the flag for multi-entry fixtures (e.g.
// `--entry a.js --entry b.js`).
//
// _config.json handling
// ---------------------
// Rolldown integration-test fixtures carry their build options in a sibling
// `_config.json` (a flattened `BundlerOptions`). The REPL share format has no
// "options" field — instead the REPL stores the bundle config as an ordinary
// `rolldown.config.ts` file in the file map (the one that does
// `input: import.meta.input`). So when a `_config.json` is present this script
// translates its `config` into a generated `rolldown.config.ts`, embeds it in
// the payload, and auto-marks the entries declared in `config.input`. That
// makes the shared link actually reproduce the fixture instead of silently
// dropping its config. Pass --no-config to skip this and emit files only.

import { argv, exit } from 'node:process';
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { Buffer } from 'node:buffer';
import zlib from 'node:zlib';

const SKIP_DIRS = new Set(['dist', 'node_modules', '.git']);
// Fixture harness files that are not REPL source files. _config.json is read
// separately (it drives the generated rolldown.config.ts) but is never emitted
// verbatim into the payload.
const SKIP_FILES = new Set(['_config.json', '_config.ts', '_test.mjs', 'mod.rs']);
const SKIP_EXT = ['.snap'];

// The REPL's config file name (CONFIG_FILES[0] in rolldown/repl bundler.ts).
const CONFIG_FILE = 'rolldown.config.ts';

// --- _config.json BundlerOptions -> JS defineConfig() mapping --------------
//
// _config.json uses the Rust serde (camelCase) names of a *flattened*
// BundlerOptions. The JS API splits these into input options (top level) and
// output options (under `output`), with a few names that differ. The tables
// below capture that mapping. They cover the options fixtures actually use;
// anything unrecognized is left at the top level with a warning so the human
// can verify, and the raw config is always echoed to stderr.

// Output options whose serde name differs from the JS API name.
const OUTPUT_RENAMES = {
  entryFilenames: 'entryFileNames',
  chunkFilenames: 'chunkFileNames',
  assetFilenames: 'assetFileNames',
  sanitizeFilename: 'sanitizeFileName',
};

// Fields that belong under `output` in the JS API (keyed by _config.json name).
const OUTPUT_FIELDS = new Set([
  'dir',
  'file',
  'exports',
  'hashCharacters',
  'format',
  'sourcemap',
  'sourcemapBaseUrl',
  'sourcemapDebugIds',
  'sourcemapIgnoreList',
  'sourcemapPathTransform',
  'sourcemapExcludeSources',
  'banner',
  'footer',
  'postBanner',
  'postFooter',
  'intro',
  'outro',
  'extend',
  'esModule',
  'assetFilenames',
  'entryFilenames',
  'chunkFilenames',
  'sanitizeFilename',
  'minify',
  'name',
  'globals',
  'paths',
  'generatedCode',
  'externalLiveBindings',
  'inlineDynamicImports',
  'dynamicImportInCjs',
  'manualChunks',
  'codeSplitting',
  'advancedChunks',
  'legalComments',
  'comments',
  'polyfillRequire',
  'hoistTransitiveImports',
  'preserveModules',
  'virtualDirname',
  'preserveModulesRoot',
  'topLevelVar',
  'minifyInternalExports',
  'cleanDir',
  'keepNames',
  'strictExecutionOrder',
  'strict',
]);

// Fields whose JS-API home is a deeper nested path (flattened in BundlerOptions).
const NESTED_FIELDS = {
  define: ['transform', 'define'],
  dropLabels: ['transform', 'dropLabels'],
  profilerNames: ['output', 'generatedCode', 'profilerNames'],
};

// Known top-level input options. Used only to decide whether an unrecognized
// field deserves a warning.
const INPUT_FIELDS = new Set([
  'input',
  'plugins',
  'external',
  'resolve',
  'cwd',
  'platform',
  'shimMissingExports',
  'treeshake',
  'logLevel',
  'onLog',
  'onwarn',
  'moduleTypes',
  'experimental',
  'transform',
  'watch',
  'checks',
  'makeAbsoluteExternalsRelative',
  'devtools',
  'preserveEntrySignatures',
  'optimization',
  'context',
  'tsconfig',
  'inject',
]);

// Options that reference the local filesystem and are meaningless in the REPL.
const DROP_FIELDS = new Set(['cwd']);

const IMPORT_META_INPUT = '__ROLLDOWN_REPL_IMPORT_META_INPUT__';

function walk(dir, base = dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name.startsWith('.')) continue;
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) {
      if (SKIP_DIRS.has(name)) continue;
      walk(full, base, out);
    } else {
      if (SKIP_FILES.has(name)) continue;
      if (SKIP_EXT.some((ext) => name.endsWith(ext))) continue;
      out.push(relative(base, full).split(sep).join('/'));
    }
  }
  return out;
}

function setPath(obj, path, val) {
  let o = obj;
  for (let i = 0; i < path.length - 1; i++) {
    if (typeof o[path[i]] !== 'object' || o[path[i]] === null) o[path[i]] = {};
    o = o[path[i]];
  }
  o[path[path.length - 1]] = val;
}

// Strip a leading ./ (or .\) so paths line up with REPL file-map keys.
function stripDot(p) {
  return String(p).replace(/^\.\//, '').replace(/^\.\\/, '');
}

// Normalize the many shapes of `config.input` into { entries, jsInput }.
// entries: file names (no leading ./) to mark as REPL entries.
// jsInput: value to place at config.input — either the IMPORT_META_INPUT
//          sentinel (for unnamed inputs) or an explicit { name: file } map
//          (to preserve entry names).
function normalizeInput(input) {
  if (input == null) return { entries: [], jsInput: IMPORT_META_INPUT };
  if (typeof input === 'string') {
    return { entries: [stripDot(input)], jsInput: IMPORT_META_INPUT };
  }
  if (Array.isArray(input)) {
    if (input.every((x) => typeof x === 'string')) {
      return { entries: input.map(stripDot), jsInput: IMPORT_META_INPUT };
    }
    // Array of { name, import } objects.
    const named = {};
    const entries = [];
    for (const item of input) {
      const file = stripDot(item.import ?? item.file ?? '');
      entries.push(file);
      named[item.name ?? file] = file;
    }
    return { entries, jsInput: named };
  }
  if (typeof input === 'object') {
    // Object map { name: path }.
    const named = {};
    const entries = [];
    for (const [name, path] of Object.entries(input)) {
      const file = stripDot(path);
      entries.push(file);
      named[name] = file;
    }
    return { entries, jsInput: named };
  }
  return { entries: [], jsInput: IMPORT_META_INPUT };
}

// Translate a flattened BundlerOptions into a JS defineConfig() object.
// Returns { configObj, entries, warnings }.
function buildConfig(rawConfig) {
  const warnings = [];
  const cfg = { ...rawConfig };

  const { entries, jsInput } = normalizeInput(cfg.input);
  delete cfg.input;

  const result = {};
  // input first so it reads at the top of the generated object.
  result.input = jsInput;

  // Nested fields (define/dropLabels -> transform, profilerNames ->
  // output.generatedCode) are applied in a second pass so they merge onto a
  // whole `transform` / `generatedCode` object rather than being clobbered by
  // it — the BundlerOptions key order would otherwise decide who wins.
  const deferredNested = [];
  for (const [key, val] of Object.entries(cfg)) {
    if (DROP_FIELDS.has(key)) {
      warnings.push(`dropped \`${key}\` (local path — irrelevant in the REPL)`);
      continue;
    }
    if (key === 'plugins') {
      warnings.push('`plugins` cannot be serialized to JSON — port any plugins manually');
      continue;
    }
    if (NESTED_FIELDS[key]) {
      deferredNested.push([key, val]);
      continue;
    }
    if (OUTPUT_FIELDS.has(key)) {
      setPath(result, ['output', OUTPUT_RENAMES[key] || key], val);
      continue;
    }
    if (INPUT_FIELDS.has(key)) {
      result[key] = val;
      continue;
    }
    // Unrecognized — keep at top level but flag it.
    result[key] = val;
    warnings.push(`unknown option \`${key}\` left at top level — verify its placement`);
  }
  for (const [key, val] of deferredNested) {
    setPath(result, NESTED_FIELDS[key], val);
  }

  return { configObj: result, entries, warnings };
}

function genConfigFile(configObj) {
  let body = JSON.stringify(configObj, null, 2);
  // Swap the sentinel string for the real import.meta.input expression.
  body = body.replaceAll(`"${IMPORT_META_INPUT}"`, 'import.meta.input');
  return `import { defineConfig } from 'rolldown'\n\nexport default defineConfig(${body})\n`;
}

function main() {
  const args = argv.slice(2);
  let dir = null;
  const entries = [];
  let version = 'latest';
  let base = 'https://repl.rolldown.rs/';
  let variant = null;
  let useConfig = true;
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === '--entry') entries.push(args[++i]);
    else if (a === '--version') version = args[++i];
    else if (a === '--base') base = args[++i];
    else if (a === '--variant') variant = args[++i];
    else if (a === '--no-config') useConfig = false;
    else if (!dir) dir = a;
  }
  if (!dir) {
    console.error(
      'usage: encode.mjs <dir> [--entry <file>]... [--version <v>] [--base <url>] [--variant <name>] [--no-config]',
    );
    exit(2);
  }

  const files = walk(dir);
  const fileSet = new Set(files);

  // Auto-derive entries + a rolldown.config.ts from _config.json, unless the
  // user opted out or supplied their own config file.
  let generatedConfig = null;
  const configEntries = [];
  const configPath = join(dir, '_config.json');
  const hasUserConfig = [
    'rolldown.config.ts',
    'rolldown.config.js',
    'rolldown.config.mjs',
    'rolldown.config.cjs',
  ].some((c) => files.includes(c));

  if (useConfig && existsSync(configPath)) {
    if (hasUserConfig) {
      console.error(
        'note: rolldown.config.* already present in <dir> — not overwriting it from _config.json',
      );
    } else {
      let parsed;
      try {
        parsed = JSON.parse(readFileSync(configPath, 'utf-8'));
      } catch (e) {
        console.error(`warning: could not parse _config.json (${e.message}) — emitting files only`);
      }
      if (parsed) {
        let rawConfig = { ...(parsed.config || {}) };

        const variants = parsed.configVariants || [];
        if (variant) {
          const v = variants.find((x) => x._configName === variant);
          if (v) {
            const merged = { ...rawConfig, ...v };
            delete merged._configName;
            rawConfig = merged;
            console.error(`note: merged config variant "${variant}"`);
          } else {
            const names = variants.map((x) => x._configName).join(', ') || '(none)';
            console.error(`warning: variant "${variant}" not found; available: ${names}`);
          }
        } else if (variants.length) {
          const names = variants.map((x) => x._configName).join(', ');
          console.error(
            `note: ${variants.length} config variant(s) not encoded (base config used): ${names}`,
          );
          console.error('      re-run with --variant <name> to encode a specific variant.');
        }

        const { configObj, entries: detected, warnings } = buildConfig(rawConfig);
        if (entries.length > 0 && configObj.input && typeof configObj.input === 'object') {
          console.error(
            'warning: --entry is ignored — _config.json declares named inputs, so the generated rolldown.config.ts hardcodes `input` and the REPL bundles from it, not from entry markers.',
          );
        }
        generatedConfig = genConfigFile(configObj);
        for (const e of detected) {
          if (fileSet.has(e)) configEntries.push(e);
          else console.error(`warning: _config.json entry "${e}" has no matching source file`);
        }
        for (const w of warnings) console.error(`warning: ${w}`);
        console.error(`note: generated ${CONFIG_FILE} from _config.json`);
      }
    }
  } else if (useConfig && existsSync(join(dir, '_config.ts'))) {
    console.error(
      'note: found _config.ts (not JSON) — its options cannot be auto-translated; port them into a rolldown.config.ts in the REPL.',
    );
  }

  // Entry resolution: explicit --entry wins, else _config.json entries, else
  // fall back to the conventional default file names.
  let entryList = entries;
  if (entryList.length === 0) entryList = configEntries;
  if (entryList.length === 0) {
    for (const cand of ['entry.js', 'src/main.js', 'index.js', 'main.js', 'index.ts']) {
      if (fileSet.has(cand)) {
        entryList = [cand];
        break;
      }
    }
  }
  const entrySet = new Set(entryList);

  const f = {};
  for (const name of files) {
    const c = readFileSync(join(dir, name), 'utf-8');
    f[name] = entrySet.has(name) ? { n: name, c, e: true } : { n: name, c };
  }
  if (generatedConfig) {
    f[CONFIG_FILE] = { n: CONFIG_FILE, c: generatedConfig };
  }

  const json = JSON.stringify({ v: version, f });
  const compressed = zlib.deflateSync(Buffer.from(json, 'utf-8'), { level: 9 });
  const b64 = compressed.toString('base64');
  const url = base.replace(/#.*$/, '').replace(/\/?$/, '/') + '#' + b64;
  console.log(url);
}

main();
