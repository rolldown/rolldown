import { expect } from 'vitest'
import path from 'node:path'
import { defineTest } from '@tests'
import { NormalizedInputOptions, NormalizedOutputOptions } from 'rolldown'

const entry = path.join(__dirname, './main.js')

const allInputOptions: NormalizedInputOptions[] = []
const allOutputOptions: NormalizedOutputOptions[] = []

export default defineTest({
  config: {
    input: entry,
    cwd: __dirname,
    platform: 'node',
    output: {
      name: 'test',
      cssEntryFileNames: '[name].css',
      cssChunkFileNames: () => {
        return '[name]-[hash].css'
      },
      entryFileNames: '[name].js',
      chunkFileNames: () => {
        return '[name]-[hash].js'
      },
      assetFileNames: 'assets/[name]-[hash][extname]',
      file: 'dist/[name].js',
      format: 'umd',
      exports: 'auto',
      esModule: 'if-default-prop',
      inlineDynamicImports: true,
      sourcemap: 'inline',
      banner: '/* banner */',
      footer: () => {
        return '/* footer */'
      },
      outro: '/* outro */',
      externalLiveBindings: true,
    },

    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (_code, _chunk, outputOptions) => {
          allOutputOptions.push(outputOptions)
        },
        generateBundle: (outputOptions) => {
          allOutputOptions.push(outputOptions)
        },
        writeBundle: (outputOptions) => {
          allOutputOptions.push(outputOptions)
        },
        renderStart: (outputOptions, inputOptions) => {
          allInputOptions.push(inputOptions)
          allOutputOptions.push(outputOptions)
        },
        buildStart: (inputOptions) => {
          allInputOptions.push(inputOptions)
        },
      },
    ],
  },
  afterTest: (_) => {
    expect(allInputOptions.length).toBeGreaterThan(0)
    expect(allOutputOptions.length).toBeGreaterThan(0)

    allInputOptions.forEach((option) => {
      expect(option.input).toEqual([entry])
      expect(option.cwd).toEqual(__dirname)
      expect(option.platform).toEqual('node')
      expect(option.shimMissingExports).toBe(false)
    })

    allOutputOptions.forEach((option) => {
      expect(option.name).toBe('test')
      expect(option.cssEntryFileNames).toBe('[name].css')
      expect(option.cssChunkFileNames).toBeInstanceOf(Function)
      expect(option.cssChunkFileNames).toThrow(
        'You should not take `NormalizedOutputOptions#cssChunkFileNames` and call it directly',
      )
      expect(option.entryFileNames).toBe('[name].js')
      expect(option.chunkFileNames).toBeInstanceOf(Function)
      expect(option.assetFileNames).toBe('assets/[name]-[hash][extname]')
      expect(option.file).toBe('dist/[name].js')
      expect(option.dir).toBe(undefined)
      expect(option.format).toBe('umd')
      expect(option.exports).toBe('auto')
      expect(option.esModule).toBe('if-default-prop')
      expect(option.inlineDynamicImports).toBe(true)
      expect(option.sourcemap).toBe('inline')
      // all of these addon options are Function in rust side currently
      expect(option.banner).toBeInstanceOf(Function)
      expect(option.banner).toThrow(
        'You should not take `NormalizedOutputOptions#banner` and call it directly',
      )
      expect(option.footer).toBeInstanceOf(Function)
      expect(option.footer).toThrow(
        'You should not take `NormalizedOutputOptions#footer` and call it directly',
      )
      expect(option.intro).toBeInstanceOf(Function)
      expect(option.intro).toThrow(
        'You should not take `NormalizedOutputOptions#intro` and call it directly',
      )
      expect(option.outro).toBeInstanceOf(Function)
      expect(option.outro).toThrow(
        'You should not take `NormalizedOutputOptions#outro` and call it directly',
      )
      expect(option.externalLiveBindings).toBe(true)
    })
  },
})
