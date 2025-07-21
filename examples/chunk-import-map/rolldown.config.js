import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'rolldown';

const dirname = path.dirname(fileURLToPath(import.meta.url));

function highlightDiff(oldStr, newStr) {
  const regex = /"([^"]*)"/g;

  const oldMatches = [...oldStr.matchAll(regex)].map(m => m[0]);
  const newMatches = [...newStr.matchAll(regex)].map(m => m[0]);

  const marked = newMatches.map((segment, i) => {
    if (segment !== oldMatches[i]) {
      return `<mark>${segment}</mark>`;
    }
    return segment;
  });

  let result = newStr, idx = 0;
  result = result.replace(regex, () => marked[idx++] || '');
  return result;
}

export default defineConfig({
  input: {
    entry: './entry.ts',
  },
  experimental: {
    chunkImportMap: {
      baseUrl: '/dist/',
      fileName: 'importmap.json',
    },
  },
  plugins: [
    {
      name: 'inject-import-map',
      async generateBundle(_, bundle) {
        const chunkImportMap = bundle['importmap.json'];
        if (chunkImportMap?.type === 'asset') {
          const importMap = chunkImportMap.source;
          const htmlPath = path.resolve(dirname, 'index.html');

          try {
            let html = fs.readFileSync(htmlPath, 'utf-8');

            html = html.replace(
              /<script\s+type="importmap"[^>]*>[\s\S]*?<\/script>/i,
              `<script type="importmap">${importMap}</script>`,
            );

            let oldImportMap = importMap;
            try {
              oldImportMap = fs.readFileSync(
                path.resolve(dirname, './dist/importmap.json'),
                'utf-8',
              );
            } catch {}

            html = html.replace(
              /<div\s+id="importmap"[^>]*>[\s\S]*?<\/div>/i,
              `<div id="importmap">${
                highlightDiff(oldImportMap, importMap)
              }</div>`,
            );

            fs.writeFileSync(htmlPath, html);
            console.info(`[plugin] updated importmap in index.html`);
          } catch (err) {
            console.warn(`[plugin] failed to patch index.html:`, err);
          }
        }
      },
    },
  ],
});
