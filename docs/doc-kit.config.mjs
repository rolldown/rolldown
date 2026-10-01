import { execFileSync } from 'node:child_process';
import { globSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import { brandAssets, createBundler } from '@voidzero-dev/doc-kit-theme/config';

import { generateHelpText } from '../packages/rolldown/src/cli/commands/help.ts';

const DOCS = import.meta.dirname;
const THEME = join(DOCS, 'theme');
const require = createRequire(import.meta.url);

const { version } = JSON.parse(
  readFileSync(join(DOCS, '../packages/rolldown/package.json'), 'utf8'),
);

const DESCRIPTION = 'Fast Rust-based bundler for JavaScript with Rollup-compatible API';

// The pages: every Markdown file, but for the repository notes and the
// directories holding no pages
const PAGES = '**/*.{md,mdx}';
const IGNORED = ['README.md', 'dist/**', 'node_modules/**', 'public/**', 'theme/**'];

const theme = (file) => join(THEME, file);
const voidzero = (file) => require.resolve(`@voidzero-dev/doc-kit-theme/components/${file}`);

// Components usable in MDX pages, and the islands the layouts hydrate
const components = {
  ApiIndex: theme('components/ApiIndex.jsx'),
  Home: theme('components/Home.jsx'),
  Outline: voidzero('Outline.jsx'),
  ReferenceIndex: voidzero('ReferenceIndex.jsx'),
  RiveAnimation: voidzero('RiveAnimation.jsx'),
  TeamMembers: voidzero('TeamMembers.jsx'),
  ThemeToggle: voidzero('ThemeToggle.jsx'),
};

// Named exports usable in MDX pages
const DATA_COMPONENTS = ['CliHelp', 'NodeVersion'];

/**
 * The import aliases of the components, and the components doc-kit makes
 * available to MDX pages.
 */
const componentImports = () => {
  const imports = {};
  const available = {};

  for (const [name, file] of Object.entries(components)) {
    imports[`#theme/${name}`] = file;
    available[name] = `#theme/${name}`;
  }

  imports['#theme/data-components'] = theme('components/data.jsx');

  for (const name of DATA_COMPONENTS) {
    available[name] = { name, source: '#theme/data-components', isDefaultExport: false };
  }

  return { imports, available };
};

const { imports, available } = componentImports();

/**
 * When each page last changed, from Git history, by page path
 * (`/guide/introduction`). Empty without Git history.
 */
const lastUpdated = () => {
  const pages = {};

  try {
    const log = execFileSync(
      'git',
      ['log', '--format=%x00%ct', '--name-only', '--relative', '--', '.'],
      {
        cwd: DOCS,
        encoding: 'utf8',
      },
    );

    // Newest first: `\0<time>\n\n<file>\n<file>…` per commit
    for (const commit of log.split('\0').slice(1)) {
      const [time, ...files] = commit.trim().split('\n');

      for (const file of files) {
        const page = /^(.+)\.mdx?$/.exec(file)?.[1];
        if (page) pages[`/${page}`] ??= Number(time) * 1000;
      }
    }
  } catch {}

  return pages;
};

// Build-time data for the `#theme/data` module
const data = {
  nodeVersion: readFileSync(join(DOCS, '../.node-version'), 'utf8').trim(),
  cliHelp: stripVTControlCharacters(generateHelpText()),
  lastUpdated: lastUpdated(),
};

/** @type {import('@doc-kit/core/utils/configuration/types').Configuration} */
export default {
  target: ['html', 'orama-db', 'llms-txt', 'sitemap'],

  global: {
    project: 'Rolldown',
    version,
    repository: 'rolldown/rolldown',
    ref: 'main',
    baseURL: 'https://rolldown.rs',
    input: [join(DOCS, PAGES)],
    ignore: IGNORED.map((pattern) => join(DOCS, pattern)),
    output: join(DOCS, 'dist'),
  },

  metadata: {
    // Links the API reference's type names, generated with the reference
    typeMap: join(DOCS, 'reference/type-map.json'),
  },

  // llms.txt links each page's Markdown, which the site serves too, and
  // llms-full.txt holds them all
  'llms-txt': {
    templatePath: theme('llms.txt'),
    writeMarkdown: true,
    writeFull: true,
  },

  sitemap: {
    pageURL: '{baseURL}{path}',
  },

  html: {
    title: '{project}',
    templatePath: theme('template.html'),
    stylesheets: [theme('styles.css')],
    generateAllPage: false,

    head: {
      meta: [
        { name: 'description', content: DESCRIPTION },
        { name: 'theme-color', content: '#ff7e17' },
        { property: 'og:description', content: DESCRIPTION },
        { property: 'og:image', content: 'https://rolldown.rs/og.jpg' },
        { property: 'og:site_name', content: 'Rolldown' },
        { name: 'twitter:card', content: 'summary_large_image' },
        { name: 'twitter:site', content: '@rolldown_rs' },
      ],
      links: [{ rel: 'icon', type: 'image/svg+xml', href: '/logo-without-border.svg' }],
      html: [],
    },

    imports: {
      '#theme/Layout': theme('Layout.jsx'),
      '#theme/reference-pages': join(DOCS, 'reference/pages.json'),
      ...imports,
    },

    virtualImports: {
      '#theme/data': Object.entries(data)
        .map(([name, value]) => `export const ${name} = ${JSON.stringify(value)};`)
        .join('\n'),
    },

    components: available,

    pathsToCopy: [{ [join(DOCS, 'public')]: '.' }, brandAssets('rolldown')],

    bundler: createBundler({
      pages: globSync(PAGES, { cwd: DOCS, exclude: IGNORED }).map((page) => join(DOCS, page)),
    }),
  },
};
