import { defineConfig } from 'vitepress';
import {
  groupIconMdPlugin,
  groupIconVitePlugin,
  localIconLoader,
} from 'vitepress-plugin-group-icons';
import llmstxt from 'vitepress-plugin-llms';

const sharedSidebar = [
  {
    text: 'User Guide',
    items: [
      { text: 'Introduction', link: '/guide/index.md' },
      { text: 'Getting Started', link: '/guide/getting-started.md' },
      { text: 'Notable Features', link: '/guide/features.md' },
      {
        text: 'Plugin Development',
        link: '/guide/plugin-development.md',
      },
    ],
  },
  {
    text: 'Reference',
    items: [
      {
        text: 'Config Options',
        link: '/reference/config-options.md',
      },
      { text: 'Bundler API', link: '/reference/bundler-api.md' },
      { text: 'Plugin API', link: '/reference/plugin-api.md' },
      { text: 'Command Line Interface', link: '/reference/cli.md' },
    ],
  },
  {
    text: 'In Depth',
    items: [
      { text: 'Why Bundlers', link: '/guide/in-depth/why-bundlers.md' },
      { text: 'Module Types', link: '/guide/in-depth/module-types.md' },
      { text: 'Top Level Await', link: '/guide/in-depth/tla-in-rolldown.md' },
      { text: 'Advanced Chunks', link: '/guide/in-depth/advanced-chunks.md' },
      // { text: 'Use Strict', link: '/guide/in-depth/use-strict.md' },
    ],
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
    ['link', {
      rel: 'icon',
      type: 'image/svg+xml',
      href: '/lightning-down.svg',
    }],
    ['meta', { name: 'theme-color', content: '#ff7e17' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:locale', content: 'en' }],
    ['meta', {
      property: 'og:title',
      content: 'Rolldown | Rust bundler for JavaScript',
    }],
    ['meta', {
      property: 'og:image',
      content: 'https://rolldown.rs/og-image.png',
    }],
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
      { text: 'User Guide', link: '/guide/' },
      { text: 'Reference', link: '/reference/config-options.md' },
      { text: 'Contribute', link: '/contrib-guide/' },
      {
        text: 'Resources',
        items: [
          {
            text: 'Team',
            link: '/team.md',
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
    ],

    sidebar: {
      '/guide/': sharedSidebar,
      '/reference/': sharedSidebar,
      '/contrib-guide/': [
        {
          text: 'Contribution Guide',
          items: [
            {
              text: 'Overview',
              link: '/contrib-guide/',
            },
            {
              text: 'Etiquette',
              link:
                'https://developer.mozilla.org/en-US/docs/MDN/Community/Open_source_etiquette',
            },
          ],
        },
        {
          text: 'Development',
          items: [
            {
              text: 'Setup the project',
              link: '/contrib-guide/setup-the-project.md',
            },
            {
              text: 'Building and running',
              link: '/contrib-guide/building-and-running.md',
            },
            { text: 'Testing', link: '/contrib-guide/testing.md' },
            { text: 'Benchmarking', link: '/contrib-guide/benchmarking.md' },
            {
              text: 'Tracing/Logging',
              link: '/contrib-guide/tracing-logging.md',
            },
            { text: 'Docs', link: '/contrib-guide/docs.md' },
          ],
        },
        {
          text: 'Coding Style',
          link: '/contrib-guide/coding-style.md',
        },
        {
          text: 'Misc',
          items: [{ text: 'Release', link: '/contrib-guide/release.md' }],
        },
      ],
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
        ignoreFiles: ['contrib-guide/**/*', 'index.md', 'README.md', 'team.md'],
        description:
          'Fast Rust-based bundler for JavaScript with Rollup-compatible API',
        details: '',
      }),
    ],
  },
  markdown: {
    config(md) {
      md.use(groupIconMdPlugin);
    },
  },
});
