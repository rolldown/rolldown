// Set up the vendored Vite submodule (`vite/` at the repo root) so the
// browser-platform tests run on Vite's full bundle mode backed by the
// workspace's local rolldown — WITHOUT modifying anything in the Vite repo:
//
//   1. init the submodule if needed and build the checkout exactly as-is
//      (the superproject's pinned commit after a fresh init; updating an
//      existing checkout is always a manual git step — see checkout.ts),
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
// Usage: `just setup-vite` (or `vp run --filter @rolldown-internal/scripts setup-vite`)

import nodeFs from 'node:fs';
import { createRequire } from 'node:module';
import nodePath from 'node:path';
import { ensureViteCheckout, repoRoot, run, viteDir } from './checkout.js';

const localRolldownDir = nodePath.join(repoRoot, 'packages', 'rolldown');

// 0. The local rolldown must exist — the harness (and Vite, after the swap)
// loads it at runtime.
if (!nodeFs.existsSync(nodePath.join(localRolldownDir, 'dist', 'index.mjs'))) {
  console.error(
    '[setup-vite] packages/rolldown/dist is missing — run `just build-rolldown` first.',
  );
  process.exit(1);
}

// 1. Ensure the submodule has a checkout (see checkout.ts), then build
// whatever commit is checked out.
ensureViteCheckout();

// 2. Install Vite's workspace deps exactly as pinned upstream, via vp — it
// delegates to the submodule's pinned pnpm itself, so no pnpm needs to be
// installed separately. (The simple-git-hooks postinstall warns inside a
// submodule — harmless.) This also resets any previous symlink swap from
// step 4, so the build below always uses Vite's own pinned rolldown.
run('vp install --frozen-lockfile', viteDir);

// 3. Build the vite package (dist/node + dist/client, plus its type build)
// via its own `build` script. vp delegates to the submodule's pinned package
// manager, so no pnpm needs to be installed separately.
const vitePkgDir = nodePath.join(viteDir, 'packages', 'vite');
run('vp run build', vitePkgDir);

// 4. Point Vite's runtime `rolldown` resolution at the workspace package.
const linkPath = nodePath.join(vitePkgDir, 'node_modules', 'rolldown');
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

// 5. Verify the override took: resolving `rolldown` from the vite package must
// land inside the workspace copy. Failing loudly here beats silently running
// the tests against the npm-pinned rolldown.
const viteRequire = createRequire(nodePath.join(vitePkgDir, 'package.json'));
const resolvedRolldown = nodeFs.realpathSync(viteRequire.resolve('rolldown'));
if (!resolvedRolldown.startsWith(nodeFs.realpathSync(localRolldownDir) + nodePath.sep)) {
  console.error(
    `[setup-vite] vite resolves rolldown to ${resolvedRolldown}, not the workspace ` +
      'packages/rolldown — the override did not take.',
  );
  process.exit(1);
}

console.log('[setup-vite] done — vite/packages/vite/dist is ready, submodule left pristine');
