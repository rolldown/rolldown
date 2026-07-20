// Builds the demo app with the repo's rolldown (packages/rolldown/dist), devtools
// metrics mode on, so every build also refreshes the build-side metrics report
// (entry bytes, initial-load bytes, delta/baselineDelta) next to the runtime numbers.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const ROLLDOWN_DIST =
  process.env.ROLLDOWN_DIST ?? path.join(REPO_ROOT, 'packages', 'rolldown', 'dist', 'index.mjs');

export async function buildApp({ appDir, metricsDir }) {
  if (!fs.existsSync(ROLLDOWN_DIST)) {
    throw new Error(
      `rolldown dist not found at ${ROLLDOWN_DIST} - run \`just build-rolldown\` first (or set ROLLDOWN_DIST).`,
    );
  }
  const { build } = await import(pathToFileURL(ROLLDOWN_DIST).href);
  const distDir = path.join(appDir, 'dist');
  fs.rmSync(distDir, { recursive: true, force: true });
  const started = Date.now();
  await build({
    cwd: appDir,
    input: { main: './src/main.ts' },
    devtools: { mode: 'metrics', metricsDir },
    output: {
      dir: 'dist',
      format: 'esm',
      sourcemap: true,
      minify: false, // keep offsets readable; relative deltas match minified builds
      entryFileNames: '[name].js',
      chunkFileNames: 'chunks/[name]-[hash].js',
    },
  });
  fs.copyFileSync(path.join(appDir, 'index.html'), path.join(distDir, 'index.html'));

  const entryBytes = fs.statSync(path.join(distDir, 'main.js')).size;
  const chunksDir = path.join(distDir, 'chunks');
  const chunks = fs.existsSync(chunksDir)
    ? fs
        .readdirSync(chunksDir)
        .filter((f) => f.endsWith('.js'))
        .map((f) => ({ file: `chunks/${f}`, bytes: fs.statSync(path.join(chunksDir, f)).size }))
    : [];
  let buildMetrics = null;
  try {
    buildMetrics = JSON.parse(fs.readFileSync(path.join(metricsDir, 'metrics.json'), 'utf8'));
  } catch {
    // metrics report is best-effort; the harness still works without it
  }
  return { distDir, entryBytes, chunks, buildMetrics, wallMs: Date.now() - started };
}
