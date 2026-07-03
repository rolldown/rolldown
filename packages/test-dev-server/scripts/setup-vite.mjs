#!/usr/bin/env node
// Set up the vendored Vite submodule (`packages/test-dev-server/vite`) so the
// browser-platform tests run on Vite's full bundle mode backed by the
// workspace's local rolldown — WITHOUT modifying anything in the Vite repo:
//
//   1. init the submodule — or re-sync it when the checkout is not on the
//      pinned vitejs/vite commit (e.g. after pulling a submodule bump),
//   2. `pnpm install --frozen-lockfile` (no manifest or lockfile writes),
//   3. build the `vite` package with its own pinned dependencies,
//   4. swap the `packages/vite/node_modules/rolldown` symlink to point at the
//      workspace's `packages/rolldown`, so Vite's dist resolves the local
//      rolldown (and its native binding) at runtime.
//
// node_modules is untracked, so the submodule stays pristine in git. The swap
// is undone by any `pnpm install` inside the submodule — re-run this script
// after that (it is idempotent).
//
// Requires `packages/rolldown` to be built first (`just build-rolldown`).
//
// Usage: node scripts/setup-vite.mjs   (from packages/test-dev-server)

import { execSync } from 'node:child_process';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import nodeUrl from 'node:url';

const packageDir = nodePath.dirname(nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url)));
const viteDir = nodePath.join(packageDir, 'vite');
const localRolldownDir = nodePath.join(packageDir, '..', 'rolldown');

const run = (cmd, cwd) => {
  console.log(`[setup-vite] ${cmd}`);
  execSync(cmd, { cwd, stdio: 'inherit' });
};
const capture = (cmd, cwd) => execSync(cmd, { cwd, encoding: 'utf8' }).trim();

// 0. The local rolldown must exist — the harness (and Vite, after the swap)
// loads it at runtime.
if (!nodeFs.existsSync(nodePath.join(localRolldownDir, 'dist', 'index.mjs'))) {
  console.error(
    '[setup-vite] packages/rolldown/dist is missing — run `just build-rolldown` first.',
  );
  process.exit(1);
}

// 1. Init the submodule, or re-sync a checkout that is not on the pinned
// commit — e.g. after pulling a submodule bump, where the working copy stays
// on the old commit until `git submodule update` runs. The pinned SHA comes
// from the superproject index (`git ls-files -s` on the gitlink), which is
// also what `git submodule update` checks out — so a locally staged bump
// syncs to the staged commit. (Pathspec is repo-root-relative, so run git
// from the repo root.)
const repoRoot = nodePath.dirname(nodePath.dirname(packageDir));
const pinnedSha = capture('git ls-files -s -- packages/test-dev-server/vite', repoRoot).split(
  /\s+/,
)[1];
const checkedOutSha = nodeFs.existsSync(nodePath.join(viteDir, '.git'))
  ? capture('git rev-parse HEAD', viteDir)
  : null;
if (checkedOutSha !== pinnedSha || !nodeFs.existsSync(nodePath.join(viteDir, 'package.json'))) {
  if (checkedOutSha !== null && checkedOutSha !== pinnedSha) {
    console.log(
      `[setup-vite] submodule checkout ${checkedOutSha.slice(0, 12)} != pinned ${pinnedSha.slice(0, 12)} — syncing`,
    );
  }
  try {
    // Shallow: only the pinned commit is needed (GitHub allows SHA-addressed
    // shallow fetches, saving the full vite history on CI).
    run('git submodule update --init --depth 1 packages/test-dev-server/vite', repoRoot);
  } catch {
    run('git submodule update --init packages/test-dev-server/vite', repoRoot);
  }
}

// 2. Install Vite's workspace deps exactly as pinned upstream, via vp — it
// delegates to the submodule's pinned pnpm itself, so no pnpm needs to be
// installed separately. (The simple-git-hooks postinstall warns inside a
// submodule — harmless.) This also resets any previous symlink swap from
// step 4, so the build below always uses Vite's own pinned rolldown.
run('vp install --frozen-lockfile', viteDir);

// 3. Build the vite package (dist/node + dist/client), replicating its
// `build-bundle` script (`premove dist && rolldown --config rolldown.config.ts`)
// by invoking the rolldown CLI directly through node:
// - `vp run`/`pnpm run` can't be used here: vp's workspace scan trips over
//   Vite's intentionally-broken playground fixtures (a UTF-8-BOM
//   package.json), and pnpm may not be installed at all.
// - Vite's `build-types` step is skipped: the harness loads vite's dist at
//   runtime and carries its own minimal structural types (see
//   src/vite-server.ts), so building .d.ts would only slow CI down.
const vitePkgDir = nodePath.join(viteDir, 'packages', 'vite');
const pinnedRolldownDir = nodePath.join(vitePkgDir, 'node_modules', 'rolldown');
const rolldownBin = JSON.parse(
  nodeFs.readFileSync(nodePath.join(pinnedRolldownDir, 'package.json'), 'utf8'),
).bin;
const rolldownCli = nodePath.join(
  pinnedRolldownDir,
  typeof rolldownBin === 'string' ? rolldownBin : rolldownBin.rolldown,
);
nodeFs.rmSync(nodePath.join(vitePkgDir, 'dist'), { recursive: true, force: true });
run(`node ${JSON.stringify(rolldownCli)} --config rolldown.config.ts`, vitePkgDir);

// 4. Point Vite's runtime `rolldown` resolution at the workspace package.
const linkPath = nodePath.join(viteDir, 'packages', 'vite', 'node_modules', 'rolldown');
const target = nodePath.relative(nodePath.dirname(linkPath), localRolldownDir);
const current = nodeFs.existsSync(linkPath) ? nodeFs.realpathSync(linkPath) : null;
if (current !== nodeFs.realpathSync(localRolldownDir)) {
  nodeFs.rmSync(linkPath, { recursive: true, force: true });
  // 'junction' sidesteps Windows' symlink privilege requirement (admin /
  // Developer Mode); on other platforms the type is ignored. Node resolves
  // the target to an absolute path itself when creating a junction.
  nodeFs.symlinkSync(target, linkPath, 'junction');
  console.log(`[setup-vite] linked ${linkPath} -> ${target}`);
} else {
  console.log('[setup-vite] rolldown symlink already points at the workspace package');
}

console.log('[setup-vite] done — vite/packages/vite/dist is ready, submodule left pristine');
