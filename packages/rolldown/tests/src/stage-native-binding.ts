import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, readdirSync } from 'node:fs';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../../../', import.meta.url));
const SRC_DIR = fileURLToPath(new URL('../../src/', import.meta.url));
const DIST_DIR = fileURLToPath(new URL('../../dist/', import.meta.url));

// Tests that import `../src/` modules load `src/binding.cjs`, which needs the
// `.node` beside it. CI downloads the artifact into `dist` only.
export function setup(): void {
  // The WASI lanes share this config and have no `.node` to stage.
  if (process.env.ROLLDOWN_WASI_TEST) return;
  if (readdirSync(SRC_DIR).some((file) => file.endsWith('.node'))) return;
  // No build yet: the loader's own error says to run `just build-rolldown`.
  if (!existsSync(DIST_DIR)) return;

  const artifacts = readdirSync(DIST_DIR, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.node'))
    .map((entry) => nodePath.join(DIST_DIR, entry.name));
  if (artifacts.length === 0) return;
  if (artifacts.length > 1) {
    throw new Error(
      `Expected exactly one top-level native binding in ${nodePath.relative(REPO_ROOT, DIST_DIR)}, found ${artifacts.length}`,
    );
  }

  const artifact = artifacts[0];
  const staged = nodePath.join(SRC_DIR, nodePath.basename(artifact));
  copyFileSync(artifact, staged);

  // docs/guide/getting-started.md tells contributors to export this; it would
  // make the check load a different binary than the one just staged. Unset it
  // for the child only so the tests still see the contributor's env.
  execFileSync(
    process.execPath,
    ['-e', 'require(process.argv[1])', nodePath.join(SRC_DIR, 'binding.cjs')],
    { stdio: 'inherit', env: { ...process.env, NAPI_RS_NATIVE_LIBRARY_PATH: undefined } },
  );

  console.log(
    `Staged and verified ${nodePath.relative(REPO_ROOT, artifact)} as ${nodePath.relative(REPO_ROOT, staged)}`,
  );
}
