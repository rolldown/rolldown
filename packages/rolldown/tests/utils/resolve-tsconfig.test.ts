import path from 'node:path';
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

  it('should resolve extended tsconfig options', () => {
    const result = resolveTsconfig(path.join(fixtures, 'extends', 'test.ts'));
    expect(result).not.toBeNull();
    // Own option from extends/tsconfig.json
    expect(result!.tsconfig.compilerOptions.experimentalDecorators).toBe(true);
    // Inherited option from parent tsconfig.json
    expect(result!.tsconfig.compilerOptions.useDefineForClassFields).toBe(false);
  });
});
