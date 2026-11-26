import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Rollup sets `hasModuleSideEffects` to `() => true` when `treeshake` is false
// (`ModuleLoader` constructor). With no hook value, every module, including an
// external, reports `true`. An explicit hook value is left alone. `this.resolve` reports only
// its own resolution: it skips the `resolveId` hook of this plugin, so it reports `true`.
const expected: Record<string, boolean | 'no-treeshake'> = {
  'main.js': true,
  'effect.js': true,
  'hooked.js': 'no-treeshake',
  'pure.js': false,
  path: true,
};

function label(id: string): string | undefined {
  if (id === 'path') return 'path';
  const norm = id.replaceAll('\\', '/');
  if (norm.endsWith('/main.js')) return 'main.js';
  if (norm.endsWith('/effect.js')) return 'effect.js';
  if (norm.endsWith('/hooked.js')) return 'hooked.js';
  if (norm.endsWith('/pure.js')) return 'pure.js';
  return undefined;
}

export default defineTest({
  config: {
    input: 'main.js',
    external: ['path'],
    treeshake: false,
    plugins: [
      {
        name: 'report-treeshake-false-module-side-effects',
        resolveId(source) {
          const norm = source.replaceAll('\\', '/');
          if (norm.endsWith('/hooked.js') || norm.endsWith('hooked.js')) {
            return {
              id: path.join(import.meta.dirname, 'hooked.js'),
              moduleSideEffects: 'no-treeshake',
            };
          }
          if (norm.endsWith('/pure.js') || norm.endsWith('pure.js')) {
            return {
              id: path.join(import.meta.dirname, 'pure.js'),
              moduleSideEffects: false,
            };
          }
        },
        async buildEnd() {
          const seen = new Set<string>();
          for (const id of this.getModuleIds()) {
            const name = label(id);
            if (name == null) continue;
            seen.add(name);
            const info = this.getModuleInfo(id);
            const resolved = await this.resolve(id);
            expect(info?.moduleSideEffects, `getModuleInfo ${name}`).toBe(expected[name]);
            expect(resolved?.moduleSideEffects, `this.resolve ${name}`).toBe(
              name === 'hooked.js' || name === 'pure.js' ? true : expected[name],
            );
          }
          expect([...seen].sort()).toEqual(Object.keys(expected).sort());
        },
      },
    ],
  },
  afterTest(output) {
    const code = output.output
      .map((chunk) => (chunk.type === 'chunk' ? chunk.code : ''))
      .join('\n');
    // `treeshake: false` still keeps the module. Reporting `true` does not drop it.
    expect(code).toContain('treeshake-false-effect');
    expect(code).toContain('"path"');
  },
});
