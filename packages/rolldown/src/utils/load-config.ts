import { randomBytes } from 'node:crypto';
import fs from 'node:fs';
import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { cwd } from 'node:process';
import { pathToFileURL } from 'node:url';
import { rolldown } from '../api/rolldown';
import type { ConfigExport } from './define-config';
import type { OutputChunk } from '../types/rolldown-output';

interface BundledConfig {
  entryFile: string;
  generatedFiles: string[];
}

/** Returns the removal errors, so a cleanup failure never hides the error that caused it. */
async function removeFiles(files: string[]): Promise<unknown[]> {
  const results = await Promise.allSettled(
    files.map((file) => fs.promises.rm(file, { force: true })),
  );
  return results.flatMap((result) => (result.status === 'rejected' ? [result.reason] : []));
}

let configLoadCount = 0;

async function bundleTsConfig(configFile: string, isEsm: boolean): Promise<BundledConfig> {
  const dirnameVarName = 'injected_original_dirname';
  const filenameVarName = 'injected_original_filename';
  const importMetaUrlVarName = 'injected_original_import_meta_url';
  // Before `rolldown()`: a throw here must not skip `close()`.
  // A unique name per load: the counter defeats the process-wide module cache
  // (keyed by URL), the random part defeats other processes loading the same config.
  const outputDir = path.dirname(configFile);
  const outputPrefix = `rolldown.config.${++configLoadCount}.${randomBytes(8).toString('hex')}.`;
  const bundle = await rolldown({
    input: configFile,
    platform: 'node',
    resolve: {
      mainFields: ['main'],
    },
    transform: {
      define: {
        __dirname: dirnameVarName,
        __filename: filenameVarName,
        'import.meta.url': importMetaUrlVarName,
        'import.meta.dirname': dirnameVarName,
        'import.meta.filename': filenameVarName,
      },
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
              `const ${importMetaUrlVarName} = ${JSON.stringify(pathToFileURL(id).href)};`;
            return { code: injectValues + code, map: null };
          },
        },
      },
    ],
  });
  const errors: unknown[] = [];
  let entryFile: string | undefined;
  let generatedFiles: string[] | undefined;
  try {
    const result = await bundle.write({
      dir: outputDir,
      format: isEsm ? 'esm' : 'cjs',
      // One file: a deferred config function may still `import()` after cleanup.
      codeSplitting: false,
      sourcemap: 'inline',
      // respect the original file extension, mts -> mjs, cts -> cjs
      // mts should be generate mjs, it avoid add `type: module` at package.json
      entryFileNames: `${outputPrefix}${path.extname(configFile).replace('ts', 'js')}`,
    });
    generatedFiles = result.output.map((output) => path.join(outputDir, output.fileName));
    const fileName = result.output.find(
      (chunk): chunk is OutputChunk => chunk.type === 'chunk' && chunk.isEntry,
    )?.fileName;
    if (fileName === undefined) {
      throw new Error(`Rolldown did not emit an entry chunk for config file "${configFile}"`);
    }
    entryFile = path.join(outputDir, fileName);
  } catch (error) {
    errors.push(error);
  }

  try {
    await bundle.close();
  } catch (error) {
    errors.push(error);
  }

  if (errors.length > 0) {
    try {
      generatedFiles ??= (await readdir(outputDir))
        .filter((name) => name.startsWith(outputPrefix))
        .map((name) => path.join(outputDir, name));
      errors.push(...(await removeFiles(generatedFiles)));
    } catch (error) {
      errors.push(error);
    }
    throwCollectedErrors(errors, 'Config bundling and cleanup both failed');
  }
  return { entryFile: entryFile!, generatedFiles: generatedFiles! };
}

const SUPPORTED_JS_CONFIG_FORMATS = ['.js', '.mjs', '.cjs'];
const SUPPORTED_TS_CONFIG_FORMATS = ['.ts', '.mts', '.cts'];
const SUPPORTED_CONFIG_FORMATS = [...SUPPORTED_JS_CONFIG_FORMATS, ...SUPPORTED_TS_CONFIG_FORMATS];

