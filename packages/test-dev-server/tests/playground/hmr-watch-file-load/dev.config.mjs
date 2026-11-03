import { defineDevConfig } from '@rolldown/test-dev-server';
import nodeFs from 'node:fs';
import nodePath from 'node:path';

// Ports the retired node fixture `load-dependencies`: same as
// `hmr-watch-file-transform`, but `this.addWatchFile` is called from the `load` hook —
// a different plugin context and invalidation path than `transform`.
function loadDependencyPlugin() {
  return {
    name: 'load-dependency-plugin',
    load(id) {
      if (!id.endsWith('app.js')) {
        return null;
      }
      const configPath = nodePath.join(nodePath.dirname(id), 'config.json');
      this.addWatchFile(configPath);
      const code = nodeFs.readFileSync(id, 'utf-8');
      const config = JSON.parse(nodeFs.readFileSync(configPath, 'utf-8'));
      return {
        code: code.replace('// INJECT_CONFIG_HERE', `const CONFIG = ${JSON.stringify(config)};`),
      };
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
    plugins: [loadDependencyPlugin()],
  },
});
