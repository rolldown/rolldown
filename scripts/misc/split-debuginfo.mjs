// Split the debug info out of a freshly built release binding.
//
// The release workflow builds the binding with debug info enabled. This script
// moves that debug info into a side file, strips the `.node` back down to what
// npm ships today, and packs the side file for upload to the GitHub Release.
// See internal-docs/panic-symbolication/implementation.md
//
// Usage: node scripts/misc/split-debuginfo.mjs [--target <triple>] [--out-dir <dir>]

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const BINDING_DIR = path.join(REPO_ROOT, 'packages/rolldown/src');

function parseArgs(argv) {
  const args = { outDir: path.join(REPO_ROOT, 'target/debuginfo') };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--target') args.target = argv[++i];
    else if (argv[i] === '--out-dir') args.outDir = path.resolve(argv[++i]);
    else throw new Error(`unknown argument: ${argv[i]}`);
  }
  return args;
}

function run(cmd, cmdArgs, opts = {}) {
  console.info(`$ ${cmd} ${cmdArgs.join(' ')}`);
  return execFileSync(cmd, cmdArgs, {
    stdio: ['ignore', 'pipe', 'inherit'],
    encoding: 'utf8',
    ...opts,
  });
}

function hostTriple() {
  const line = run('rustc', ['-vV'])
    .split('\n')
    .find((l) => l.startsWith('host: '));
  return line.slice('host: '.length).trim();
}

// `rust-objcopy` (llvm-objcopy) ships with every rustc toolchain, so cross-built
// ELF bindings need no extra tool. Fall back to whatever is on PATH.
function findObjcopy() {
  const sysroot = run('rustc', ['--print', 'sysroot']).trim();
  const candidate = path.join(sysroot, 'lib/rustlib', hostTriple(), 'bin/rust-objcopy');
  if (fs.existsSync(candidate)) return candidate;
  for (const name of ['llvm-objcopy', 'objcopy']) {
    try {
      run(name, ['--version']);
      return name;
    } catch {}
  }
  throw new Error('no objcopy found: install the `llvm-tools` rustup component');
}

function findBinding() {
  const nodes = fs.readdirSync(BINDING_DIR).filter((f) => /^rolldown-binding\..+\.node$/.test(f));
  if (nodes.length !== 1) {
    throw new Error(
      `expected exactly one .node in ${BINDING_DIR}, found: ${nodes.join(', ') || 'none'}`,
    );
  }
  return path.join(BINDING_DIR, nodes[0]);
}

// `cargo build --target <triple>` writes to `target/<triple>/release`; a plain
// `cargo build --release` writes to `target/release`.
function findProfileDir(triple) {
  for (const dir of [
    path.join(REPO_ROOT, 'target', triple, 'release'),
    path.join(REPO_ROOT, 'target/release'),
  ]) {
    if (fs.existsSync(dir)) return dir;
  }
  throw new Error(`no release build found for ${triple}`);
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const triple = args.target ?? hostTriple();

  if (triple.startsWith('wasm32-')) {
    console.info(`${triple}: napi already emits a separate debug wasm, nothing to split`);
    return;
  }

  const binding = findBinding();
  const bindingName = path.basename(binding);
  const stage = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-debuginfo-'));
  // The name inside the archive must match what the debugger looks for, so the
  // files are staged under that exact name before packing.
  let staged;

  if (triple.endsWith('-apple-darwin')) {
    // `split-debuginfo = "packed"` makes cargo run dsymutil. backtrace-rs matches
    // any `*.dSYM` in the binding's directory by LC_UUID, so the bundle can carry
    // the binding's name.
    const dsym = path.join(findProfileDir(triple), 'librolldown_binding.dylib.dSYM');
    if (!fs.existsSync(dsym))
      throw new Error(`${dsym} not found; was the build run with split-debuginfo=packed?`);
    staged = `${bindingName}.dSYM`;
    // cargo leaves a symlink to `deps/` here, so dereference it.
    fs.cpSync(dsym, path.join(stage, staged), { recursive: true, dereference: true });
    // Mirrors what rustc does for `strip = "symbols"` on a cdylib (`-x`), plus `-S` for debug entries.
    run('strip', ['-x', '-S', binding]);
  } else if (triple.endsWith('-pc-windows-msvc')) {
    // MSVC already writes the debug info to a separate PDB. The PE keeps a
    // CodeView record naming `rolldown_binding.pdb`, so the name must stay.
    const pdb = path.join(findProfileDir(triple), 'rolldown_binding.pdb');
    if (!fs.existsSync(pdb)) throw new Error(`${pdb} not found`);
    staged = 'rolldown_binding.pdb';
    fs.copyFileSync(pdb, path.join(stage, staged));
  } else {
    const objcopy = findObjcopy();
    staged = `${bindingName}.debug`;
    const debugFile = path.join(stage, staged);
    run(objcopy, ['--only-keep-debug', binding, debugFile]);
    // `--strip-all` is what rustc passes for `strip = "symbols"`.
    run(objcopy, ['--strip-all', binding]);
    // The link records `<bindingName>.debug` plus a CRC; backtrace-rs looks for
    // it next to the binding and under `.debug/`.
    run(objcopy, [`--add-gnu-debuglink=${debugFile}`, binding]);
  }

  fs.mkdirSync(args.outDir, { recursive: true });
  const archive = path.join(args.outDir, `${bindingName}.debuginfo.tar.gz`);
  run('tar', ['-czf', archive, '-C', stage, staged]);
  fs.rmSync(stage, { recursive: true, force: true });

  const mb = (f) => (fs.statSync(f).size / 1024 / 1024).toFixed(1);
  console.info(`binding:   ${binding} (${mb(binding)} MB)`);
  console.info(`debuginfo: ${archive} (${mb(archive)} MB)`);
}

main();
