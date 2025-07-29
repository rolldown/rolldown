import { Plugin } from 'rolldown';
import { NormalizedDevOptions } from '../types/normalized-dev-options';

export function createDevServerPlugin(
  devOptions: NormalizedDevOptions,
): Plugin {
  return {
    name: 'rolldown-dev-server',
    generateBundle() {
      if (devOptions.platform === 'browser') {
        console.log('Generating index.html...');
        this.emitFile({
          type: 'asset',
          fileName: 'index.html',
          source: `<!doctype html>
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
  `,
        });
      }
    },
  };
}
