import path from 'node:path';
import type {
  ManglePropertiesOptions,
  NormalizedInputOptions,
  NormalizedOutputOptions,
} from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const entry = path.join(__dirname, './main.js');

const allInputOptions: NormalizedInputOptions[] = [];
const allOutputOptions: NormalizedOutputOptions[] = [];

const chunkFileNames = () => '[name]-[hash].js';
const footer = () => '/* footer */';
const outputPlugin = {
  name: 'test-output-plugin',
};
const sourcemapIgnoreList = () => false;

export default defineTest({
  config: {
    input: entry,
    cwd: __dirname,
    platform: 'node',
    context: 'window',
    output: {
      name: 'test',
      entryFileNames: '[name].js',
      chunkFileNames,
      assetFileNames: 'assets/[name]-[hash][extname]',
      file: 'dist/[name].js',
      format: 'umd',
      exports: 'auto',
      esModule: 'if-default-prop',
      inlineDynamicImports: true,
      sourcemap: 'inline',
      banner: '/* banner */',
      footer,
      outro: '/* outro */',
      externalLiveBindings: true,
      plugins: [outputPlugin],
      sourcemapIgnoreList,
      legalComments: 'inline',
      preserveModulesRoot: 'src',
      virtualDirname: 'virtual',
      minifyInternalExports: true,
      minify: {
        compress: false,
        mangle: false,
        codegen: false,
        mangleProps: {
          include: /^_/i,
          exclude: /^_skip$/s,
          reserved: ['_reserved'],
          quoted: true,
          debug: true,
          cache: { _shared: 'shared', _keep: false },
        },
      },
    },
    plugins: [
      {
        name: 'test-plugin',
        renderChunk(_code, _chunk, outputOptions) {
          allOutputOptions.push(outputOptions);
        },
        generateBundle(outputOptions) {
          allOutputOptions.push(outputOptions);
        },
        writeBundle(outputOptions) {
          allOutputOptions.push(outputOptions);
        },
        renderStart(outputOptions, inputOptions) {
          allInputOptions.push(inputOptions);
          allOutputOptions.push(outputOptions);
        },
        buildStart(inputOptions) {
          allInputOptions.push(inputOptions);
        },
      },
    ],
  },
  afterTest: (_) => {
    expect(allInputOptions.length).toBeGreaterThan(0);
    expect(allOutputOptions.length).toBeGreaterThan(0);

    allInputOptions.forEach((option) => {
      expect(option.input).toEqual([entry]);
      expect(option.cwd).toEqual(__dirname);
      expect(option.platform).toEqual('node');
      expect(option.shimMissingExports).toBe(false);
      expect(option.context).toBe('window');
    });

    allOutputOptions.forEach((option) => {
      expect(option.name).toBe('test');
      expect(option.entryFileNames).toBe('[name].js');
      expect(option.chunkFileNames).toStrictEqual(chunkFileNames);
      expect(option.assetFileNames).toBe('assets/[name]-[hash][extname]');
      expect(option.file).toBe('dist/[name].js');
      expect(option.dir).toBe(undefined);
      expect(option.format).toBe('umd');
      expect(option.exports).toBe('auto');
      expect(option.esModule).toBe('if-default-prop');
      expect(option.inlineDynamicImports).toBe(true);
      expect(option.sourcemap).toBe('inline');
      // all of these addon options are Function in Rust side currently
      // @ts-expect-error need to RenderedChunk as argument
      expect(option.banner()).toBe('/* banner */');
      expect(option.footer).toStrictEqual(footer);
      // @ts-expect-error need to RenderedChunk as argument
      expect(option.intro()).toBe('');
      // @ts-expect-error need to RenderedChunk as argument
      expect(option.outro()).toBe('/* outro */');
      expect(option.externalLiveBindings).toBe(true);
      expect(option.plugins[0]).toStrictEqual(outputPlugin);
      expect(option.sourcemapIgnoreList).toStrictEqual(sourcemapIgnoreList);
      expect(option.legalComments).toBe('inline');
      expect(option.preserveModules).toBe(false);
      expect(option.preserveModulesRoot).toStrictEqual(path.join(__dirname, 'src'));
      expect(option.virtualDirname).toBe('virtual');
      expect(option.minifyInternalExports).toBe(true);

      const normalizedMangleProps = (
        option.minify as {
          mangleProps: ManglePropertiesOptions;
        }
      ).mangleProps;
      expect(normalizedMangleProps.include).toBeInstanceOf(RegExp);
      expect(normalizedMangleProps.include.source).toBe('^_');
      expect(normalizedMangleProps.include.flags).toBe('i');
      expect(normalizedMangleProps.exclude).toBeInstanceOf(RegExp);
      expect(normalizedMangleProps.exclude?.source).toBe('^_skip$');
      expect(normalizedMangleProps.exclude?.flags).toBe('s');
      expect(normalizedMangleProps.reserved).toEqual(['_reserved']);
      expect(normalizedMangleProps.quoted).toBe(true);
      expect(normalizedMangleProps.debug).toBe(true);
      expect(normalizedMangleProps.cache).toEqual({ _shared: 'shared', _keep: false });
    });
  },
});
