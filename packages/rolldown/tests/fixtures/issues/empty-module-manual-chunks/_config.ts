import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Regression: empty module in named chunk via manualChunks produced
// `export { empty_exports as t }` without ever defining `empty_exports`.
// Note: virtual module IDs must NOT use \0 prefix — it prevents chunk creation.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'empty-module',
        resolveId(source) {
          if (source === 'fake-pkg/empty') return 'fake-pkg/empty';
        },
        load(id) {
          if (id === 'fake-pkg/empty') return '';
        },
      },
    ],
    output: {
      manualChunks: (id) => {
        if (id.includes('fake-pkg')) {
          return 'fake-pkg';
        }
      },
    },
  },
  afterTest(output) {
    for (const chunk of output.output) {
      if (chunk.type !== 'chunk') continue;
      // Every exported identifier must be defined in the chunk's code.
      const exportMatch = chunk.code.match(/export\s*\{([^}]+)\}/);
      if (!exportMatch) continue;
      const exportedNames = exportMatch[1].split(',').map((s) =>
        s
          .trim()
          .split(/\s+as\s+/)[0]
          .trim(),
      );
      for (const name of exportedNames) {
        expect(
          chunk.code,
          `exported "${name}" should be defined in chunk "${chunk.fileName}"`,
        ).toContain(`var ${name}`);
      }
    }
  },
});
