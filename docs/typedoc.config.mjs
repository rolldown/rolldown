// The API reference: TypeDoc reads the `rolldown` package, and doc-kit's
// TypeDoc plugin writes it as doc-kit Markdown into `reference/`.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { OptionDefaults, ReflectionKind } from 'typedoc';

const ROOT = join(import.meta.dirname, '..');
const PACKAGE = join(ROOT, 'packages/rolldown');

// Entry points of the package left out of the reference
const EXCLUDED = new Set(['./experimental', './parallelPlugin']);

const { exports } = JSON.parse(readFileSync(join(PACKAGE, 'package.json'), 'utf8'));

// The source of each documented entry point, named by its `@module` tag
const entryPoints = Object.entries(exports).flatMap(([path, { dev }]) =>
  dev && !EXCLUDED.has(path) ? [join(PACKAGE, dev)] : [],
);

/**
 * The URL of a page, as rolldown.rs has always had it: `Interface.Plugin`, and
 * `InputOptions.input` for an option. Entry points keep doc-kit's.
 *
 * @param {string} url
 * @param {import('typedoc').Reflection} reflection
 */
const pageUrl = (url, reflection) => {
  if (reflection.kindOf(ReflectionKind.SomeModule)) {
    return url;
  }

  if (reflection.kindOf(ReflectionKind.SomeMember)) {
    return `${reflection.parent.name}.${reflection.name}`;
  }

  // The kind's name in TypeDoc's enum: `TypeAlias.Format`
  return `${ReflectionKind[reflection.kind]}.${reflection.name}`;
};

/** @type {Partial<import('typedoc').TypeDocOptions> & Record<string, unknown>} */
export default {
  plugin: [
    // Resolved from here: TypeDoc resolves plugin names from its own location
    fileURLToPath(import.meta.resolve('@doc-kit/typedoc')),
    join(import.meta.dirname, 'typedoc-hooks.mjs'),
  ],
  entryPoints,
  tsconfig: join(PACKAGE, 'tsconfig.json'),
  // Source links are relative to the repository
  basePath: ROOT,
  docKit: join(import.meta.dirname, 'reference'),
  readme: 'none',
  excludeInternal: true,
  excludeExternals: true,
  externalPattern: ['**/packages/pluginutils/**', '**/node_modules/**/@oxc-project/types/**'],
  // `@kind` gives a plugin hook's kind: `@kind async, parallel`
  blockTags: [...OptionDefaults.blockTags, '@kind'],
  logLevel: 'Error',

  // Each option has a page of its own
  docKitMemberPages: ['InputOptions', 'OutputOptions'],
  docKitUrlAdapter: pageUrl,
  // `this` in plugin hooks, and plugins themselves
  docKitReceivers: {
    MinimalPluginContext: 'this',
    PluginContext: 'this',
    TransformPluginContext: 'this',
    Plugin: 'plugin',
  },
};
