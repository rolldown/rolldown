import colors from 'ansis';
import { globSync } from 'glob';
import fs from 'node:fs';
import nodePath from 'node:path';
// import pkgJson from './package.json' with { type: 'json' };
import { build, BuildOptions, type Plugin } from './src/index';

const isCI = !!process.env.CI;
const isReleasingCI = !!process.env.RELEASING;

// In `@rolldown/browser`, there will be three builds:
// - CJS and ESM for Node (used in StackBlitz / WebContainers)
// - ESM for bundlers (used in Vite and running in the browser)
// - ESM with inlined dependencies for CDN imports
const isBrowserPkg = !!process.env.BROWSER_PKG;

const pkgRoot = isBrowserPkg
  ? nodePath.resolve(__dirname, '../browser')
  : __dirname;
const outputDir = nodePath.resolve(pkgRoot, 'dist');
const pkgJson = JSON.parse(
  fs.readFileSync(nodePath.resolve(pkgRoot, 'package.json'), 'utf-8'),
);

const bindingFile = nodePath.resolve('src/binding.js');
const bindingFileWasi = nodePath.resolve('src/rolldown-binding.wasi.cjs');
const bindingFileWasiBrowser = nodePath.resolve(
  'src/rolldown-binding.wasi-browser.js',
);

const configs: BuildOptions[] = [
  withShared({
    plugins: [patchBindingJs()],
    output: {
      dir: outputDir,
      format: 'esm',
      entryFileNames: `[name].mjs`,
      chunkFileNames: `shared/[name]-[hash].mjs`,
    },
  }),
  withShared({
    plugins: [shimImportMeta(), patchBindingJs()],
    output: {
      dir: outputDir,
      format: 'cjs',
      entryFileNames: '[name].cjs',
      chunkFileNames: 'shared/[name]-[hash].cjs',
    },
  }),
];

if (isBrowserPkg) {
  configs.push(
    withShared({
      browserBuild: true,
      output: {
        format: 'esm',
        file: nodePath.resolve(outputDir, 'browser-bundler.mjs'),
      },
    }),
    withShared({
      browserBuild: true,
      inlineDependency: true,
      output: {
        format: 'esm',
        file: nodePath.resolve(outputDir, 'browser.js'),
        minify: 'dce-only',
      },
    }),
  );
}

(async () => {
  for (const config of configs) {
    await build(config);
  }
  copy();
})();

function withShared(
  { browserBuild: isBrowserBuild, inlineDependency, ...options }: {
    browserBuild?: boolean;
    inlineDependency?: boolean;
  } & BuildOptions,
): BuildOptions {
  return {
    input: {
      index: './src/index',
      ...!isBrowserBuild
        ? {
          cli: './src/cli/index',
          'parallel-plugin': './src/parallel-plugin',
          'parallel-plugin-worker': './src/parallel-plugin-worker',
          'experimental-index': './src/experimental-index',
          'parse-ast-index': './src/parse-ast-index',
        }
        : {},
    },
    platform: isBrowserBuild ? 'browser' : 'node',
    resolve: {
      extensions: ['.js', '.cjs', '.mjs', '.ts'],
      alias: isBrowserPkg
        ? {
          [bindingFile]: isBrowserBuild
            ? bindingFileWasiBrowser
            : bindingFileWasi,
        }
        : {},
    },
    external: inlineDependency ? undefined : [
      /rolldown-binding\..*\.node/,
      /rolldown-binding\..*\.wasm/,
      /@rolldown\/binding-.*/,
      /\.\/rolldown-binding\.wasi\.cjs/,
      // some dependencies, e.g. zod, cannot be inlined because their types
      // are used in public APIs
      ...Object.keys(pkgJson.dependencies),
      bindingFileWasi,
      bindingFileWasiBrowser,
    ],
    define: {
      'import.meta.browserBuild': String(isBrowserBuild),
    },
    ...options,
    plugins: [
      isBrowserBuild && removeBuiltModules(),
      isBrowserBuild && inlineDependency && rewriteWasmUrl(),
      options.plugins,
    ],
  };
}

function rewriteWasmUrl(): Plugin {
  return {
    name: 'patch-new-url',
    resolveId: {
      filter: { id: /\?url$/ },
      handler(source) {
        return source;
      },
    },
    load: {
      filter: { id: /\?url$/ },
      handler(id) {
        const filename = cleanUrl(id);
        return `export default new URL(${
          JSON.stringify(filename)
        }, import.meta.url).href`;
      },
    },
  };
}

