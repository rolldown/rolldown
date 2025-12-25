import { rm } from 'node:fs/promises';
import path from 'node:path';
import { Application, type TypeDocOptions } from 'typedoc';
import type { PluginOptions } from 'typedoc-plugin-markdown';
console.log('📚 Generating reference...');

// Generate API documentation
await runTypedoc();
console.log('✅ Reference generated successfully!');
console.log('📚 Beautifying reference structure...');

await rm('reference/api/index.md', { force: true });

/**
 * Run TypeDoc with the specified tsconfig
 */
async function runTypedoc(): Promise<void> {
  const root = path.resolve(
    import.meta.dirname,
    '../../..',
  );

  const options: TypeDocOptions & PluginOptions = {
    tsconfig: path.join(root, 'packages/rolldown/tsconfig.json').split(
      path.sep,
    ).join(path.posix.sep),
    plugin: [
      'typedoc-plugin-markdown',
      'typedoc-vitepress-theme',
      path.join(import.meta.dirname, 'extract-options-plugin.ts').split(
        path.sep,
      ).join(
        path.posix.sep,
      ),
    ],
    out: './reference',
    entryPoints: [
      path.join(root, 'packages/rolldown/src/index.ts').split(path.sep).join(
        path.posix.sep,
      ),
    ],
    excludeInternal: true,

    hideBreadcrumbs: true,
    useCodeBlocks: true,
    flattenOutputFiles: true,

    categoryOrder: ['Programmatic APIs', 'Plugin APIs', '*'],

    // @ts-expect-error VitePress config
    docsRoot: './reference',
    sidebar: {
      pretty: true,
    },
  };
  const app = await Application.bootstrapWithPlugins(options);

  // May be undefined if errors are encountered.
  const project = await app.convert();

  if (project) {
    // Generate configured outputs
    await app.generateOutputs(project);
  } else {
    throw new Error('Failed to generate TypeDoc output');
  }
}
