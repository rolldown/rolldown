import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: 'Rolldown',
  description:
    'Fast Rust-based bundler for JavaScript with Rollup-compatible API',

  lastUpdated: true,
  cleanUrls: true,

  /* prettier-ignore */
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/lightning-down.svg' }],
    ['meta', { name: 'theme-color', content: '#ff7e17' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:locale', content: 'en' }],
    ['meta', { property: 'og:title', content: 'Rolldown | Rust bundler for JavaScript' }],
    ['meta', { property: 'og:image', content: 'https://rolldown.rs/og-image.png' }],
    ['meta', { property: 'og:site_name', content: 'Rolldown' }],
    ['meta', { property: 'og:url', content: 'https://rolldown.rs/' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:site', content: '@rolldown_rs' }],
  ],

  themeConfig: {
    logo: { src: '/lightning-down.svg', width: 24, height: 24 },

    // https://vitepress.dev/reference/default-theme-config
    nav: [
      {
        text: 'About',
        link: '/about.md',
      },
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
                text: 'Discord Chat',
                link: 'https://chat.rolldown.rs',
              },
            ],
          },
        ],
      },
    ],

    sidebar: {
      '/contrib-guide/': [
        { text: 'Overview', link: '/contrib-guide/' },
        { text: 'Setup', link: '/contrib-guide/setup.md' },
        { text: 'Build', link: '/contrib-guide/build.md' },
        { text: 'Testing', link: '/contrib-guide/testing.md' },
        { text: 'Benchmark', link: '/contrib-guide/benchmark.md' },
        { text: 'Docs', link: '/contrib-guide/docs.md' },
        { text: 'Release', link: '/contrib-guide/release.md' },
      ],
    },

    socialLinks: [
      { icon: 'x', link: 'https://twitter.com/rolldown_rs' },
      { icon: 'discord', link: 'https://chat.rolldown.rs' },
      { icon: 'github', link: 'https://github.com/rolldown/rolldown' },
    ],

    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2023-present Rolldown Team & Contributors',
    },
  },
})
