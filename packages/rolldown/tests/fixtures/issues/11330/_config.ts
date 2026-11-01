import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// https://github.com/rolldown/rolldown/issues/11330
// `getModuleInfo` and `this.resolve` report the moduleSideEffects value Rolldown applies:
// hook, then treeshake.moduleSideEffects, then package.json `sideEffects`, then `true`.
const expected: Record<string, boolean | 'no-treeshake'> = {
  'index.js': true,
  'local.js': false,
  'hooked.js': 'no-treeshake',
  'dep/a.js': false,
  'dep/b.js': false,
  'dep/index.js': false,
};

function label(id: string): string | undefined {
  const norm = id.replaceAll('\\', '/');
  if (norm.endsWith('/dep/a.js')) return 'dep/a.js';
  if (norm.endsWith('/dep/b.js')) return 'dep/b.js';
  if (norm.endsWith('/dep/index.js')) return 'dep/index.js';
  if (norm.endsWith('/local.js')) return 'local.js';
  if (norm.endsWith('/hooked.js')) return 'hooked.js';
  if (norm.endsWith('/index.js')) return 'index.js';
  return undefined;
}

export default defineTest({
  config: {
    input: 'index.js',
    treeshake: {
      moduleSideEffects(id) {
        if (id.replaceAll('\\', '/').endsWith('/local.js')) return false;
        return undefined;
      },
    },
    plugins: [
      {
        name: 'effective-module-side-effects',
        resolveId(source) {
          if (source.replaceAll('\\', '/').endsWith('hooked.js')) {
            return {
              id: path.join(import.meta.dirname, 'hooked.js'),
              moduleSideEffects: 'no-treeshake',
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
            expect(resolved?.moduleSideEffects, `this.resolve ${name}`).toBe(expected[name]);
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
    expect(code).not.toContain('pkg-a-side-effect');
    expect(code).not.toContain('local-side-effect');
    expect(code).toContain('hooked-side-effect');
  },
});
