import { defineDevConfig } from '@rolldown/test-dev-server';
import fs from 'node:fs';
import path from 'node:path';

// Plugin that watches external dependencies during transform
function transformDependencyPlugin() {
  return {
    name: 'transform-dependency-plugin',
    transform(code, id) {
      if (id.endsWith('main.js')) {
        // During transform, we watch a config file that affects this module
        const configPath = path.join(path.dirname(id), 'config.json');
        this.addWatchFile(configPath);

        // Read the config and inject it into the module
        if (fs.existsSync(configPath)) {
          const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
          const injected = code.replace(
            '// INJECT_CONFIG_HERE',
            `const CONFIG = ${JSON.stringify(config)};`,
          );
          return { code: injected };
        }
      }
      return null;
    },
  };
}

export default defineDevConfig({
  dev: {
    platform: 'node',
  },
  build: {
    input: 'src/main.js',
    experimental: {
      hmr: {},
    },
    platform: 'node',
    treeshake: false,
    plugins: [transformDependencyPlugin()],
  },
});
