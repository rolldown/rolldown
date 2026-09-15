import path from 'node:path';
import fs from 'node:fs';
import { resolveTsconfig } from 'rolldown/experimental';
import { TsconfigCache } from 'rolldown/utils';
import { expect, describe, it } from 'vitest';

describe('resolveTsconfig', () => {
  const fixtures = path.join(import.meta.dirname, 'fixtures');

  it('should resolve tsconfig for a file', () => {
    const result = resolveTsconfig(path.join(fixtures, 'test1.ts'));
    expect(result).not.toBeNull();
    expect(result!.tsconfig.compilerOptions).toBeDefined();
    expect(result!.tsconfig.compilerOptions.useDefineForClassFields).toBe(false);
    expect(result!.tsconfigFilePaths.length).toBeGreaterThan(0);
    expect(result!.tsconfigFilePaths[0]).toContain('tsconfig.json');
  });

  it('should return null for a file with no tsconfig', () => {
    // Use a path in the filesystem root where no tsconfig.json exists
    const result = resolveTsconfig('/nonexistent/path/test.ts');
    expect(result).toBeNull();
  });

  it('should accept a TsconfigCache', () => {
    const cache = new TsconfigCache();
    const result1 = resolveTsconfig(path.join(fixtures, 'test1.ts'), cache);
    expect(result1).not.toBeNull();
    expect(cache.size()).toBe(1);

    // Second call should use the cache
    const result2 = resolveTsconfig(path.join(fixtures, 'test1.ts'), cache);
    expect(result2).not.toBeNull();
    expect(cache.size()).toBe(1);

    expect(result1!.tsconfig.compilerOptions.useDefineForClassFields).toBe(
      result2!.tsconfig.compilerOptions.useDefineForClassFields,
    );
  });

  it('should use an explicit tsconfig with TsconfigCache', () => {
    const explicitTsconfig = path.join(fixtures, 'extends', 'tsconfig.json');
    const cache = new TsconfigCache(explicitTsconfig);
    const result = resolveTsconfig(path.join(fixtures, 'test1.ts'), cache);

    expect(result).not.toBeNull();
    expect(result!.tsconfig.compilerOptions.experimentalDecorators).toBe(true);
    expect(result!.tsconfigFilePaths[0]).toBe(explicitTsconfig);
  });

  it('should reload a tsconfig after clearing the cache', () => {
    const fixture = path.join(fixtures, 'cache-clear');
    const tsconfig = path.join(fixture, 'tsconfig.json');
    const source = path.join(fixture, 'source.ts');
    const originalTsconfig = fs.readFileSync(tsconfig, 'utf8');

    try {
      const cache = new TsconfigCache(tsconfig);
      expect(resolveTsconfig(source, cache)!.tsconfig.compilerOptions.useDefineForClassFields).toBe(
        false,
      );

      fs.writeFileSync(
        tsconfig,
        JSON.stringify({ compilerOptions: { useDefineForClassFields: true } }),
      );
      cache.clear();

      expect(resolveTsconfig(source, cache)!.tsconfig.compilerOptions.useDefineForClassFields).toBe(
        true,
      );
    } finally {
      fs.writeFileSync(tsconfig, originalTsconfig);
    }
  });

  it('should resolve extended tsconfig options', () => {
    const result = resolveTsconfig(path.join(fixtures, 'extends', 'test.ts'));
    expect(result).not.toBeNull();
    // Own option from extends/tsconfig.json
    expect(result!.tsconfig.compilerOptions.experimentalDecorators).toBe(true);
    // Inherited option from parent tsconfig.json
    expect(result!.tsconfig.compilerOptions.useDefineForClassFields).toBe(false);
  });
});
