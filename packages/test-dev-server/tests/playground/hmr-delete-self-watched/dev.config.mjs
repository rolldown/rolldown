import { defineDevConfig } from '@rolldown/test-dev-server';

// A transform plugin registers child.js's OWN file with `addWatchFile` — a
// defensive pattern some real plugins use. A delete event for child.js then
// reaches the engine through two routes: "the file is a module" and "the file
// is a transform dependency". The spec pins that this doubled registration
// adds no failure mode of its own: deleting the still-imported file surfaces
// the normal unresolved-import error, and recreating it recovers.
//
// The plugin enters the bundled environment through `applyToEnvironment` so
// `addWatchFile` runs on rolldown's own transform context — the same proven
// path as the `hmr-hot-update-hook` playground.
const selfWatchPlugin = {
  name: 'test-self-watch',
  applyToEnvironment() {
    return {
      name: 'test-self-watch:rolldown',
      transform: {
        filter: { id: /child\.js$/ },
        handler(_code, id) {
          this.addWatchFile(id);
          return null;
        },
      },
    };
  },
};

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
    plugins: [selfWatchPlugin],
  },
});