const DEFAULT_CONFIG_BASE = 'rolldown.config';

async function findConfigFileNameInCwd(): Promise<string> {
  const filesInWorkingDirectory = new Set(await readdir(cwd()));
  for (const extension of SUPPORTED_CONFIG_FORMATS) {
    const fileName = `${DEFAULT_CONFIG_BASE}${extension}`;
    if (filesInWorkingDirectory.has(fileName)) return fileName;
  }
  throw new Error('No `rolldown.config` configuration file found.');
}

async function loadTsConfig(configFile: string): Promise<ConfigExport> {
  const isEsm = isFilePathESM(configFile);
  const { entryFile, generatedFiles } = await bundleTsConfig(configFile, isEsm);
  const errors: unknown[] = [];
  let config: ConfigExport | undefined;
  try {
    config = (await import(pathToFileURL(entryFile).href)).default;
  } catch (error) {
    errors.push(error);
  }
  errors.push(...(await removeFiles(generatedFiles)));
  throwCollectedErrors(errors, 'Config import and cleanup both failed');
  return config!;
}

function throwCollectedErrors(errors: unknown[], message: string): void {
  if (errors.length > 1) {
    throw new AggregateError(errors, message, { cause: errors[0] });
  }
  if (errors.length === 1) throw errors[0];
}

function isFilePathESM(filePath: string): boolean {
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

function findNearestPackageData(basedir: string): any {
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

export type ConfigLoader = 'bundle' | 'native';

export interface LoadConfigOptions {
  /**
   * How to load the config file.
   * - `'bundle'` (default): bundle the config with Rolldown, then import it.
   * - `'native'`: import the config directly, delegating TypeScript/loader
   *   handling to the runtime. Faster, but requires runtime support.
   *
   * @default 'bundle'
   */
  configLoader?: ConfigLoader;
}

async function loadNativeConfig(resolvedPath: string): Promise<ConfigExport> {
  const url = pathToFileURL(resolvedPath).href;
  const { freshImport } = await import('fresh-import');
  const freshImported = freshImport(url);
  if (freshImported) {
    const { result } = await freshImported;
    return (result as { [Symbol.toStringTag]: 'Module'; default: ConfigExport }).default;
  }
  // Runtimes without Module-hook support (e.g. Bun/Deno)
  const mod = await import(url + '?t=' + Date.now());
  return mod.default;
}

/**
 * Load config from a file in a way that Rolldown does.
 *
 * @param configPath The path to the config file. If empty, it will look for `rolldown.config` with supported extensions in the current working directory.
 * @param options Loading options. `configLoader` selects `'bundle'` (default) or `'native'`.
 * @returns The loaded config export
 *
 * @category Config
 */
export async function loadConfig(
  configPath: string,
  options: LoadConfigOptions = {},
): Promise<ConfigExport> {
  const configLoader = options.configLoader ?? 'bundle';
  const ext = path.extname((configPath = configPath || (await findConfigFileNameInCwd())));

  try {
    if (configLoader === 'native') {
      return await loadNativeConfig(path.resolve(configPath));
    }

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
        `Unsupported config format. Expected: \`${SUPPORTED_CONFIG_FORMATS.join(
          ',',
        )}\` but got \`${ext}\``,
      );
    }
  } catch (err) {
    if (configLoader === 'native') {
      const isTsConfig = SUPPORTED_TS_CONFIG_FORMATS.includes(ext);
      const tsHint =
        isTsConfig && !process.features.typescript
          ? ' This runtime does not natively support TypeScript config files.'
          : '';
      throw new Error(
        `Failed to load the config file "${configPath}" using the "native" config loader.${tsHint} ` +
          `Try "--configLoader bundle", or register a loader such as "--import tsx".`,
        { cause: err },
      );
    }
    throw new Error('Error happened while loading config.', { cause: err });
  }
}
