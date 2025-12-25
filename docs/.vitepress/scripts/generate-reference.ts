import { copyFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { Application, type TypeDocOptions } from 'typedoc';
import type { PluginOptions } from 'typedoc-plugin-markdown';
console.log('📚 Generating reference...');

// Generate API documentation
await runTypedoc();
console.log('✅ Reference generated successfully!');

const referenceIndexPath = path.join(import.meta.dirname, '..', 'reference', 'index.md');
const apiIndexSourcePath = path.join(
  import.meta.dirname,
  '..',
  'theme',
  'components',
  'api.index.md',
);

await rm(referenceIndexPath, { force: true });
await copyFile(
  apiIndexSourcePath,
  referenceIndexPath,
);
console.log('📚 New index added successfully');

type TypedocVitepressThemeOptions = {
  docsRoot?: string;
  sidebar?: any;
};

/**
 * Run TypeDoc with the specified tsconfig
 */
async function runTypedoc(): Promise<void> {
  const root = path.resolve(
    import.meta.dirname,
    '../../..',
  );

  const options: TypeDocOptions & PluginOptions & TypedocVitepressThemeOptions =
    {
      tsconfig: path.join(root, 'packages/rolldown/tsconfig.json'),
      plugin: [
        'typedoc-plugin-markdown',
        'typedoc-vitepress-theme',
        path.join(import.meta.dirname, 'extract-options-plugin.ts'),
        path.join(import.meta.dirname, 'custom-theme-plugin.ts'),
      ],
      theme: 'customTheme',
      out: './reference',
      entryPoints: [
        path.join(root, 'packages/rolldown/src/index.ts').replaceAll('\\', '/'),
      ],
      readme: 'none',
      excludeInternal: true,

      hideBreadcrumbs: true,
      flattenOutputFiles: true,

      categoryOrder: ['Programmatic APIs', 'Plugin APIs', '*'],

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
