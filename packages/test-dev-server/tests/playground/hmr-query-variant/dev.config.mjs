import { defineDevConfig } from '@rolldown/test-dev-server';
import nodeFs from 'node:fs';
import nodePath from 'node:path';

// A plugin that serves `<file>?upper` as a module whose content is derived from
// `<file>` in its `load` hook — the html-proxy / `foo.vue?vue&type=...` pattern.
// Editing `<file>` must re-fetch the variant, or its cached copy goes stale.
function queryVariantPlugin() {
  return {
    name: 'query-variant-plugin',
    resolveId(source, importer) {
      if (source.endsWith('?upper') && importer) {
        const cleanPath = nodePath.join(
          nodePath.dirname(importer),
          source.slice(0, -'?upper'.length),
        );
        return `${cleanPath}?upper`;
      }
      return null;
    },
    load(id) {
      if (!id.endsWith('?upper')) {
        return null;
      }
      const cleanPath = id.slice(0, -'?upper'.length);
      const code = nodeFs.readFileSync(cleanPath, 'utf-8');
      const message = code.match(/msg = '(\w+)'/)[1];
      return { code: `export const upper = ${JSON.stringify(message.toUpperCase())};` };
    },
  };
}

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: {},
    },
    plugins: [queryVariantPlugin()],
  },
});
