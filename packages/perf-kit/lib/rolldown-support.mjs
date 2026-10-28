// The app's rolldown must carry the devtools report mode, or no build writes the report
// and graph / what-if / cut / why / graph-diff have nothing to read. The npm release does
// not carry it yet; this checkout's packages/rolldown does. Two things live here: the probe
// that names the app's rolldown and whether it has the mode, and the install that copies
// the checkout's build over the copy the app's vite resolves, whatever the package manager
// laid out (bun and npm hoist a real directory, pnpm symlinks into its store).

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const LAB_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
// The sibling package is this checkout's packages/rolldown only when the lab runs from the
// repo. An installed copy sits under node_modules, where the sibling is the app's own release.
export const CHECKOUT_ROLLDOWN = /[\\/]node_modules[\\/]/.test(LAB_DIR)
  ? null
  : path.resolve(LAB_DIR, '..', 'rolldown');
// What a built package carries (the npm tarball leaves dist/*.node out; the install keeps it,
// because the loader tries dist/rolldown-binding.<platform>.node before the binding package).
const PACKAGE_PARTS = ['dist', 'bin', 'cli', 'package.json', 'LICENSE'];

const readJson = (file) => JSON.parse(fs.readFileSync(file, 'utf8'));
const REPORT_MODE = /mode\?:\s*['"]full['"]\s*\|\s*['"]report['"]/;

/** True when a package's type declarations offer `mode: 'report'` for devtools. */
export function hasReportMode(pkgDir) {
  const dist = path.join(pkgDir, 'dist');
  if (!fs.existsSync(dist)) return false;
  const stack = [dist];
  while (stack.length) {
    const dir = stack.pop();
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const file = path.join(dir, entry.name);
      if (entry.isDirectory()) stack.push(file);
      else if (/\.d\.[cm]?ts$/.test(entry.name) && REPORT_MODE.test(fs.readFileSync(file, 'utf8')))
        return true;
    }
  }
  return false;
}

/**
 * The rolldown package the app's vite resolves (the app's own when vite is absent):
 * `{ dir, version, viteVersion, reportMode }`; `dir` is null when none resolves.
 */
export function appRolldown(appDir) {
  const fromApp = createRequire(path.join(appDir, 'package.json'));
  let viteDir = null;
  let viteVersion = null;
  try {
    const vitePkg = fromApp.resolve('vite/package.json');
    viteDir = path.dirname(fs.realpathSync(vitePkg));
    viteVersion = readJson(vitePkg).version;
  } catch {
    /* no vite: resolve from the app itself */
  }
  let pkgFile = null;
  for (const from of [viteDir, appDir]) {
    if (!from) continue;
    try {
      pkgFile = createRequire(path.join(from, 'package.json')).resolve('rolldown/package.json');
      break;
    } catch {
      /* try the next base */
    }
  }
  if (!pkgFile) return { dir: null, version: null, viteVersion, reportMode: false };
  const dir = path.dirname(fs.realpathSync(pkgFile));
  return { dir, version: readJson(pkgFile).version, viteVersion, reportMode: hasReportMode(dir) };
}

/** This checkout's packages/rolldown: `{ dir, version, built, binding, reportMode }`, or null when none (or `dir` is null). */
export function checkoutRolldown(dir = CHECKOUT_ROLLDOWN) {
  if (!dir) return null;
  const pkgFile = path.join(dir, 'package.json');
  if (!fs.existsSync(pkgFile)) return null;
  const dist = path.join(dir, 'dist');
  const built = fs.existsSync(path.join(dist, 'index.mjs'));
  const binding = built
    ? (fs.readdirSync(dist).find((f) => /^rolldown-binding\..+\.node$/.test(f)) ?? null)
    : null;
  return {
    dir,
    version: readJson(pkgFile).version,
    built,
    binding,
    reportMode: built && hasReportMode(dir),
  };
}

/**
 * Why no build of this app writes the report, with the fix, or null when its rolldown has
 * the report mode. `cli` is how to spell the harness in the hint.
 */
export function reportSupportNote(appDir, cli, { checkout = CHECKOUT_ROLLDOWN } = {}) {
  const app = appRolldown(appDir);
  if (app.reportMode) return null;
  const rel = path.relative(process.cwd(), appDir);
  const appRel = !rel ? '.' : rel.startsWith('..') ? appDir : rel;
  if (!app.dir) {
    return `no rolldown package resolves from ${appDir} - install the app's dependencies first (vite >= 8 brings rolldown).`;
  }
  const src = checkoutRolldown(checkout);
  const lines = [
    `rolldown ${app.version} at ${app.dir} has no devtools report mode, so no build of this app writes the report`,
    '(the npm release does not carry it yet; graph / what-if / cut / why / graph-diff need it).',
  ];
  if (src?.built && src.binding) {
    lines.push(
      `Install the build next to the lab (rolldown ${src.version}) over it, then rebuild:`,
      `  ${cli} rolldown-install --app ${appRel}`,
      '(`<package manager> install --force` in the app restores the release later.)',
    );
  } else if (src) {
    lines.push(
      `The rolldown package next to the lab, ${src.dir}, is not built: run \`just build-rolldown-release\` there,`,
      `then \`${cli} rolldown-install --app ${appRel}\` copies it over the app's copy.`,
    );
  } else {
    lines.push(
      'No rolldown package sits next to the lab. Build one in a rolldown checkout (`just build-rolldown-release`',
      `writes packages/rolldown), then \`${cli} rolldown-install --app ${appRel} --from <that packages/rolldown>\`.`,
    );
  }
  return lines.join('\n');
}

/**
 * Copy the checkout's built package over the copy the app's vite resolves. Each part is
 * removed before the copy: pnpm hard-links its store, and a write in place would change
 * every project that shares the file. Returns `{ from, dir, before, after }`.
 */
export function installCheckoutRolldown(appDir, { checkout = CHECKOUT_ROLLDOWN } = {}) {
  const app = appRolldown(appDir);
  if (!app.dir)
    throw new Error(
      `no rolldown package resolves from ${appDir} - install the app's dependencies first.`,
    );
  const src = checkoutRolldown(checkout);
  if (!src) {
    throw new Error(
      checkout
        ? `no rolldown package at ${checkout}`
        : 'no rolldown package sits next to the lab: pass --from <a built packages/rolldown>.',
    );
  }
  if (!src.built || !src.binding) {
    throw new Error(
      `${src.dir} is not built (dist/index.mjs and dist/rolldown-binding.*.node): run \`just build-rolldown-release\` in the checkout first.`,
    );
  }
  for (const part of PACKAGE_PARTS) {
    const from = path.join(src.dir, part);
    if (!fs.existsSync(from)) continue;
    const to = path.join(app.dir, part);
    fs.rmSync(to, { recursive: true, force: true });
    fs.cpSync(from, to, { recursive: true, dereference: true });
  }
  return { from: src, dir: app.dir, before: app.version, after: appRolldown(appDir) };
}
