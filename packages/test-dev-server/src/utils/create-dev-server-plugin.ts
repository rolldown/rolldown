import nodeFs from 'node:fs';
import nodePath from 'node:path';
import type { NormalizedInputOptions, Plugin } from 'rolldown';
import { injectOverlayScript } from '../error-overlay.js';
import type { Logger } from '../types/logger.js';
import type { NormalizedDevOptions } from '../types/normalized-dev-options';

export function createDevServerPlugin(
  devOptions: NormalizedDevOptions,
  logger: Logger = console,
): Plugin {
  let inputOptions: NormalizedInputOptions | null = null;
  return {
    name: 'rolldown-dev-server',
    buildStart(opts) {
      inputOptions = opts;
    },
    generateBundle() {
      if (devOptions.platform === 'browser') {
        logger.info('[createDevServerPlugin] Generating index.html for browser platform');
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
            logger.info(`[createDevServerPlugin] Using custom html file: ${customHtmlPath}`);
            htmlSource = nodeFs.readFileSync(customHtmlPath, 'utf-8');
          }
        }
        this.emitFile({
          type: 'asset',
          fileName: 'index.html',
          // Inject the dev-server error overlay client (kept out of the shared
          // rolldown HMR runtime — see error-overlay.ts).
          source: injectOverlayScript(htmlSource),
        });
      }
    },
  };
}
