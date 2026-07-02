#!/usr/bin/env node
// Set up the vendored Vite submodule (`packages/test-dev-server/vite`) so the
// browser-platform tests run on Vite's full bundle mode backed by the
// workspace's local rolldown — WITHOUT modifying anything in the Vite repo:
//
//   1. init the submodule (pinned to a vitejs/vite commit),
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

// 0. The local rolldown must exist — the harness (and Vite, after the swap)
// loads it at runtime.
if (!nodeFs.existsSync(nodePath.join(localRolldownDir, 'dist', 'index.mjs'))) {
  console.error(
    '[setup-vite] packages/rolldown/dist is missing — run `just build-rolldown` first.',
  );
  process.exit(1);
}

// 1. Submodule present?
if (!nodeFs.existsSync(nodePath.join(viteDir, 'package.json'))) {
  run('git submodule update --init -- packages/test-dev-server/vite', packageDir);
}

// 2. Install Vite's workspace deps exactly as pinned upstream. (The
// simple-git-hooks postinstall warns inside a submodule — harmless.)
run('pnpm install --frozen-lockfile', viteDir);

// 3. Build the vite package (dist/node + dist/client + types).
run('pnpm --filter vite run build', viteDir);

// 4. Point Vite's runtime `rolldown` resolution at the workspace package.
const linkPath = nodePath.join(viteDir, 'packages', 'vite', 'node_modules', 'rolldown');
const target = nodePath.relative(nodePath.dirname(linkPath), localRolldownDir);
const current = nodeFs.existsSync(linkPath) ? nodeFs.realpathSync(linkPath) : null;
if (current !== nodeFs.realpathSync(localRolldownDir)) {
  nodeFs.rmSync(linkPath, { recursive: true, force: true });
  nodeFs.symlinkSync(target, linkPath);
  console.log(`[setup-vite] linked ${linkPath} -> ${target}`);
} else {
  console.log('[setup-vite] rolldown symlink already points at the workspace package');
}

console.log('[setup-vite] done — vite/packages/vite/dist is ready, submodule left pristine');
