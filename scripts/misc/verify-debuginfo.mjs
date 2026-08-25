// Check that a stripped release binding symbolicates a panic backtrace once its
// published debug info sits next to it, and not before.
// See internal-docs/panic-symbolication/implementation.md
//
// Usage: node scripts/misc/verify-debuginfo.mjs [--debuginfo <archive.tar.zst>]
// Without `--debuginfo`, the single archive under `target/debuginfo/` is used.

import { execFileSync, spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const BINDING_DIR = path.join(REPO_ROOT, 'packages/rolldown/src');
const BINDING_CJS = path.join(BINDING_DIR, 'binding.cjs');

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--debuginfo') args.debuginfo = path.resolve(argv[++i]);
    else throw new Error(`unknown argument: ${argv[i]}`);
  }
  if (!args.debuginfo) {
    const dir = path.join(REPO_ROOT, 'target/debuginfo');
    const archives = fs.readdirSync(dir).filter((f) => f.endsWith('.debuginfo.tar.zst'));
    if (archives.length !== 1)
      throw new Error(`expected one archive in ${dir}, found: ${archives.join(', ') || 'none'}`);
    args.debuginfo = path.join(dir, archives[0]);
  }
  return args;
}

function forcePanic() {
  const result = spawnSync(
    process.execPath,
    ['-e', `require(${JSON.stringify(BINDING_CJS)}).__internalForcePanic()`],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        RUST_BACKTRACE: '1',
        // dbghelp searches the module's own directory first; this is the fallback.
        _NT_SYMBOL_PATH: BINDING_DIR,
      },
    },
  );
  if (result.status === 0) throw new Error('__internalForcePanic() did not fail');
  if (!result.stderr.includes('Rolldown panicked')) {
    throw new Error(`panic hook output missing from stderr:\n${result.stderr}`);
  }
  return result.stderr;
}

// A frame with a source location only appears when debug info was found. The
// panic message itself also names `lib.rs`, so match the indented ` at ` frame
// lines that the default hook prints below `stack backtrace:`.
const SOURCE_FRAME = /^\s+at .*\.rs:\d+/m;
// The name may or may not carry its crate path, depending on the debug format.
const PANIC_FRAME = /^\s+\d+: (?:\S*::)?internal_force_panic\s*$/m;

function main() {
  const { debuginfo } = parseArgs(process.argv.slice(2));

  console.info('1. panic without debug info');
  const before = forcePanic();
  if (SOURCE_FRAME.test(before)) {
    throw new Error(`the binding still carries debug info:\n${before}`);
  }

  console.info(`2. unpack ${path.basename(debuginfo)} next to the binding`);
  const tarball = path.join(
    fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-debuginfo-')),
    'debuginfo.tar',
  );
  execFileSync('zstd', ['-d', '-q', '-f', debuginfo, '-o', tarball], { stdio: 'inherit' });
  const entry = execFileSync('tar', ['-tf', tarball], { encoding: 'utf8' }).split('/')[0].trim();
  execFileSync('tar', ['-xf', tarball, '-C', BINDING_DIR], { stdio: 'inherit' });
  fs.rmSync(path.dirname(tarball), { recursive: true, force: true });

  console.info('3. panic with debug info');
  let after;
  try {
    after = forcePanic();
  } finally {
    fs.rmSync(path.join(BINDING_DIR, entry), { recursive: true, force: true });
  }
  if (!PANIC_FRAME.test(after) || !SOURCE_FRAME.test(after)) {
    throw new Error(`backtrace was not symbolicated:\n${after}`);
  }
  console.info(
    after
      .split('\n')
      .filter((l) => PANIC_FRAME.test(l) || SOURCE_FRAME.test(l))
      .slice(0, 4)
      .join('\n'),
  );
  console.info('ok');
}

main();
