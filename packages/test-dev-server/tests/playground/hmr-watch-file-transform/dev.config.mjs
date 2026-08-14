import { defineDevConfig } from '@rolldown/test-dev-server';
import nodeFs from 'node:fs';
import nodePath from 'node:path';

// Ports the retired node fixture `transform-dependencies`: during `transform` the plugin
// registers `config.json` with `this.addWatchFile` and inlines its content. Editing that
// watched file — which is NOT a module in the graph — must rebuild and hot-update the
// module that watched it.
function transformDependencyPlugin() {
  return {
    name: 'transform-dependency-plugin',
    transform(code, id) {
      if (!id.endsWith('app.js')) {
        return null;
      }
      const configPath = nodePath.join(nodePath.dirname(id), 'config.json');
      this.addWatchFile(configPath);
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
    plugins: [transformDependencyPlugin()],
  },
});
