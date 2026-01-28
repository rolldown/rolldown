import { defineDevConfig } from '@rolldown/test-dev-server';
import fs from 'node:fs';
import path from 'node:path';

// Plugin that watches external dependencies during load
function loadDependencyPlugin() {
  return {
    name: 'load-dependency-plugin',
    load(id) {
      if (id.endsWith('main.js')) {
        const content = fs.readFileSync(id, 'utf8');
        this.addWatchFile(id);

        // During load, we watch a config file that affects this module
        const configPath = path.join(path.dirname(id), 'config.json');
        this.addWatchFile(configPath);

        // Read the config and inject it into the module
        if (fs.existsSync(configPath)) {
          const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
          const injected = content.replace(
            '// INJECT_CONFIG_HERE',
            `const CONFIG = ${JSON.stringify(config)};`,
          );
          return { code: injected };
        }
        return { code: content };
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
      devMode: {},
    },
    platform: 'node',
    treeshake: false,
    plugins: [loadDependencyPlugin()],
  },
});
