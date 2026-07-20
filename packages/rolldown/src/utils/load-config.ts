import fs from 'node:fs';
import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { cwd } from 'node:process';
import { pathToFileURL } from 'node:url';
import { rolldown } from '../api/rolldown';
import type { ConfigExport } from './define-config';
import type { OutputChunk } from '../types/rolldown-output';

interface BundledConfig {
  /** Absolute path of the generated entry chunk. */
  entryFile: string;
  /** Absolute paths of every file the config build emitted. */
  generatedFiles: string[];
}

/**
 * Remove every file the config build emitted, collecting instead of throwing so
 * a cleanup failure never hides the failure that triggered the cleanup.
 */
async function removeGeneratedFiles(generatedFiles: string[]): Promise<unknown[]> {
  const errors: unknown[] = [];
  for (const generatedFile of generatedFiles) {
    try {
      await fs.promises.rm(generatedFile, { force: true });
    } catch (error) {
      errors.push(error);
    }
  }
  return errors;
}

async function bundleTsConfig(configFile: string, isEsm: boolean): Promise<BundledConfig> {
  const dirnameVarName = 'injected_original_dirname';
  const filenameVarName = 'injected_original_filename';
  const importMetaUrlVarName = 'injected_original_import_meta_url';
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
  // The generated module has to sit next to the original config file: configs may
  // keep runtime-relative resolution that Rolldown leaves untouched (for example
  // `require.resolve('./helper')`), and Node resolves those against the directory
  // the generated file lives in. Writing into a nested temporary directory would
  // break them, so every emitted file is tracked and removed individually instead.
  const outputDir = path.dirname(configFile);
  let entryFile: string | undefined;
  let generatedFiles: string[] = [];
  let bundleFailed = false;
  let operationError: unknown;
  try {
    const result = await bundle.write({
      dir: outputDir,
      format: isEsm ? 'esm' : 'cjs',
      codeSplitting: false,
      sourcemap: 'inline',
      // respect the original file extension, mts -> mjs, cts -> cjs
      // mts should be generate mjs, it avoid add `type: module` at package.json
      entryFileNames: `rolldown.config.[hash]${path.extname(configFile).replace('ts', 'js')}`,
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
    bundleFailed = true;
    operationError = error;
  }

  let closeFailed = false;
  let closeError: unknown;
  try {
    await bundle.close();
  } catch (error) {
    closeFailed = true;
    closeError = error;
  }

  const errors: unknown[] = [];
  if (bundleFailed) errors.push(operationError);
  if (closeFailed) errors.push(closeError);
  if (errors.length > 0) {
    errors.push(...(await removeGeneratedFiles(generatedFiles)));
  }

  throwCollectedErrors(errors, 'Config bundling and cleanup both failed');
  return { entryFile: entryFile!, generatedFiles };
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
  let config: ConfigExport | undefined;
  // Track completion separately from the rejection value: a config is free to
  // reject with any value, `undefined` included, and that still is a failure.
  let imported = false;
  let importError: unknown;
  try {
    config = (await import(pathToFileURL(entryFile).href)).default;
    imported = true;
  } catch (error) {
    importError = error;
  }

  const errors: unknown[] = [];
  if (!imported) errors.push(importError);
  errors.push(...(await removeGeneratedFiles(generatedFiles)));
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
