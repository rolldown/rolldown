import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import type {
  ExperimentalInlineCommonChunksOptions,
  InputOptions,
  OutputChunk,
  OutputOptions,
  Plugin,
  RolldownOutput,
} from 'rolldown';
import { rolldown } from 'rolldown';

import { getOutputChunk } from '../utils';
import { expect } from 'vitest';

export type Modules = Record<string, string>;

export type Mode = 'off' | 'on';

export interface CaseOptions {
  input: Record<string, string>;
  modules: Modules;
  /** Used for the `on` build; `{ maxSize: 1 MiB }` unless given. */
  inline?: ExperimentalInlineCommonChunksOptions;
  output?: OutputOptions;
  inputOptions?: InputOptions;
  plugins?: Plugin[];
  /** Files written next to both builds' output, such as modules the build treats as external. */
  files?: Record<string, string>;
}

export interface Built {
  dir: string;
  output: RolldownOutput;
  chunks: OutputChunk[];
}

export interface RootResult {
  logs: string[];
  /** The exports of the last root. */
  exports: Record<string, string> | null;
  /** The first error thrown by a root. */
  error: unknown;
  /** Per root, by base name. */
  roots: { file: string; exports: Record<string, string> | null; error: unknown }[];
}

/** The tests' own folder: the root runner script lives there and the built cases go under its `dist/`. */
const CASES_ROOT = path.resolve(import.meta.dirname, '../../behaviors/inline-common-chunks');
const DIST_ROOT = path.join(CASES_ROOT, 'dist');
const RUNTIME_NAME = 'rolldown-runtime';

/** Serves the case's modules by id. A `.d.ts` id is served as JavaScript so its extension alone is what the bundler sees. */
export function virtualPlugin(modules: Modules): Plugin {
  return {
    name: 'virtual',
    resolveId(id) {
      return id in modules ? id : undefined;
    },
    load(id) {
      if (!(id in modules)) return undefined;
      return id.endsWith('.d.ts') ? { code: modules[id], moduleType: 'js' } : modules[id];
    },
  };
}

export async function buildCase(name: string, mode: Mode, options: CaseOptions): Promise<Built> {
  const dir = path.join(DIST_ROOT, name, mode);
  fs.rmSync(dir, { recursive: true, force: true });
  const bundle = await rolldown({
    input: options.input,
    preserveEntrySignatures: false,
    ...options.inputOptions,
    plugins: [virtualPlugin(options.modules), ...(options.plugins ?? [])],
  });
  const output = await bundle.write({
    dir,
    format: 'esm',
    strictExecutionOrder: true,
    // Stable names, so the same root file exists in both builds.
    entryFileNames: '[name].js',
    chunkFileNames: '[name].js',
    ...options.output,
    codeSplitting: {
      ...(options.output?.codeSplitting && typeof options.output.codeSplitting === 'object'
        ? options.output.codeSplitting
        : {}),
      ...(mode === 'on'
        ? { experimentalInlineCommonChunks: options.inline ?? { maxSize: 1 << 20 } }
        : {}),
    },
  });
  await bundle.close();
  // `#ext/<name>` resolves to `ext-<name>.js` next to the output: a bare specifier the bundler
  // leaves alone when it is external, and Node resolves through this map.
  fs.writeFileSync(
    path.join(dir, 'package.json'),
    '{ "type": "module", "imports": { "#ext/*": "./ext-*.js" } }\n',
  );
  for (const [name, code] of Object.entries(options.files ?? {})) {
    fs.writeFileSync(path.join(dir, name), code);
  }
  return {
    dir,
    output,
    chunks: getOutputChunk(output),
  };
}

export async function buildBoth(
  name: string,
  options: CaseOptions,
): Promise<{ off: Built; on: Built }> {
  return { off: await buildCase(name, 'off', options), on: await buildCase(name, 'on', options) };
}

export function runRoot(built: Built, fileName: string): RootResult {
  return runRoots(built, [fileName]);
}

/** Runs the files as roots, in order, in one fresh Node process. */
export function runRoots(built: { dir: string }, fileNames: string[]): RootResult {
  const proc = spawnSync(
    process.execPath,
    [path.join(CASES_ROOT, 'run-root.mjs'), ...fileNames.map((name) => path.join(built.dir, name))],
    { encoding: 'utf8' },
  );
  if (proc.status !== 0 || !proc.stdout) {
    throw new Error(`running ${fileNames.join(', ')} in ${built.dir} crashed:\n${proc.stderr}`);
  }
  return JSON.parse(proc.stdout.trim().split('\n').at(-1)!);
}

/** Configured and plugin-emitted entries, each run as the root of a fresh process. */
export function rootFiles(built: Built): string[] {
  return built.chunks
    .filter((chunk) => chunk.isEntry)
    .map((chunk) => chunk.fileName)
    .sort();
}

/** Runs every root of the `off` build in both builds and requires identical logs, exports and errors. */
export function compareRoots(pair: { off: Built; on: Built }, roots = rootFiles(pair.off)): void {
  expect(roots.length).toBeGreaterThan(0);
  for (const root of roots) {
    expect(runRoot(pair.on, root), `root ${root}`).toEqual(runRoot(pair.off, root));
  }
}

export function chunkContaining(built: Built, moduleSuffix: string): OutputChunk | undefined {
  return built.chunks.find((chunk) => chunk.moduleIds.some((id) => id.endsWith(moduleSuffix)));
}

const escapeRegExp = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * The ids the chunk registers: every call of the `__share` helper under its local name there. The
 * minifier prints the id as a template literal.
 */
