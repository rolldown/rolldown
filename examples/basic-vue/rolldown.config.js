import { defineConfig } from 'rolldown'
import * as fs from 'fs'

export default defineConfig({
  input: './index.js',
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  // plugins: [
  //   {
  //     name: "test-plugin1",
  //     load: function (id) {
  //       throw new Error('load error')
  //     },
  //   },
  //   {
  //     name: "test-plugin",
  //     outputOptions: function (options) {
  //       options.banner = "/* banner */";
  //       return options;
  //     },
  //   },
  //   {
  //     name: "test-plugin2",
  //     outputOptions: function (options) {
  //       options.banner = "/* banner */";
  //       return options;
  //     },
  //   },
  // ],
  output: {
    exports: 'named',
    format: 'iife',
    esModule: 'if-default-prop',
  },
  // experimental: {
  //   enableComposingJsPlugins: true,
  // },
})
