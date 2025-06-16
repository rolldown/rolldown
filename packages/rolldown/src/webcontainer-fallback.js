import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';

const rolldownPkg = JSON.parse(
  readFileSync(require.resolve('rolldown/package.json'), 'utf-8'),
);
const version = rolldownPkg.version;
const baseDir = `/tmp/rolldown-${version}`;
const bindingEntry =
  `${baseDir}/node_modules/@rolldown/binding-wasm32-wasi/rolldown-binding.wasi.cjs`;

if (!existsSync(bindingEntry)) {
  const bindingPkg = `@rolldown/binding-wasm32-wasi@${version}`;
  rmSync(baseDir, { recursive: true, force: true });
  mkdirSync(baseDir, { recursive: true });
  // eslint-disable-next-line: no-console
  console.log(`[rolldown] Downloading ${bindingPkg} on WebContainer...`);
  execFileSync('pnpm', ['i', bindingPkg], {
    cwd: baseDir,
    stdio: 'inherit',
  });
}

export default require(bindingEntry);
