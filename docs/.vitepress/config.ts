import { fileURLToPath } from 'node:url';
import { defineConfig, UserConfig } from 'vitepress';
import {
  groupIconMdPlugin,
  groupIconVitePlugin,
  localIconLoader,
} from 'vitepress-plugin-group-icons';
import llmstxt from 'vitepress-plugin-llms';

const CONFIG_LINK = '/options/input.md';

const sidebarForUserGuide: UserConfig['themeConfig']['sidebar'] = [
  {
    text: 'Guide',
    link: '/guide/getting-started.md',
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
      { text: 'Configuration Options', link: CONFIG_LINK },
      { text: 'Bundler API', link: '/apis/bundler-api.md' },
      { text: 'Plugin API', link: '/apis/plugin-api.md' },
      { text: 'Plugin Hook Filters', link: '/apis/plugin-hook-filters.md' },
      { text: 'Command Line Interface', link: '/apis/cli.md' },
    ],
  },
  {
    text: 'In Depth',
    items: [
      { text: 'Why Bundlers', link: '/in-depth/why-bundlers.md' },
      { text: 'Module Types', link: '/in-depth/module-types.md' },
      { text: 'Top Level Await', link: '/in-depth/tla-in-rolldown.md' },
      { text: 'Advanced Chunks', link: '/in-depth/advanced-chunks.md' },
      { text: 'Bundling CJS', link: '/in-depth/bundling-cjs.md' },
      { text: 'Native MagicString', link: '/in-depth/native-magic-string.md' },
      {
        text: 'Why Plugin Hook Filter',
        link: '/in-depth/why-plugin-hook-filter.md',
      },
      // { text: 'Code Splitting', link: '/in-depth/code-splitting.md' },
      { text: 'Directives', link: '/in-depth/directives.md' },
    ],
  },
];

const sidebarForOptions: UserConfig['themeConfig']['sidebar'] = [
  {
    text: 'Rolldown Options',
    items: [
      { text: 'input', link: '/options/input.md' },
      { text: 'external', link: '/options/external.md' },
      { text: 'resolve', link: '/options/resolve.md' },
      { text: 'cwd', link: '/options/cwd.md' },
      { text: 'platform', link: '/options/platform.md' },
      { text: 'shimMissingExports', link: '/options/shim-missing-exports.md' },
      { text: 'treeshake', link: '/options/treeshake.md' },
      { text: 'logLevel', link: '/options/log-level.md' },
      { text: 'onLog', link: '/options/on-log.md' },
      { text: 'onwarn', link: '/options/onwarn.md' },
      { text: 'moduleTypes', link: '/options/module-types.md' },
      {
        text: 'preserveEntrySignatures',
        link: '/options/preserve-entry-signatures.md',
      },
      { text: 'optimization', link: '/options/optimization.md' },
      { text: 'context', link: '/options/context.md' },
      { text: 'tsconfig', link: '/options/tsconfig.md' },
      { text: 'experimental', link: '/options/experimental.md' },
      { text: 'output', link: '/options/output.md' },
      {
        text: 'output.sourcemap',
        link: '/options/output-sourcemap.md',
      },
      {
        text: 'output.generatedCode',
        link: '/options/output-generated-code.md',
      },
      {
        text: 'output.advancedChunks',
        link: '/options/output-advanced-chunks.md',
      },
      {
        text: 'output.cleanDir',
        link: '/options/output-clean-dir.md',
      },
    ],
  },
];

