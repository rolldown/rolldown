import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `@rollup/plugin-alias` returns the object from `this.resolve()`. With treeshake
// off that object now carries the effective `moduleSideEffects: true`. A hook
// `true` must count as no value in that case: `preserveModules` keeps
// `import './pure.js'`, and getModuleInfo / this.resolve still report `true`.
function isPure(id: string): boolean {
  return id.replaceAll('\\', '/').endsWith('/pure.js');
}

export default defineTest({
  config: {
    input: 'main.js',
    treeshake: false,
    output: { minify: false, preserveModules: true },
    plugins: [
      {
        name: 'alias-returns-resolve',
        async resolveId(source, importer) {
          if (source !== 'aliased-pure') return;
          const resolved = await this.resolve('./pure.js', importer, { skipSelf: true });
          if (resolved == null) {
            throw new Error('failed to resolve ./pure.js');
          }
          return resolved;
        },
        async buildEnd() {
          let pureId: string | undefined;
          for (const id of this.getModuleIds()) {
            if (isPure(id)) pureId = id;
          }
          expect(pureId, 'pure.js was bundled').toBeTruthy();
          const info = this.getModuleInfo(pureId!);
          const resolved = await this.resolve(pureId!);
          expect(info?.moduleSideEffects, 'getModuleInfo').toBe(true);
          expect(resolved?.moduleSideEffects, 'this.resolve').toBe(true);
        },
      },
    ],
  },
  afterTest(output) {
    const entry = output.output.find(
      (chunk) => chunk.type === 'chunk' && chunk.fileName.replaceAll('\\', '/').endsWith('main.js'),
    );
    if (entry?.type !== 'chunk') {
      throw new Error('missing main.js chunk');
    }
    // The side-effect-free module stays imported. Statement analysis would drop this.
    expect(entry.code).toMatch(/import\s+["'][^"']*pure\.js["']/);
  },
});
