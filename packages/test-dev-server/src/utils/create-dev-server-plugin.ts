import nodeFs from 'node:fs';
import nodePath from 'node:path';
import type { NormalizedInputOptions, Plugin } from 'rolldown';
import type { NormalizedDevOptions } from '../types/normalized-dev-options';

export function createDevServerPlugin(devOptions: NormalizedDevOptions): Plugin {
  let inputOptions: NormalizedInputOptions | null = null;
  return {
    name: 'rolldown-dev-server',
    buildStart(opts) {
      inputOptions = opts;
    },
    generateBundle() {
      if (devOptions.platform === 'browser') {
        console.log('[createDevServerPlugin] Generating index.html for browser platform');
        let htmlSource = `<!doctype html>
  <html lang="en">
    <head>
      <meta charset="UTF-8" />
      <link rel="icon" type="image/svg+xml" href="/vite.svg" />
      <meta name="viewport" content="width=device-width, initial-scale=1.0" />
      <title>Test</title>
    </head>
    <body>
      <div id="root"></div>
      <script type="module" src="/main.js"></script>
    </body>
  </html>
  `;
        if (inputOptions) {
          const customHtmlPath = nodePath.resolve(inputOptions.cwd, 'index.html');
          if (nodeFs.existsSync(customHtmlPath)) {
            console.log(`[createDevServerPlugin] Using custom html file: ${customHtmlPath}`);
            htmlSource = nodeFs.readFileSync(customHtmlPath, 'utf-8');
          }
        }
        this.emitFile({
          type: 'asset',
          fileName: 'index.html',
          source: htmlSource,
        });
      }
    },
  };
}
