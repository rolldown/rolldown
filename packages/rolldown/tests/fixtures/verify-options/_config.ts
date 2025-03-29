import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    output: {
      file: 'dist/main.js',
      target: 'es2015',
      banner: '/* banner */',
      intro: '/* intro */',
      outro: '/* outro */',
      comments: 'preserve-legal',
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
      cssEntryFileNames: '[name]-[hash].css',
      cssChunkFileNames: '[name]-[hash].css',
      assetFileNames: '[name]-[hash][extname]',
    },
    plugins: [
      {
        name: 'test-plugin',
        outputOptions: function (options) {
          expect(options.file).toBe('dist/main.js')
          expect(options.target).toBe('es2015')
          expect(options.banner).toBe('/* banner */')
          expect(options.intro).toBe('/* intro */')
          expect(options.outro).toBe('/* outro */')
          expect(options.comments).toBe('preserve-legal')
          expect(options.entryFileNames).toBe('[name]-[hash].js')
          expect(options.chunkFileNames).toBe('[name]-[hash].js')
          expect(options.cssEntryFileNames).toBe('[name]-[hash].css')
          expect(options.cssChunkFileNames).toBe('[name]-[hash].css')
          expect(options.assetFileNames).toBe('[name]-[hash][extname]')
        },
      },
    ],
  },
})
