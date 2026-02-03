import { extendConfig } from '@voidzero-dev/vitepress-theme/config';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { type DefaultTheme, defineConfig } from 'vitepress';
import { groupIconMdPlugin, groupIconVitePlugin } from 'vitepress-plugin-group-icons';
import llmstxt from 'vitepress-plugin-llms';
import { addOgImage } from 'vitepress-plugin-og';
import { hooksGraphPlugin } from './markdown-hooks-graph.ts';

const sidebarForUserGuide: DefaultTheme.SidebarItem[] = [
  {
    text: 'Guide',
    items: [
      { text: 'Introduction', link: '/guide/introduction.md' },
      { text: 'Getting Started', link: '/guide/getting-started.md' },
      { text: 'Notable Features', link: '/guide/notable-features.md' },
      {
        text: 'Troubleshooting',
        link: '/guide/troubleshooting.md',
      },
    ],
  },
  {
    text: 'APIs',
    items: [
      { text: 'Configuration Options', link: '/reference' },
      { text: 'Bundler API', link: '/apis/bundler-api.md' },
      {
        text: 'Plugin API',
        link: '/apis/plugin-api.md',
        items: [
          { text: 'Hook Filters', link: '/apis/plugin-api/hook-filters.md' },
          { text: 'File URLs', link: '/apis/plugin-api/file-urls.md' },
          { text: 'Source Code Transformations', link: '/apis/plugin-api/transformations.md' },
          {
            text: 'Inter-plugin communication',
            link: '/apis/plugin-api/inter-plugin-communication.md',
          },
        ],
      },
      { text: 'Command Line Interface', link: '/apis/cli.md' },
    ],
  },
  {
    text: 'Builtin Plugins',
    items: [
      {
        text: 'Introduction',
        link: '/builtin-plugins/',
      },
      {
        text: 'builtin:esm-external-require',
        link: '/builtin-plugins/esm-external-require.md',
      },
      {
        text: 'builtin:replace',
        link: '/builtin-plugins/replace.md',
      },
    ],
  },
];

const sidebarForInDepth: DefaultTheme.SidebarItem[] = [
  {
    text: 'In-Depth',
    items: [
      { text: 'Why Bundlers', link: '/in-depth/why-bundlers.md' },
      { text: 'Module Types', link: '/in-depth/module-types.md' },
      { text: 'Top Level Await', link: '/in-depth/tla-in-rolldown.md' },
      { text: 'Automatic Code Splitting', link: '/in-depth/automatic-code-splitting.md' },
      { text: 'Manual Code Splitting', link: '/in-depth/manual-code-splitting.md' },
      { text: 'Bundling CJS', link: '/in-depth/bundling-cjs.md' },
      {
        text: 'Non ESM Output Formats',
        link: '/in-depth/non-esm-output-formats.md',
      },
      { text: 'Dead Code Elimination', link: '/in-depth/dead-code-elimination.md' },
      { text: 'Lazy Barrel Optimization', link: '/in-depth/lazy-barrel-optimization.md' },
      { text: 'Native MagicString', link: '/in-depth/native-magic-string.md' },
      {
        text: 'Why Plugin Hook Filter',
        link: '/in-depth/why-plugin-hook-filter.md',
      },
      { text: 'Directives', link: '/in-depth/directives.md' },
    ],
  },
];

const importantAPIs: (string | undefined)[] = [
  '/Function.build.md',
  '/Function.rolldown.md',
  '/Function.watch.md',
  '/Interface.Plugin.md',
  '/Interface.PluginContext.md',
  '/Variable.VERSION.md',
  '/Function.defineConfig.md',
];

function getTypedocSidebar() {
  const filepath = path.resolve(import.meta.dirname, '../reference/typedoc-sidebar.json');
  if (!existsSync(filepath)) return [];

  try {
    return JSON.parse(readFileSync(filepath, 'utf-8')) as DefaultTheme.SidebarItem[];
  } catch (error) {
    console.error('Failed to load typedoc sidebar:', error);
    return [];
  }
}

const typedocSidebar = getTypedocSidebar().map((item) => {
  const stringifyForSort = (item: DefaultTheme.SidebarItem) =>
    (importantAPIs.includes(item.link) ? '0' : '1') + (item.text ?? '');
  return {
    ...item,
    base: '/reference',
    items: item.items
      ?.map((item) => ({
        ...item,
        text: (importantAPIs.includes(item.link) ? '★ ' : '') + item.text,
      }))
      .toSorted((a, b) => stringifyForSort(a).localeCompare(stringifyForSort(b))),
  };
});