function registrationIds(built: Built, chunk: OutputChunk): string[] {
  const share = runtimeLocalNames(built, chunk).__share;
  if (!share) return [];
  const pattern = new RegExp(`\\b${escapeRegExp(share)}\\(["\`]([^"\`]+)["\`],`, 'g');
  return [...chunk.code.matchAll(pattern)].map((match) => match[1]);
}

/** The chunks whose code registers a record factory. */
export function carriers(built: Built): OutputChunk[] {
  return built.chunks.filter((chunk) => registrationIds(built, chunk).length > 0);
}

/** The registry ids registered anywhere in the output. */
export function recordIds(built: Built): string[] {
  return [...new Set(built.chunks.flatMap((chunk) => registrationIds(built, chunk)))];
}

export function runtimeChunk(built: Built): OutputChunk | undefined {
  return built.chunks.find((item) => item.name === RUNTIME_NAME);
}

function runtimeLocalNames(built: Built, chunk: OutputChunk): Record<string, string> {
  const runtime = runtimeChunk(built);
  if (!runtime) return {};
  // `export { __share as n, ... }` in the runtime chunk: alias -> helper.
  const aliasToHelper = new Map<string, string>();
  for (const match of runtime.code.matchAll(/export\s*\{([^}]*)\}/g)) {
    for (const specifier of match[1].split(',')) {
      const [helper, , alias] = specifier.trim().split(/\s+/);
      if (helper) aliasToHelper.set(alias ?? helper, helper);
    }
  }
  const locals: Record<string, string> = {};
  const importPattern = new RegExp(
    `import\\s*\\{([^}]*)\\}\\s*from\\s*"\\./${escapeRegExp(runtime.fileName)}";?`,
    'g',
  );
  for (const match of chunk.code.matchAll(importPattern)) {
    for (const specifier of match[1].split(',')) {
      const [alias, , local] = specifier.trim().split(/\s+/);
      const helper = aliasToHelper.get(alias);
      if (helper) locals[helper] = local ?? alias;
    }
  }
  return locals;
}

/** Checks local registration order and the runtime import on unminified output. */
export function assertRegistrationOrder(built: Built): void {
  const runtime = runtimeChunk(built);
  const allIds = new Set(recordIds(built));
  const events = (chunk: OutputChunk): { kind: 'share' | 'require'; id: string }[] => {
    const locals = runtimeLocalNames(built, chunk);
    const share = locals.__share;
    const require = locals.__share_require;
    if (!share && !require) return [];
    const names = [share, require].filter(Boolean).join('|');
    const pattern = new RegExp(`\\b(${names})\\("([^"]+)"`, 'g');
    return [...chunk.code.matchAll(pattern)].map((match) => ({
      kind: match[1] === share ? 'share' : 'require',
      id: match[2],
    }));
  };
  for (const chunk of built.chunks) {
    if (chunk === runtime) continue;
    const own = events(chunk);
    if (!own.some((event) => event.kind === 'require')) {
      expect(
        registrationIds(built, chunk),
        `${chunk.fileName} registers nothing and reads nothing`,
      ).toEqual([]);
      continue;
    }
    const firstImport = chunk.code.match(/^import [^\n]*?from "([^"]+)";/m);
    expect(firstImport?.[1], `${chunk.fileName} imports the runtime chunk first`).toBe(
      `./${runtime!.fileName}`,
    );
    const ownIds = new Set(registrationIds(built, chunk));
    const available = new Set<string>();
    for (const event of own) {
      if (event.kind === 'share') {
        available.add(event.id);
      } else {
        expect(
          ownIds.has(event.id) ? available.has(event.id) : allIds.has(event.id),
          `${chunk.fileName}: __share_require("${event.id}") before any registration it can see`,
        ).toBe(true);
      }
    }
  }
}

/** The `off` output keeps `moduleSuffix` in a non-entry chunk; the `on` output must too when the rule rejects it. */
export function expectKeptAsFile(pair: { off: Built; on: Built }, moduleSuffix: string): void {
  const off = chunkContaining(pair.off, moduleSuffix);
  const on = chunkContaining(pair.on, moduleSuffix);
  expect(off, `off build has a chunk for ${moduleSuffix}`).toBeDefined();
  expect(on, `on build keeps a chunk for ${moduleSuffix}`).toBeDefined();
  expect(on!.isEntry || on!.isDynamicEntry).toBe(off!.isEntry || off!.isDynamicEntry);
  expect(carriers(pair.on), 'nothing became a record').toEqual([]);
}

export function expectInlined(pair: { off: Built; on: Built }, moduleSuffix: string): void {
  const off = chunkContaining(pair.off, moduleSuffix);
  expect(off, `off build has a chunk for ${moduleSuffix}`).toBeDefined();
  expect(off!.isEntry || off!.isDynamicEntry).toBe(false);
  // The module's own file is gone; every file that lists the module prints its factory.
  const onChunks = pair.on.chunks.filter((chunk) =>
    chunk.moduleIds.some((id) => id.endsWith(moduleSuffix)),
  );
  expect(onChunks.length).toBeGreaterThan(0);
  const carrierNames = carriers(pair.on).map((chunk) => chunk.fileName);
  for (const chunk of onChunks) {
    expect(carrierNames, `${chunk.fileName} prints the factory it lists`).toContain(chunk.fileName);
  }
  expect(pair.on.chunks.map((chunk) => chunk.name)).not.toContain(off!.name);
}
