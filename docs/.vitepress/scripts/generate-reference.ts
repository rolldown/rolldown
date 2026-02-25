import { copyFile, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { Application, type TypeDocOptions } from 'typedoc';
import type { PluginOptions } from 'typedoc-plugin-markdown';

const root = path.resolve(import.meta.dirname, '../../..');

console.log('📚 Generating reference...');

const exportPaths = await discoverExports();
const allEntryPoints = exportPaths.map((p) => p.replaceAll('\\', '/'));

// Generate API documentation
await runTypedoc(allEntryPoints);
console.log('✅ Reference generated successfully!');

await relativizeInternalLinks('reference');

await rm('reference/index.md', { force: true });
await copyFile('.vitepress/theme/components/api.index.md', 'reference/index.md');
console.log('📚 New index added successfully');

async function discoverExports(): Promise<string[]> {
  const excludedExports = new Set(['./experimental', './parallelPlugin']);

  const pkgJsonPath = path.join(root, 'packages/rolldown/package.json');
  const pkgJson: { exports: Record<string, { dev?: string }> } = JSON.parse(
    await readFile(pkgJsonPath, 'utf-8'),
  );
  return Object.entries(pkgJson.exports).flatMap(([key, entry]) => {
    if (excludedExports.has(key) || !entry.dev) return [];
    return path.join(root, 'packages/rolldown', entry.dev);
  });
}

/**
 * Convert absolute rolldown.rs URLs in markdown links to relative paths,
 * so VitePress treats them as internal navigation instead of opening
 * external links with target="_blank".
 */
async function relativizeInternalLinks(dir: string): Promise<void> {
  for (const file of await readdir(dir)) {
    if (!file.endsWith('.md')) continue;
    const filePath = path.join(dir, file);
    const content = await readFile(filePath, 'utf-8');
    const updated = content.replace(/\]\(https:\/\/rolldown\.rs\//g, '](/');
    if (updated !== content) {
      await writeFile(filePath, updated);
    }
  }
}

type TypedocVitepressThemeOptions = {
  docsRoot?: string;
  sidebar?: any;
};

/**
 * Run TypeDoc with the specified tsconfig
 */
async function runTypedoc(entryPoints: string[]): Promise<void> {
  const options: TypeDocOptions & PluginOptions & TypedocVitepressThemeOptions = {
    tsconfig: path.join(root, 'packages/rolldown/tsconfig.json'),
    plugin: [
      'typedoc-plugin-markdown',
      'typedoc-vitepress-theme',
      path.join(import.meta.dirname, 'custom-theme-plugin.ts'),
      'typedoc-plugin-merge-modules',
      path.join(import.meta.dirname, 'extract-options-plugin.ts'),
    ],
    theme: 'customTheme',
    out: './reference',
    entryPoints,
    readme: 'none',
    excludeInternal: true,
    excludeExternals: true,
    externalPattern: ['**/packages/pluginutils/**', '**/node_modules/**/@oxc-project/types/**'],

    hideBreadcrumbs: true,
    flattenOutputFiles: true,
    expandObjects: true,

    categoryOrder: [
      'Programmatic APIs',
      'Plugin APIs',
      'Config',
      'Builtin Plugins',
      'Utilities',
      '*',
    ],

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
