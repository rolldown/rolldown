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
import { mergeConfig } from 'vite';
import base from './${baseConfigFile.replace(/\.[cm]?[jt]s$/, '')}';

export default async (env: any) => {
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
