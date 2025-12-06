import type { Plugin } from 'rolldown';
import type { NormalizedDevOptions } from '../types/normalized-dev-options';

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
          source: `
<h1>HMR Full Bundle Mode</h1>

<div class="app"></div>
<div class="hmr"></div>

<script type="module" src="./main.js"></script>
  `,
        });
      }
    },
  };
}
