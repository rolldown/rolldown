import { describe, expect, it, vi } from 'vitest';
import { transform } from '../../src/utils/transform';

const { enhancedTransform } = vi.hoisted(() => ({
  enhancedTransform: vi.fn(async () => ({
    code: 'const x = 1;\n',
    errors: [],
    warnings: [],
  })),
}));

vi.mock('../../src/binding.cjs', () => ({
  enhancedTransform,
  enhancedTransformSync: vi.fn(),
}));

describe('transform cwd default', () => {
  it('should not require process.cwd', async () => {
    // A browser build can expose a partial `process` object without `cwd`.
    const processWithOptionalCwd = process as unknown as { cwd?: () => string };
    const cwd = processWithOptionalCwd.cwd;
    processWithOptionalCwd.cwd = undefined;
    try {
      const result = await transform('test.ts', 'const x: number = 1;');
      expect(result.code).toBe('const x = 1;\n');
    } finally {
      processWithOptionalCwd.cwd = cwd;
    }
    expect(enhancedTransform).toHaveBeenCalledWith(
      'test.ts',
      'const x: number = 1;',
      undefined,
      undefined,
      false,
    );
  });
});
