import { copyFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const DIST_DIR = path.join(REPO_ROOT, 'packages/rolldown/dist');
const SOURCE_LOADER = path.join(REPO_ROOT, 'packages/rolldown/src/binding.cjs');

const artifacts = readdirSync(DIST_DIR, { withFileTypes: true })
  .filter((entry) => entry.isFile() && entry.name.endsWith('.node'))
  .map((entry) => path.join(DIST_DIR, entry.name));

if (artifacts.length !== 1) {
  throw new Error(
    `Expected exactly one top-level native binding in ${path.relative(REPO_ROOT, DIST_DIR)}, found ${artifacts.length}`,
  );
}

const artifact = artifacts[0];
const stagedArtifact = path.join(path.dirname(SOURCE_LOADER), path.basename(artifact));
copyFileSync(artifact, stagedArtifact);

// docs/guide/getting-started.md tells contributors to export this; it would make
// the smoke-load below verify a different binary than the one just staged.
delete process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
const require = createRequire(import.meta.url);
require(SOURCE_LOADER);

console.log(
  `Staged and verified ${path.relative(REPO_ROOT, artifact)} as ${path.relative(REPO_ROOT, stagedArtifact)}`,
);
