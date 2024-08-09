import { defineConfig } from 'rolldown'

export default defineConfig({
  input: './index.js',
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  plugins: [
    {
      name: 'post',
      buildStart: {
        handler: () => {
          console.log('post')
        },
        order: 'post',
      },
    },
    {
      name: 'pre',
      buildStart: {
        handler: () => {
          console.log('pre')
        },
        order: 'pre',
      },
    },
    {
      name: 'normal',
      buildStart: () => {
        console.log('normal')
      },
    },
  ],
})
