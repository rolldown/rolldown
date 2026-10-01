// The API reference: TypeDoc reads the `rolldown` package, and doc-kit's
// TypeDoc plugin writes it as doc-kit Markdown into `reference/`.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { OptionDefaults } from 'typedoc';

const PACKAGE = join(import.meta.dirname, '../packages/rolldown');

// Entry points of the package left out of the reference
const EXCLUDED = new Set(['./experimental', './parallelPlugin']);

const { exports } = JSON.parse(readFileSync(join(PACKAGE, 'package.json'), 'utf8'));

// Each documented entry point's source, with the path it is imported from
const entryPoints = Object.fromEntries(
  Object.entries(exports).flatMap(([path, { dev }]) =>
    dev && !EXCLUDED.has(path)
      ? [[join(PACKAGE, dev), path === '.' ? 'rolldown' : `rolldown/${path.slice(2)}`]]
      : [],
  ),
);

/** @type {Partial<import('typedoc').TypeDocOptions> & Record<string, unknown>} */
export default {
  // Resolved from here: TypeDoc resolves plugin names from its own location
  plugin: [fileURLToPath(import.meta.resolve('@doc-kit/typedoc'))],
  entryPoints: Object.keys(entryPoints),
  tsconfig: join(PACKAGE, 'tsconfig.json'),
  outputs: [{ name: 'doc-kit', path: join(import.meta.dirname, 'reference') }],
  readme: 'none',
  excludeInternal: true,
  excludeExternals: true,
  externalPattern: ['**/packages/pluginutils/**', '**/node_modules/**/@oxc-project/types/**'],
  // `@kind` gives a plugin hook's kind: `@kind async, parallel`
  blockTags: [...OptionDefaults.blockTags, '@kind'],
  logLevel: 'Error',

  docKitSiteUrl: 'https://rolldown.rs',
  docKitImportPaths: entryPoints,
  // Each option has a page of its own
  docKitMemberPages: ['InputOptions', 'OutputOptions'],
  // `this` in plugin hooks, and plugins themselves
  docKitReceivers: {
    MinimalPluginContext: 'this',
    PluginContext: 'this',
    TransformPluginContext: 'this',
    Plugin: 'plugin',
  },
  // Plugin hooks are typed, with their documentation, in `FunctionPluginHooks`
  docKitSignatureSources: { Plugin: 'FunctionPluginHooks' },
  docKitEvents: { RolldownWatcher: 'RolldownWatcherWatcherEventMap' },
  // Keep the member anchors links into rolldown.rs use (`#input`)
  docKitMemberAnchors: true,
};