const postfixRE = /[#?].*$/s;
function cleanUrl(url: string) {
  return url.replace(postfixRE, '');
}

function removeBuiltModules(): Plugin {
  return {
    name: 'remove-built-modules',
    resolveId(id) {
      if (id === 'node:path') {
        return this.resolve('pathe');
      }
      if (id === 'node:os' || id === 'node:worker_threads') {
        return { id, external: true, moduleSideEffects: false };
      }
    },
  };
}

function shimImportMeta(): Plugin {
  return {
    name: 'shim-import-meta',
    transform: {
      filter: {
        code: {
          include: ['import.meta.resolve'],
        },
      },
      handler(code, id) {
        if (id.endsWith('.ts') && code.includes('import.meta.resolve')) {
          return code.replace('import.meta.resolve', 'undefined');
        }
      },
    },
  };
}

function patchBindingJs(): Plugin {
  return {
    name: 'patch-binding-js',
    transform: {
      filter: {
        id: 'src/binding.js',
      },
      handler(code) {
        return (
          code
            // strip off unneeded createRequire in cjs, which breaks mjs
            .replace('require = createRequire(__filename)', '')
            // inject binding auto download fallback for webcontainer
            .replace(
              '\nif (!nativeBinding) {',
              (s) =>
                `
if (!nativeBinding && globalThis.process?.versions?.["webcontainer"]) {
  try {
    nativeBinding = require('./webcontainer-fallback.js');
  } catch (err) {
    loadErrors.push(err)
  }
}
` + s,
            )
        );
      },
    },
  };
}

function copy() {
  // wasm build rely on `.node` binaries. But we don't want to copy `.node` files
  // to the dist folder, so we need to distinguish between `.wasm` and `.node` files.
  const wasmFiles = globSync('./src/rolldown-binding.*.wasm', {
    absolute: true,
  });
  const nodeFiles = globSync('./src/rolldown-binding.*.node', {
    absolute: true,
  });

  // Binary build is on the separate step on CI
  if (!isCI) {
    if (isBrowserPkg && !wasmFiles.length) {
      throw new Error('No WASM files found.');
    }
    if (!isBrowserPkg && !nodeFiles.length) {
      throw new Error('No Node files found.');
    }
  }

  const copyTo = nodePath.resolve(outputDir);
  fs.mkdirSync(copyTo, { recursive: true });

  if (!isReleasingCI) {
    // Released `rolldown` package import binary via `@rolldown/binding-<platform>` packages.
    // There's no need to copy binary files to dist folder.

    if (isBrowserPkg) {
      // Move the wasm file to dist
      wasmFiles.forEach((file) => {
        const fileName = nodePath.basename(file);
        if (isBrowserPkg && fileName.includes('debug')) {
          // NAPI-RS now generates a debug wasm binary no matter how and we don't want to ship it to npm.
          console.log(colors.yellow('[build:done]'), 'Skipping', file);
        } else {
          console.log(
            colors.green('[build:done]'),
            'Copying',
            file,
            `to ${copyTo}`,
          );
          fs.cpSync(file, nodePath.join(copyTo, fileName));
        }
      });

      const browserShims = globSync(
        './src/*wasi*js',
        { absolute: true },
      );
      browserShims.forEach((file) => {
        const fileName = nodePath.basename(file);
        console.log(
          colors.green('[build:done]'),
          'Copying',
          file,
          `to ${copyTo}`,
        );
        fs.cpSync(file, nodePath.join(copyTo, fileName));
      });
    } else {
      // Move the binary file to dist
      nodeFiles.forEach((file) => {
        const fileName = nodePath.basename(file);
        console.log(
          colors.green('[build:done]'),
          'Copying',
          file,
          `to ${copyTo}`,
        );
        fs.cpSync(file, nodePath.join(copyTo, fileName));
      });
    }
  }

  // Copy binding types and rollup types to dist
  const distTypesDir = nodePath.resolve(outputDir, 'types');
  fs.mkdirSync(distTypesDir, { recursive: true });
  const types = globSync(['./src/*.d.ts'], {
    absolute: true,
  });
  types.forEach((file) => {
    const fileName = nodePath.basename(file);
    console.log(
      colors.green('[build:done]'),
      'Copying',
      file,
      `to ${distTypesDir}`,
    );
    fs.cpSync(file, nodePath.join(distTypesDir, fileName), {
      recursive: true,
      force: true,
    });
  });
}
