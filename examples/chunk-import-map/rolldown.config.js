import fs from 'node:fs';
import path from 'node:path';
import { defineConfig } from 'rolldown';

const URL_BASE = '/dist/';

function highlightDiff(oldStr, newStr) {
  const lenOld = oldStr.length;
  const lenNew = newStr.length;
  let start = 0;
  while (start < lenOld && start < lenNew && oldStr[start] === newStr[start]) {
    start++;
  }
  let endOld = lenOld - 1;
  let endNew = lenNew - 1;
  while (
    endOld >= start &&
    endNew >= start &&
    oldStr[endOld] === newStr[endNew]
  ) {
    endOld--;
    endNew--;
  }
  if (start > endNew) {
    return newStr;
  }

  let startMark = start;
  while (startMark > 0 && newStr[startMark] !== '"') {
    startMark--;
  }
  let endMark = endNew;
  while (endMark < lenNew && newStr[endMark] !== '"') {
    endMark++;
  }
  endMark = Math.min(endMark + 1, lenNew);

  return (
    newStr.slice(0, startMark) +
    '<mark>' +
    newStr.slice(startMark, endMark) +
    '</mark>' +
    newStr.slice(endMark)
  );
}

export default defineConfig({
  input: {
    entry: './entry.ts',
  },
  experimental: {
    chunkImportMap: true,
  },
  plugins: [
    {
      name: 'test',
      async generateBundle(_, bundle) {
        const chunkImportMap = bundle['.chunk-import-map.json'];
        if (chunkImportMap?.type === 'asset') {
          const importMap = JSON.stringify({
            imports: Object.fromEntries(
              Object.entries(JSON.parse(chunkImportMap.source)).map((
                [key, value],
              ) => [
                path.posix.join(URL_BASE, key),
                path.posix.join(URL_BASE, value),
              ]),
            ),
          });

          const htmlPath = path.resolve(import.meta.dirname, 'index.html');
          try {
            let html = fs.readFileSync(htmlPath, 'utf-8');

            html = html.replace(
              /<script\s+type="importmap"[^>]*>[\s\S]*?<\/script>/i,
              `<script type="importmap">${importMap}</script>`,
            );

            let oldImportMap = importMap;
            try {
              oldImportMap = fs.readFileSync(
                path.resolve(import.meta.dirname, './dist/.importmap.json'),
                'utf-8',
              );
              console.log(highlightDiff(oldImportMap, importMap));
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

          delete bundle['.chunk-import-map.json'];
          this.emitFile({
            type: 'asset',
            fileName: '.importmap.json',
            source: importMap,
          });
        }
      },
    },
  ],
});