function getOptionsSidebar() {
  const filepath = path.resolve(import.meta.dirname, '../reference/options-sidebar.json');
  if (!existsSync(filepath)) return [];

  try {
    return JSON.parse(readFileSync(filepath, 'utf-8')) as DefaultTheme.SidebarItem[];
  } catch (error) {
    console.error('Failed to load options sidebar:', error);
    return [];
  }
}

const sidebarForReference: DefaultTheme.SidebarItem[] = [
  {
    text: 'Options',
    base: '/reference',
    items: getOptionsSidebar(),
    collapsed: false,
  },
  ...typedocSidebar,
];

const sidebarForDevGuide: DefaultTheme.SidebarItem[] = [
  {
    text: 'Contribution Guide',
    items: [
      {
        text: 'Overview',
        link: '/contribution-guide/',
      },
      {
        text: 'Etiquette',
        link: 'https://developer.mozilla.org/en-US/docs/MDN/Community/Open_source_etiquette',
      },
    ],
  },
  {
    text: 'Development Guide',
    items: [
      {
        text: 'Setup the project',
        link: '/development-guide/setup-the-project.md',
      },
      {
        text: 'Building and running',
        link: '/development-guide/building-and-running.md',
      },
      { text: 'Testing', link: '/development-guide/testing.md' },
      {
        text: 'Benchmarking',
        link: '/development-guide/benchmarking.md',
      },
      {
        text: 'Tracing/Logging',
        link: '/development-guide/tracing-logging.md',
      },
      {
        text: 'Profiling',
        link: '/development-guide/profiling.md',
      },
      { text: 'Docs', link: '/development-guide/docs.md' },
      {
        text: 'Coding Style',
        link: '/development-guide/coding-style.md',
      },
    ],
  },
];

const sidebarForGlossary: DefaultTheme.SidebarItem[] = [
  {
    text: 'Glossary',
    items: [
      { text: 'Barrel Module', link: '/glossary/barrel-module.md' },
      { text: 'Entry', link: '/glossary/entry.md' },
      { text: 'Entry Chunk', link: '/glossary/entry-chunk.md' },
      { text: 'Entry Name', link: '/glossary/entry-name.md' },
      { text: 'User-defined Entry', link: '/glossary/user-defined-entry.md' },
    ],
  },
];

const sidebarForResources: DefaultTheme.SidebarItem[] = [
  {
    text: 'Team',
    link: '/team.md',
  },
  {
    text: 'Acknowledgements',
    link: '/acknowledgements.md',
  },
];

