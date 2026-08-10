import { randomBytes } from 'node:crypto';
import fs from 'node:fs';
import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { cwd } from 'node:process';
import { pathToFileURL } from 'node:url';
import { rolldown } from '../api/rolldown';
import type { ConfigExport } from './define-config';
import type { OutputChunk } from '../types/rolldown-output';
import { onExit } from './signal-exit';

interface BundledConfig {
  /** Absolute path of the emitted entry module. */
  outputFile: string;
  /** Absolute paths of every emitted file (entry included), for cleanup. */
  outputFiles: string[];
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
  // Must be emitted beside the config file: the emitted module's URL is the
  // resolution base for any runtime-computed relative dynamic import a
  // deferred config function performs later. The per-call random token keeps
  // concurrent loads collision-free.
  const outputDir = path.dirname(configFile);
  const outputPrefix = `.rolldown.config.${randomBytes(4).toString('hex')}.`;
  let outputFile: string | undefined;
  let outputFiles: string[] | undefined;
  let operationError: unknown;
  try {
    const result = await bundle.write({
      dir: outputDir,
      format: isEsm ? 'esm' : 'cjs',
      // Single-file output: everything a deferred config function may import
      // later is either inlined into the entry or resolved from the user's
      // own files beside the config, never from a transient sibling chunk.
      codeSplitting: false,
      sourcemap: 'inline',
      // respect the original file extension, mts -> mjs, cts -> cjs
      // mts should be generate mjs, it avoid add `type: module` at package.json
      entryFileNames: `${outputPrefix}[hash]${path.extname(configFile).replace('ts', 'js')}`,
    });
    const entryFileName = result.output.find(
      (chunk): chunk is OutputChunk => chunk.type === 'chunk' && chunk.isEntry,
    )?.fileName;
    if (entryFileName === undefined) {
      throw new Error(`Rolldown did not emit an entry chunk for config file "${configFile}"`);
    }
    outputFiles = result.output.map(({ fileName }) => path.join(outputDir, fileName));
    outputFile = path.join(outputDir, entryFileName);
  } catch (error) {
    operationError = error;
  }

  let closeError: unknown;
  try {
    await bundle.close();
  } catch (error) {
    closeError = error;
  }

  const errors: unknown[] = [];
  if (operationError !== undefined) errors.push(operationError);
  if (closeError !== undefined) errors.push(closeError);
  if (errors.length > 0) {
    // Nothing was (or will be) imported, so every file this call emitted can
    // be removed right away. If the write itself failed the emitted names are
    // unknown; fall back to the per-call entry prefix.
    try {
      const files =
        outputFiles ??
        (await readdir(outputDir))
          .filter((name) => name.startsWith(outputPrefix))
          .map((name) => path.join(outputDir, name));
      await removeBundledFiles(files);
    } catch (error) {
      errors.push(error);
    }
  }

  throwCollectedErrors(errors, 'Config bundling and cleanup both failed');
  return { outputFile: outputFile!, outputFiles: outputFiles! };
}

async function removeBundledFiles(files: string[]): Promise<void> {
  await Promise.all(files.map((file) => fs.promises.rm(file, { force: true })));
}

const filesPendingRemovalAtExit = new Set<string>();

/**
 * Defer removal of emitted files a deferred config function might still
 * runtime-import. With `codeSplitting: false` this should never receive any
 * file; it is a safety net so cleanup cannot break a deferred dynamic import.
 */
function scheduleRemovalAtExit(files: string[]): void {
  if (files.length === 0) return;
  if (filesPendingRemovalAtExit.size === 0) {
    onExit(() => {
      for (const file of filesPendingRemovalAtExit) {
        try {
          fs.rmSync(file, { force: true });
        } catch {
          // Ignore errors: best-effort cleanup while the process is exiting.
        }
      }
    });
  }
  for (const file of files) filesPendingRemovalAtExit.add(file);
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
  const { outputFile, outputFiles } = await bundleTsConfig(configFile, isEsm);
  let config: ConfigExport | undefined;
  let importError: unknown;
  try {
    config = (await import(pathToFileURL(outputFile).href)).default;
  } catch (error) {
    importError = error;
  }

  let cleanupError: unknown;
  try {
    if (importError === undefined) {
      // The entry is in memory now, so its file can go. Any other emitted file
      // must survive until process exit: a deferred config function the caller
      // invokes after this returns may still runtime-import it.
      await removeBundledFiles([outputFile]);
      scheduleRemovalAtExit(outputFiles.filter((file) => file !== outputFile));
    } else {
      await removeBundledFiles(outputFiles);
    }
  } catch (error) {
    cleanupError = error;
  }

  const errors: unknown[] = [];
  if (importError !== undefined) errors.push(importError);
  if (cleanupError !== undefined) errors.push(cleanupError);
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
