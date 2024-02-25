import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: 'Rolldown',
  description:
    'Fast JavaScript/TypeScript bundler in Rust with Rollup-compatible API',
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Guide', link: '/guide/getting-started.md' },
      { text: 'Dev Guide', link: '/dev-guide/getting-started.md' },
    ],

    sidebar: {
      '/guide/': {
        base: '/guide/',
        items: [{ text: 'Getting Started', link: '/getting-started.md' }],
      },
      '/dev-guide/': {
        base: '/dev-guide/',
        items: [
          { text: 'Getting Started', link: '/getting-started.md' },
          { text: 'Setup', link: '/setup.md' },
          { text: 'Testing', link: '/testing.md' },
        ],
      },
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/vuejs/vitepress' },
    ],
  },
})