const sidebarForDevGuide: UserConfig['themeConfig']['sidebar'] = [
  {
    text: 'Contribution Guide',
    items: [
      {
        text: 'Overview',
        link: '/contribution-guide/',
      },
      {
        text: 'Etiquette',
        link:
          'https://developer.mozilla.org/en-US/docs/MDN/Community/Open_source_etiquette',
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

const sidebarForPluginGuide: UserConfig['themeConfig']['sidebar'] = [
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

const sidebarForGlossary: UserConfig['themeConfig']['sidebar'] = [
  {
    text: 'Glossary',
    link: '/glossary/',
    items: [
      { text: 'Entry', link: '/glossary/entry.md' },
      { text: 'Entry Chunk', link: '/glossary/entry-chunk.md' },
      { text: 'Entry Name', link: '/glossary/entry-name.md' },
      { text: 'User-defined Entry', link: '/glossary/user-defined-entry.md' },
    ],
  },
];

const sidebarForResources: UserConfig['themeConfig']['sidebar'] = [
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
export default defineConfig({
  title: 'Rolldown',
  description:
    'Fast Rust-based bundler for JavaScript with Rollup-compatible API',
  lastUpdated: true,
  cleanUrls: true,
  head: [
    [
      'link',
      {
        rel: 'icon',
        type: 'image/svg+xml',
        href: '/lightning-down.svg',
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
        content: 'https://rolldown.rs/og-image.png',
      },
    ],
    ['meta', { property: 'og:site_name', content: 'Rolldown' }],
    ['meta', { property: 'og:url', content: 'https://rolldown.rs/' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:site', content: '@rolldown_rs' }],
  ],

  themeConfig: {
    search: {
      provider: 'algolia',
      options: {
        appId: process.env.ALGOLIA_APP_ID || '',
        apiKey: process.env.ALGOLIA_API_KEY || '',
        indexName: 'rolldown',
      },
    },
    logo: { src: '/lightning-down.svg', width: 24, height: 24 },

    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: 'Guide', link: '/guide/getting-started.md' },
      { text: 'Config', link: CONFIG_LINK },
      { text: 'Plugins', link: '/builtin-plugins/' },
      { text: 'Contribute', link: '/contribution-guide/' },
      {
        text: 'Resources',
        items: [
          {
            text: 'Team',
            link: '/team.md',
          },
          {
            text: 'Glossary',
            link: '/glossary/',
          },
          {
            text: 'Roadmap',
            link: 'https://github.com/rolldown/rolldown/discussions/153',
          },
          {
            items: [
              {
                text: 'Twitter',
                link: 'https://twitter.com/rolldown_rs',
              },
              {
                text: 'Bluesky',
                link: 'https://bsky.app/profile/rolldown.rs',
              },
              {
                text: 'Discord Chat',
                link: 'https://chat.rolldown.rs',
              },
            ],
          },
        ],
      },
      { text: 'REPL', link: 'https://repl.rolldown.rs/' },
    ],

    sidebar: {
      // --- Guide ---
      '/guide/': sidebarForUserGuide,
      '/apis/': sidebarForUserGuide,
      '/in-depth/': sidebarForUserGuide,
      // --- Options ---
      '/options/': sidebarForOptions,
      // --- Plugin ---
      '/builtin-plugins/': sidebarForPluginGuide,
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
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2023-present VoidZero Inc.',
    },

    editLink: {
      pattern: 'https://github.com/rolldown/rolldown/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },
  },

  vite: {
    plugins: [
      groupIconVitePlugin({
        customIcon: {
          homebrew: 'logos:homebrew',
          cargo: 'vscode-icons:file-type-cargo',
          rolldown: localIconLoader(
            import.meta.url,
            '../public/lightning-down.svg',
          ),
        },
      }) as any,
      llmstxt({
        ignoreFiles: [
          'development-guide/**/*',
          'index.md',
          'README.md',
          'team.md',
        ],
        description:
          'Fast Rust-based bundler for JavaScript with Rollup-compatible API',
        details: '',
      }),
    ],
    resolve: {
      alias: [
        {
          find: /^.*\/VPHero\.vue$/,
          replacement: fileURLToPath(
            new URL('./theme/components/overrides/VPHero.vue', import.meta.url),
          ),
        },
      ],
    },
  },
  markdown: {
    config(md) {
      md.use(groupIconMdPlugin);
    },
  },
});