// https://vitepress.dev/reference/site-config
const config = defineConfig({
  title: 'Rolldown',
  description: 'Fast Rust-based bundler for JavaScript with Rollup-compatible API',
  lastUpdated: true,
  cleanUrls: true,
  sitemap: {
    hostname: 'https://rolldown.rs',
  },
  head: [
    [
      'link',
      {
        rel: 'icon',
        type: 'image/svg+xml',
        href: '/logo-without-border.svg',
      },
    ],
    ['meta', { name: 'theme-color', content: '#ff7e17' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:locale', content: 'en' }],
    [
      'meta',
      {
        property: 'og:title',
        content: 'Rolldown | Rust bundler for JavaScript',
      },
    ],
    [
      'meta',
      {
        property: 'og:image',
        content: 'https://rolldown.rs/og.jpg',
      },
    ],
    ['meta', { property: 'og:site_name', content: 'Rolldown' }],
    ['meta', { property: 'og:url', content: 'https://rolldown.rs/' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:site', content: '@rolldown_rs' }],
    [
      'script',
      {
        src: 'https://cdn.usefathom.com/script.js',
        'data-site': 'RBMPDFTV',
        'data-spa': 'auto',
        defer: '',
      },
    ],
  ],

  themeConfig: {
    variant: 'rolldown',
    search: {
      provider: 'algolia',
      options: {
        appId: process.env.ALGOLIA_APP_ID || '',
        apiKey: process.env.ALGOLIA_API_KEY || '',
        indexName: 'rolldown',
      },
    },

    // https://vitepress.dev/reference/default-theme-config
    nav: [
      {
        text: 'Docs',
        activeMatch: '/(guide|in-depth|glossary|apis|builtin-plugins)',
        items: [
          {
            text: 'Guide',
            activeMatch: '/(guide|apis|builtin-plugins)',
            link: '/guide/getting-started.md',
          },
          {
            text: 'In-Depth',
            activeMatch: '/in-depth',
            link: '/in-depth/why-bundlers.md',
          },
          {
            text: 'Glossary',
            activeMatch: '/glossary',
            link: '/glossary/',
          },
        ],
      },
      { text: 'Options & APIs', activeMatch: '/reference', link: '/reference' },
      { text: 'REPL', link: 'https://repl.rolldown.rs/' },
      {
        text: 'Resources',
        activeMatch: '/(team|acknowledgements|contribution-guide|development-guide)',
        items: [
          {
            text: 'Team',
            activeMatch: '/(team|acknowledgements)',
            link: '/team.md',
          },
          {
            text: 'Contribute',
            activeMatch: '/(contribution-guide|development-guide)',

            link: '/contribution-guide/',
          },

          {
            text: 'Roadmap',
            link: 'https://github.com/rolldown/rolldown/discussions/153',
          },
        ],
      },
    ],

    sidebar: {
      // --- Guide ---
      '/guide/': sidebarForUserGuide,
      '/apis/': sidebarForUserGuide,
      '/builtin-plugins/': sidebarForUserGuide,
      // --- In-Depth ---
      '/in-depth/': sidebarForInDepth,
      // --- Reference ---
      '/reference/': sidebarForReference,
      // --- Glossary ---
      '/glossary/': sidebarForGlossary,
      // --- Contribute ---
      '/contribution-guide/': sidebarForDevGuide,
      '/development-guide/': sidebarForDevGuide,
      // --- Resources ---
      '/team': sidebarForResources,
      '/acknowledgements': sidebarForResources,
    },
    outline: 'deep',
    socialLinks: [
      { icon: 'x', link: 'https://twitter.com/rolldown_rs' },
      {
        icon: 'bluesky',
        link: 'https://bsky.app/profile/rolldown.rs',
      },
      { icon: 'discord', link: 'https://chat.rolldown.rs' },
      { icon: 'github', link: 'https://github.com/rolldown/rolldown' },
    ],

    footer: {
      copyright: `© 2025 VoidZero Inc. and Rolldown contributors.`,
      nav: [
        {
          title: 'Rolldown',
          items: [
            { text: 'Guide', link: '/guide/getting-started' },
            { text: 'Options & APIs', link: '/reference' },
            { text: 'Plugins', link: '/builtin-plugins/' },
            { text: 'Contribute', link: '/contribution-guide/' },
            { text: 'REPL', link: 'https://repl.rolldown.rs/' },
          ],
        },
        {
          title: 'Resources',
          items: [
            {
              text: 'Roadmap',
              link: 'https://github.com/rolldown/rolldown/discussions/153',
            },
            { text: 'Team', link: '/team' },
          ],
        },
      ],
      social: [
        { icon: 'github', link: 'https://github.com/rolldown/rolldown' },
        { icon: 'discord', link: 'https://chat.rolldown.rs' },
        { icon: 'bluesky', link: 'https://bsky.app/profile/rolldown.rs' },
        { icon: 'x', link: 'https://x.com/rolldown_rs' },
      ],
    },

    editLink: {
      pattern: 'https://github.com/rolldown/rolldown/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },
  },

  vite: {
    optimizeDeps: {
      exclude: ['@docsearch/css'],
    },
    plugins: [
      groupIconVitePlugin({
        customIcon: {
          homebrew: 'logos:homebrew',
          cargo: 'vscode-icons:file-type-cargo',
        },
      }) as any,
      llmstxt({
        ignoreFiles: ['development-guide/**/*', 'index.md', 'README.md', 'team.md'],
        description: 'Fast Rust-based bundler for JavaScript with Rollup-compatible API',
        details: '',
      }),
    ],
  },
  markdown: {
    async config(md) {
      md.use(groupIconMdPlugin);
      await hooksGraphPlugin(md);
    },
  },
  async transformPageData(pageData, ctx) {
    // Disable "Edit this page on GitHub" for auto-generated reference docs
    if (pageData.relativePath.startsWith('reference/')) {
      pageData.frontmatter.editLink = false;
    }

    // Automatically handle OG images for all markdown files.
    if (!pageData.frontmatter.image && pageData.relativePath !== 'index.md') {
      await addOgImage(pageData, ctx, {
        domain: 'https://rolldown.rs',
        maxTitleSizePerLine: 16,
      });
    }
  },
});

export default extendConfig(config);
