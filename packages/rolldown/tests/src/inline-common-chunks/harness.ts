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
}

export interface Built {
  dir: string;
  output: RolldownOutput;
  chunks: OutputChunk[];
}

export interface RootResult {
  logs: string[];
  exports: Record<string, string> | null;
  error: unknown;
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
      experimentalInlineCommonChunks:
        mode === 'on' ? (options.inline ?? { maxSize: 1 << 20 }) : { maxSize: 0 },
    },
  });
  await bundle.close();
  fs.writeFileSync(path.join(dir, 'package.json'), '{ "type": "module" }\n');
  return {
    dir,
    output,
    chunks: output.output.filter((item): item is OutputChunk => item.type === 'chunk'),
  };
}

export async function buildBoth(
  name: string,
  options: CaseOptions,
): Promise<{ off: Built; on: Built }> {
  return { off: await buildCase(name, 'off', options), on: await buildCase(name, 'on', options) };
}

export function runRoot(built: Built, fileName: string): RootResult {
  const proc = spawnSync(
    process.execPath,
    [path.join(CASES_ROOT, 'run-root.mjs'), path.join(built.dir, fileName)],
    { encoding: 'utf8' },
  );
  if (proc.status !== 0 || !proc.stdout) {
    throw new Error(`running ${fileName} in ${built.dir} crashed:\n${proc.stderr}`);
  }
  return JSON.parse(proc.stdout.trim().split('\n').at(-1)!);
}

/** Every entry and dynamic entry file of a build, each of which the differential test runs as a root. */
export function rootFiles(built: Built): string[] {
  return built.chunks
    .filter((chunk) => chunk.isEntry || chunk.isDynamicEntry)
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

/** The chunks whose code registers a record factory. */
export function carriers(built: Built): OutputChunk[] {
  return built.chunks.filter((chunk) => /\b\w+\("[^"]+", \(\w+\) => \{/.test(chunk.code));
}

export function runtimeChunk(built: Built): OutputChunk | undefined {
  return built.chunks.find((item) => item.name === RUNTIME_NAME);
}

function runtimeLocalNames(built: Built, chunk: OutputChunk): Record<string, string> {
  const runtime = runtimeChunk(built);
  if (!runtime) return {};
  // `export { __share as n, ... }` in the runtime chunk: alias -> helper.
  const aliasToHelper = new Map<string, string>();
  for (const match of runtime.code.matchAll(/export \{([^}]*)\}/g)) {
    for (const specifier of match[1].split(',')) {
      const [helper, , alias] = specifier.trim().split(/\s+/);
      if (helper) aliasToHelper.set(alias ?? helper, helper);
    }
  }
  const locals: Record<string, string> = {};
  const importPattern = new RegExp(
    `import \\{([^}]*)\\} from "\\./${runtime.fileName.replace(/[.\\-]/g, '\\$&')}";`,
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

/**
 * In every file, each `__share_require(id)` must come after `__share(id, ...)` in the same file,
 * and the runtime chunk must be the first import. Only meaningful on an unminified build.
 */
export function assertRegistrationOrder(built: Built): void {
  const runtime = runtimeChunk(built);
  for (const chunk of built.chunks) {
    if (chunk === runtime) continue;
    const locals = runtimeLocalNames(built, chunk);
    const share = locals.__share;
    const require = locals.__share_require;
    if (!require) {
      expect(chunk.code, `${chunk.fileName} registers nothing and reads nothing`).not.toMatch(
        /\b\w+\("[^"]+", \(\w+\) => \{/,
      );
      continue;
    }
    expect(share, `${chunk.fileName} reads a record so it must also register`).toBeTruthy();
    const firstImport = chunk.code.match(/^import [^\n]*?from "([^"]+)";/m);
    expect(firstImport?.[1], `${chunk.fileName} imports the runtime chunk first`).toBe(
      `./${runtime!.fileName}`,
    );
    const registered = new Set<string>();
    const pattern = new RegExp(`\\b(${share}|${require})\\("([^"]+)"`, 'g');
    for (const match of chunk.code.matchAll(pattern)) {
      if (match[1] === share) {
        registered.add(match[2]);
      } else {
        expect(
          registered.has(match[2]),
          `${chunk.fileName}: __share_require("${match[2]}") before its registration`,
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
  // Every file that lists the module carries it; the module's own file is gone.
  const onChunks = pair.on.chunks.filter((chunk) =>
    chunk.moduleIds.some((id) => id.endsWith(moduleSuffix)),
  );
  expect(onChunks.length).toBeGreaterThan(0);
  for (const chunk of onChunks) {
    expect(chunk.isEntry || chunk.isDynamicEntry, `${chunk.fileName} is a real file`).toBe(true);
  }
  expect(pair.on.chunks.map((chunk) => chunk.name)).not.toContain(off!.name);
}
