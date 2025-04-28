import fs from 'node:fs';
import path from 'node:path';
import { cwd } from 'node:process';
import { pathToFileURL } from 'node:url';
import { loadConfig as loadUnConfig } from 'unconfig';
import { rolldown } from '../api/rolldown';
import type { ConfigExport } from '../types/config-export';
import type { OutputChunk } from '../types/rolldown-output';

async function bundleTsConfig(
  configFile: string,
  isEsm: boolean,
): Promise<string> {
  const dirnameVarName = 'injected_original_dirname';
  const filenameVarName = 'injected_original_filename';
  const importMetaUrlVarName = 'injected_original_import_meta_url';
  const bundle = await rolldown({
    input: configFile,
    platform: 'node',
    resolve: {
      mainFields: ['main'],
    },
    define: {
      __dirname: dirnameVarName,
      __filename: filenameVarName,
      'import.meta.url': importMetaUrlVarName,
      'import.meta.dirname': dirnameVarName,
      'import.meta.filename': filenameVarName,
    },
    treeshake: false,
    external: [/^[\w@][^:]/], // external bare imports
    plugins: [
      {
        name: 'inject-file-scope-variables',
        transform: {
          filter: { id: /\.[cm]?[jt]s$/ },
          async handler(code, id) {
            const injectValues =
              `const ${dirnameVarName} = ${JSON.stringify(path.dirname(id))};` +
              `const ${filenameVarName} = ${JSON.stringify(id)};` +
              `const ${importMetaUrlVarName} = ${
                JSON.stringify(
                  pathToFileURL(id).href,
                )
              };`;
            return { code: injectValues + code, map: null };
          },
        },
      },
    ],
  });
  const outputDir = path.dirname(configFile);
  const result = await bundle.write({
    dir: outputDir,
    format: isEsm ? 'esm' : 'cjs',
    sourcemap: 'inline',
    // respect the original file extension, mts -> mjs, cts -> cjs
    // mts should be generate mjs, it avoid add `type: module` at package.json
    entryFileNames: `rolldown.config.[hash]${
      path.extname(configFile).replace('ts', 'js')
    }`,
  });
  const fileName = result.output.find(
    (chunk): chunk is OutputChunk => chunk.type === 'chunk' && chunk.isEntry,
  )!.fileName;
  return path.join(outputDir, fileName);
}

const SUPPORTED_JS_CONFIG_FORMATS = ['.js', '.mjs', '.cjs'];
const SUPPORTED_TS_CONFIG_FORMATS = ['.ts', '.mts', '.cts'];
const SUPPORTED_CONFIG_FORMATS = [
  ...SUPPORTED_JS_CONFIG_FORMATS,
  ...SUPPORTED_TS_CONFIG_FORMATS,
];

const DEFAULT_CONFIG_BASE = 'rolldown.config';

export async function loadTsConfig(configFile: string): Promise<ConfigExport> {
  const isEsm = isFilePathESM(configFile);
  const file = await bundleTsConfig(configFile, isEsm);
  try {
    return (await import(pathToFileURL(file).href)).default;
  } finally {
    fs.unlink(file, () => {}); // Ignore errors
  }
}

export function isFilePathESM(filePath: string): boolean {
  if (/\.m[jt]s$/.test(filePath)) {
    return true;
  } else if (/\.c[jt]s$/.test(filePath)) {
    return false;
  } else {
    // check package.json for type: "module"
    const pkg = findNearestPackageData(path.dirname(filePath));
    if (pkg) {
      return pkg.type === 'module';
    }
    // no package.json, default to cjs
    return false;
  }
}

export function findNearestPackageData(basedir: string): any | null {
  while (basedir) {
    const pkgPath = path.join(basedir, 'package.json');
    if (tryStatSync(pkgPath)?.isFile()) {
      try {
        return JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));
      } catch {}
    }

    const nextBasedir = path.dirname(basedir);
    if (nextBasedir === basedir) break;
    basedir = nextBasedir;
  }

  return null;
}

function tryStatSync(file: string): fs.Stats | undefined {
  try {
    // The "throwIfNoEntry" is a performance optimization for cases where the file does not exist
    return fs.statSync(file, { throwIfNoEntry: false });
  } catch {
    // Ignore errors
  }
}

async function parseConfig(configPath: string): Promise<ConfigExport> {
  const ext = path.extname(configPath);

  if (
    SUPPORTED_JS_CONFIG_FORMATS.includes(ext) ||
    (process.env.NODE_OPTIONS?.includes('--import=tsx') &&
      SUPPORTED_TS_CONFIG_FORMATS.includes(ext))
  ) {
    return (await import(pathToFileURL(configPath).href)).default;
  } else if (SUPPORTED_TS_CONFIG_FORMATS.includes(ext)) {
    const rawConfigPath = path.resolve(configPath);
    return await loadTsConfig(rawConfigPath);
  } else {
    throw new Error(
      `Unsupported config format. Expected: \`${
        SUPPORTED_CONFIG_FORMATS.join(',')
      }\` but got \`${ext}\``,
    );
  }
}

export async function loadConfig(configPath: string): Promise<ConfigExport> {
  try {
    if (configPath) {
      return parseConfig(configPath);
    } else {
      const { config } = await loadUnConfig.async<ConfigExport>({
        sources: [
          {
            files: DEFAULT_CONFIG_BASE,
            extensions: SUPPORTED_CONFIG_FORMATS,
            parser: parseConfig,
          },
        ],
        cwd: cwd(),
        defaults: {},
      });
      return config;
    }
  } catch (err) {
    throw new Error('Error happened while loading config.', { cause: err });
  }
}
