// The Vite config shim: builds the app with the rolldown metrics report on EVERY
// environment (one directory per output dir), sourcemaps on the worker environment, and two
// variants behind environment variables - a worker-only rebuild for the loop and the
// preserveModules attribution build. It wraps the app's own config without editing it.
//
// Every environment must carry the report: the devtools tracer is process-global, and an
// environment built without it in the same process writes a full JSON-lines log under
// node_modules/.rolldown/unknown-session (hundreds of MB on a real app).

import fs from 'node:fs';
import path from 'node:path';

export const SHIM_FILE = 'perf-kit.vite.config.ts';
export const ATTRIB_DIR = 'dist-attrib';

export function viteConfigShim(baseConfigFile) {
  return `// Written by perf-kit (worker-config). Wraps ${baseConfigFile} without editing it.
//   npx vite build --config ${SHIM_FILE}                         full build, report per environment
//   ROLLDOWN_PERF_KIT_WORKER_ONLY=1 npx vite build --config ${SHIM_FILE}  rebuild only the worker environment
//   ROLLDOWN_PERF_KIT_ATTRIB=1 npx vite build --config ${SHIM_FILE}       attribution build into ${ATTRIB_DIR}/ (preserveModules)
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { mergeConfig } from 'vite';
import base from './${baseConfigFile.replace(/\.[cm]?[jt]s$/, '')}';

// A rolldown without the report mode takes \`devtools\` as the plain tracer and writes the
// full event log (hundreds of MB per build) instead of the report. Refuse to build on one.
function assertReportMode() {
  const here = createRequire(import.meta.url);
  let from = here;
  try { from = createRequire(path.dirname(fs.realpathSync(here.resolve('vite/package.json')))); } catch {}
  const pkg = JSON.parse(fs.readFileSync(from.resolve('rolldown/package.json'), 'utf8'));
  const dist = path.join(path.dirname(fs.realpathSync(from.resolve('rolldown/package.json'))), 'dist');
  const stack = [dist];
  while (stack.length) {
    const dir = stack.pop()!;
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const f = path.join(dir, e.name);
      if (e.isDirectory()) stack.push(f);
      else if (/\\.d\\.[cm]?ts$/.test(e.name) && /mode\\?:\\s*['"]full['"]\\s*\\|\\s*['"]report['"]/.test(fs.readFileSync(f, 'utf8'))) return;
    }
  }
  throw new Error(
    \`perf-kit: rolldown \${pkg.version} at \${dist} has no devtools report mode; a build with this config would write the full event log instead of the report. Run \\\`rolldown-install --app <appDir>\\\` first (\\\`--from <built packages/rolldown>\\\` when no checkout sits next to the lab).\`,
  );
}

export default async (env: any) => {
  assertReportMode();
  const resolved = typeof base === 'function' ? await (base as any)(env) : await base;
  const attrib = process.env.ROLLDOWN_PERF_KIT_ATTRIB === '1';
  const workerOnly = attrib || process.env.ROLLDOWN_PERF_KIT_WORKER_ONLY === '1';
  return mergeConfig(resolved, {
    ...(workerOnly
      ? {
          builder: {
            buildApp: async (builder: any) => {
              for (const name of Object.keys(builder.environments)) {
                if (name !== 'client') await builder.build(builder.environments[name]);
              }
            },
          },
        }
      : {}),
    plugins: [
      {
        name: 'perf-kit',
        generateBundle(_options: any, bundle: Record<string, any>) {
          // rolldown's preserveModules output ends every CommonJS file with an eager
          // \`export default require_x();\`. In a require cycle that call reaches a wrapper whose
          // file has not run yet ("require_x is not a function"), and the attribution build
          // throws at import (rolldown#11348). Its importers use the named wrapper, so this build drops the eager
          // line; the cycle then resolves at the require site, as in node. The line is the last
          // one of the file, so the sourcemap of the lines above stays right. The entry keeps
          // its default export: that is the Worker's interface, and the lab imports it.
          if (!attrib) return;
          for (const chunk of Object.values(bundle)) {
            if (chunk.type !== 'chunk' || chunk.isEntry) continue;
            const code = chunk.code.replace(/^export default require_[\\w$]+\\(\\);\\n?/m, '');
            if (code !== chunk.code) chunk.code = code;
          }
        },
        configEnvironment(name: string) {
          const worker = name !== 'client';
          return {
            build: {
              ...(attrib && worker ? { outDir: \`${ATTRIB_DIR}/\${name}\`, minify: false } : {}),
              sourcemap: worker,
              rolldownOptions: {
                ...(attrib && worker
                  ? { output: { preserveModules: true, preserveModulesRoot: '.', entryFileNames: '[name].js', chunkFileNames: '[name].js' } }
                  : {}),
                devtools: { mode: 'report' },
              },
            },
          };
        },
      },
    ],
  });
};
`;
}

/** Write the shim next to the app's vite config. Returns `{ file, baseConfigFile, existed }`. */
export function writeViteConfigShim(appDir, { force = false } = {}) {
  const base = ['vite.config.ts', 'vite.config.mts', 'vite.config.js', 'vite.config.mjs'].find(
    (f) => fs.existsSync(path.join(appDir, f)),
  );
  if (!base) throw new Error(`no vite.config.{ts,mts,js,mjs} in ${appDir}`);
  const file = path.join(appDir, SHIM_FILE);
  const existed = fs.existsSync(file);
  if (!existed || force) fs.writeFileSync(file, viteConfigShim(base));
  return { file, baseConfigFile: base, existed };
}
