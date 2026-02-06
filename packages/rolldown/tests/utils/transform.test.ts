import path from 'node:path';
import {
  transform,
  transformSync,
  TsconfigCache,
} from 'rolldown/utils';
import { expect, describe, it } from 'vitest';

describe('enhanced transform', () => {
  describe('basic transformation', () => {
    it('should transform TypeScript code', async () => {
      const result = await transform('test.ts', 'const x: number = 1;');
      expect(result.code).toBe('const x = 1;\n');
      expect(result.errors).toHaveLength(0);
    });

    it('should transform TypeScript code sync', () => {
      const result = transformSync('test.ts', 'const x: number = 1;');
      expect(result.code).toBe('const x = 1;\n');
      expect(result.errors).toHaveLength(0);
    });

    it('should generate sourcemap when enabled', async () => {
      const result = await transform('test.ts', 'const x: number = 1;', {
        sourcemap: true,
      });
      expect(result.code).toBe('const x = 1;\n');
      expect(result.map).toBeDefined();
    });
  });

  describe('tsconfig - raw options', () => {
    it('should use raw tsconfig JSX options', async () => {
      const result = await transform('test.tsx', '<div />', {
        tsconfig: {
          compilerOptions: {
            jsx: 'react-jsx',
            jsxImportSource: 'react',
          },
        },
      });
      expect(result.code).toContain('jsx');
      expect(result.errors).toHaveLength(0);
    });

    it('should use raw tsconfig decorator options', async () => {
      const code = `
        function decorator(target: any) { return target; }
        @decorator
        class MyClass {}
      `;
      const result = await transform('test.ts', code, {
        tsconfig: {
          compilerOptions: {
            experimentalDecorators: true,
          },
        },
      });
      expect(result.errors).toHaveLength(0);
    });

    it('should use raw tsconfig target options', async () => {
      const result = await transform('test.ts', 'const x: number = 1;', {
        tsconfig: {
          compilerOptions: {
            target: 'es2015',
          },
        },
      });
      expect(result.code).toBe('const x = 1;\n');
      expect(result.errors).toHaveLength(0);
    });
  });

  describe('tsconfig - auto-discovery', () => {
    const fixtures = path.join(import.meta.dirname, 'fixtures');

    it('should auto-discover tsconfig by default (no tsconfig option)', async () => {
      const result = await transform(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
      );
      expect(result.code).toBe('export const a = 1;\n');
      expect(result.errors).toHaveLength(0);
      expect(result.tsconfigFilePaths.length).toBeGreaterThan(0);
      expect(result.tsconfigFilePaths[0]).toContain('tsconfig.json');
    });

    it('should auto-discover tsconfig by default (no tsconfig option) - sync', () => {
      const result = transformSync(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
      );
      expect(result.code).toBe('export const a = 1;\n');
      expect(result.errors).toHaveLength(0);
      expect(result.tsconfigFilePaths.length).toBeGreaterThan(0);
      expect(result.tsconfigFilePaths[0]).toContain('tsconfig.json');
    });

    it('should auto-discover tsconfig by default with empty options', async () => {
      const result = await transform(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
        {},
      );
      expect(result.code).toBe('export const a = 1;\n');
      expect(result.errors).toHaveLength(0);
      expect(result.tsconfigFilePaths.length).toBeGreaterThan(0);
    });

    it('should auto-discover tsconfig with explicit tsconfig: true', async () => {
      const result = await transform(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
        { tsconfig: true },
      );
      expect(result.code).toBe('export const a = 1;\n');
      expect(result.errors).toHaveLength(0);
      expect(result.tsconfigFilePaths.length).toBeGreaterThan(0);
      expect(result.tsconfigFilePaths[0]).toContain('tsconfig.json');
    });

    it('should auto-discover tsconfig with explicit tsconfig: true (sync)', () => {
      const result = transformSync(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
        { tsconfig: true },
      );
      expect(result.code).toBe('export const a = 1;\n');
      expect(result.errors).toHaveLength(0);
      expect(result.tsconfigFilePaths.length).toBeGreaterThan(0);
      expect(result.tsconfigFilePaths[0]).toContain('tsconfig.json');
    });

    it('should not report tsconfigFilePaths when using raw tsconfig', async () => {
      const result = await transform(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
        {
          tsconfig: {
            compilerOptions: {
              target: 'es2015',
            },
          },
        },
      );
      expect(result.code).toBe('export const a = 1;\n');
      expect(result.errors).toHaveLength(0);
      // Raw tsconfig doesn't load from file, so no file paths
      expect(result.tsconfigFilePaths).toHaveLength(0);
    });
  });

  describe('TsconfigCache', () => {
    const fixtures = path.join(import.meta.dirname, 'fixtures');

    it('should create cache instance', () => {
      const cache = new TsconfigCache();
      expect(cache.size()).toBe(0);
    });

    it('should use cache for multiple transforms', async () => {
      const cache = new TsconfigCache();
      const result1 = await transform(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
        undefined,
        cache,
      );
      const result2 = await transform(
        path.join(fixtures, 'test2.ts'),
        'export const b: number = 2;',
        undefined,
        cache,
      );
      expect(result1.code).toBe('export const a = 1;\n');
      expect(result2.code).toBe('export const b = 2;\n');
      expect(cache.size()).toBe(1);
      cache.clear();
      expect(cache.size()).toBe(0);
    });

    it('should use cache for sync transforms', () => {
      const cache = new TsconfigCache();
      const result1 = transformSync(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
        undefined,
        cache,
      );
      const result2 = transformSync(
        path.join(fixtures, 'test2.ts'),
        'export const b: number = 2;',
        undefined,
        cache,
      );
      expect(result1.code).toBe('export const a = 1;\n');
      expect(result2.code).toBe('export const b = 2;\n');
      expect(cache.size()).toBe(1);
      cache.clear();
      expect(cache.size()).toBe(0);
    });

    it('should produce correct results when transforming same file twice with cache', async () => {
      const cache = new TsconfigCache();
      const result1 = await transform(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 1;',
        undefined,
        cache,
      );
      const result2 = await transform(
        path.join(fixtures, 'test1.ts'),
        'export const a: number = 2;',
        undefined,
        cache,
      );
      expect(result1.code).toBe('export const a = 1;\n');
      expect(result2.code).toBe('export const a = 2;\n');
      // Same tsconfig resolved, cache should not grow
      expect(cache.size()).toBe(1);
    });
  });

  describe('result properties', () => {
    it('should return helpersUsed', async () => {
      const result = await transform('test.ts', 'const x: number = 1;');
      expect(result.helpersUsed).toBeDefined();
      expect(typeof result.helpersUsed).toBe('object');
    });

    it('should return errors array', async () => {
      const result = await transform('test.ts', 'const x: number = 1;');
      expect(Array.isArray(result.errors)).toBe(true);
    });

    it('should return warnings array', async () => {
      const result = await transform('test.ts', 'const x: number = 1;');
      expect(Array.isArray(result.warnings)).toBe(true);
    });

    it('should return tsconfigFilePaths array', async () => {
      const result = await transform('test.ts', 'const x: number = 1;');
      expect(Array.isArray(result.tsconfigFilePaths)).toBe(true);
    });

    it('should return sourcemap with proper structure', async () => {
      const result = await transform('test.ts', 'const x: number = 1;', {
        sourcemap: true,
      });
      expect(result.map).toBeDefined();
      expect(result.map?.sources).toBeDefined();
      expect(result.map?.mappings).toBeDefined();
      expect(result.map?.sources).toContain('test.ts');
    });
  });

  describe('error handling', () => {
    it('should handle syntax errors gracefully', async () => {
      const result = await transform('test.ts', 'const x: = 1;');
      // Oxc parser recovers from errors, so code may still be produced
      expect(result.errors.length).toBeGreaterThan(0);
      for (const error of result.errors) {
        expect(error).toBeInstanceOf(Error);
        expect(typeof error.message).toBe('string');
      }
    });

    it('should handle syntax errors gracefully (sync)', () => {
      const result = transformSync('test.ts', 'const x: = 1;');
      // Oxc parser recovers from errors, so code may still be produced
      expect(result.errors.length).toBeGreaterThan(0);
      for (const error of result.errors) {
        expect(error).toBeInstanceOf(Error);
        expect(typeof error.message).toBe('string');
      }
    });
  });

  describe('helpers option', () => {
    // Code that triggers helper usage: class with decorators
    const decoratorCode = `
      function decorator(target: any) { return target; }
      @decorator
      class MyClass {
        field = 1;
      }
    `;

    it('should use Runtime mode by default', async () => {
      const result = await transform('test.ts', decoratorCode, {
        tsconfig: {
          compilerOptions: {
            experimentalDecorators: true,
          },
        },
      });
      expect(result.errors).toHaveLength(0);
      // Runtime mode imports from runtime package
      expect(result.code).toContain('@oxc-project/runtime');
      expect(result.code).not.toContain('babelHelpers');
    });

    it('should use Runtime mode when set', async () => {
      const result = await transform('test.ts', decoratorCode, {
        tsconfig: {
          compilerOptions: {
            experimentalDecorators: true,
          },
        },
        helpers: {
          mode: 'Runtime',
        },
      });
      expect(result.errors).toHaveLength(0);
      // Runtime mode imports from runtime package
      expect(result.code).toContain('@oxc-project/runtime');
      expect(result.code).not.toContain('babelHelpers');
    });

    it('should use External mode when explicitly set', async () => {
      const result = await transform('test.ts', decoratorCode, {
        tsconfig: {
          compilerOptions: {
            experimentalDecorators: true,
          },
        },
        helpers: {
          mode: 'External',
        },
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('babelHelpers');
      expect(result.code).not.toContain('@oxc-project/runtime');
    });

    it('should track helpers used in helpersUsed', async () => {
      const result = await transform('test.ts', decoratorCode, {
        tsconfig: {
          compilerOptions: {
            experimentalDecorators: true,
          },
        },
      });
      expect(result.errors).toHaveLength(0);
      expect(Object.keys(result.helpersUsed).length).toBeGreaterThan(0);
    });

    it('should use Runtime mode by default (sync)', () => {
      const result = transformSync('test.ts', decoratorCode, {
        tsconfig: {
          compilerOptions: {
            experimentalDecorators: true,
          },
        },
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('@oxc-project/runtime');
    });

    it('should use External mode when set (sync)', () => {
      const result = transformSync('test.ts', decoratorCode, {
        tsconfig: {
          compilerOptions: {
            experimentalDecorators: true,
          },
        },
        helpers: {
          mode: 'External',
        },
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('babelHelpers');
    });
  });

  describe('lang option', () => {
    it('should treat .js file as TypeScript with lang: ts', async () => {
      const result = await transform('file.js', 'const x: number = 1;', {
        lang: 'ts',
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toBe('const x = 1;\n');
    });

    it('should treat .js file as TypeScript with lang: ts (sync)', () => {
      const result = transformSync('file.js', 'const x: number = 1;', {
        lang: 'ts',
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toBe('const x = 1;\n');
    });

    it('should error when .js file has TS syntax without lang override', async () => {
      const result = await transform('file.js', 'const x: number = 1;');
      // Without lang override, .js file should not accept TypeScript type annotations
      expect(result.errors.length).toBeGreaterThan(0);
    });
  });

  describe('sourceType option', () => {
    it('should treat code as module', async () => {
      const result = await transform(
        'test.ts',
        'export const x: number = 1;',
        { sourceType: 'module' },
      );
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('export');
    });

    it('should treat code as script', async () => {
      const result = await transform('test.ts', 'const x: number = 1;', {
        sourceType: 'script',
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toBe('const x = 1;\n');
    });

    it('should treat code as commonjs', async () => {
      const result = await transform('test.js', 'const x = 1;', {
        sourceType: 'commonjs',
      });
      expect(result.errors).toHaveLength(0);
    });

    it('should treat code as unambiguous', async () => {
      const result = await transform(
        'test.ts',
        'export const x: number = 1;',
        { sourceType: 'unambiguous' },
      );
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('export');
    });

    it('should work with sourceType module (sync)', () => {
      const result = transformSync(
        'test.ts',
        'export const x: number = 1;',
        { sourceType: 'module' },
      );
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('export');
    });

    it('should combine lang and sourceType', async () => {
      const result = await transform(
        'file.js',
        'export const x: number = 1;',
        { lang: 'ts', sourceType: 'module' },
      );
      expect(result.errors).toHaveLength(0);
      expect(result.code).toBe('export const x = 1;\n');
    });

    it('should combine lang and sourceType (sync)', () => {
      const result = transformSync(
        'file.js',
        'export const x: number = 1;',
        { lang: 'ts', sourceType: 'module' },
      );
      expect(result.errors).toHaveLength(0);
      expect(result.code).toBe('export const x = 1;\n');
    });
  });

  describe('define', () => {
    it('should replace global identifiers', async () => {
      const result = await transform('test.ts', 'console.log(process.env.NODE_ENV);', {
        define: {
          'process.env.NODE_ENV': '"production"',
        },
        tsconfig: false,
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('"production"');
      expect(result.code).not.toContain('process.env.NODE_ENV');
    });

    it('should replace global identifiers (sync)', () => {
      const result = transformSync('test.ts', 'console.log(process.env.NODE_ENV);', {
        define: {
          'process.env.NODE_ENV': '"production"',
        },
        tsconfig: false,
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('"production"');
      expect(result.code).not.toContain('process.env.NODE_ENV');
    });
  });

  describe('inject', () => {
    it('should inject namespace import', async () => {
      const result = await transform('test.ts', 'console.log(Buffer.from("hello"));', {
        inject: {
          Buffer: 'buffer',
        },
        tsconfig: false,
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('import');
      expect(result.code).toContain('buffer');
    });

    it('should inject namespace import (sync)', () => {
      const result = transformSync('test.ts', 'console.log(Buffer.from("hello"));', {
        inject: {
          Buffer: 'buffer',
        },
        tsconfig: false,
      });
      expect(result.errors).toHaveLength(0);
      expect(result.code).toContain('import');
      expect(result.code).toContain('buffer');
    });
  });

  describe('inputMap', () => {
    // Use TypeScript code so the transformation actually produces meaningful sourcemaps.
    // This simulates: original.ts (TS) -> intermediate.js (via previous tool) -> final.js (via transform)
    const tsCode = 'const a: number = 1;\n';
    const intermediateCode = 'const a = 1;\n';
    const intermediateMap = {
      version: 3 as const,
      sources: ['original.ts'],
      sourcesContent: [tsCode],
      names: [],
      mappings: 'AAAA,MAAM,IAAY',
    };

    it('should collapse inputMap with output sourcemap', async () => {
      const result = await transform('intermediate.js', intermediateCode, {
        sourcemap: true,
        inputMap: intermediateMap,
      });

      expect(result.code).toBe('const a = 1;\n');
      expect(result.map).toBeDefined();
      // The collapsed map should trace back to the original TypeScript source
      expect(result.map?.sources).toEqual(['original.ts']);
      expect(result.map?.sourcesContent).toEqual([tsCode]);
    });

    it('should collapse inputMap with output sourcemap (sync)', () => {
      const result = transformSync('intermediate.js', intermediateCode, {
        sourcemap: true,
        inputMap: intermediateMap,
      });

      expect(result.code).toBe('const a = 1;\n');
      expect(result.map).toBeDefined();
      // The collapsed map should trace back to the original TypeScript source
      expect(result.map?.sources).toEqual(['original.ts']);
      expect(result.map?.sourcesContent).toEqual([tsCode]);
    });

    it('should not produce map when sourcemap is disabled', async () => {
      const result = await transform('intermediate.js', intermediateCode, {
        sourcemap: false,
        inputMap: intermediateMap,
      });

      expect(result.code).toBe('const a = 1;\n');
      expect(result.map).toBeUndefined();
    });

    it('should work without inputMap when sourcemap is enabled', async () => {
      const result = await transform('test.js', 'const a = 1;', {
        sourcemap: true,
      });

      expect(result.code).toBe('const a = 1;\n');
      expect(result.map).toBeDefined();
      expect(result.map?.sources).toEqual(['test.js']);
    });
  });
});
