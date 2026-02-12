import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js'],
    experimental: {
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'test-magic-string-caching',
        transform(code, id, meta) {
          // Skip virtual modules (like \0rolldown/runtime.js)
          if (id.startsWith('\0')) {
            return null;
          }
          if (!meta?.magicString) {
            return null;
          }

          // Test 1: Verify that multiple accesses return the same cached instance
          // The magicString getter caches the instance on first access
          meta.magicString.overwrite(0, code.length, 'replaced;');
          const result = meta.magicString.toString();

          // This should work because both accesses return the same cached instance
          expect(result).toBe('replaced;');

          // Test 2: Verify instance identity - all accesses return the same object
          const ref1 = meta.magicString;
          const ref2 = meta.magicString;
          const ref3 = meta.magicString;

          // All references should point to the same instance
          expect(ref1 === ref2).toBe(true);
          expect(ref2 === ref3).toBe(true);

          // Test 3: Verify operations on different accesses affect the same instance
          meta.magicString.append('appended;');
          expect(meta.magicString.toString()).toBe('replaced;appended;');

          meta.magicString.prepend('prepended;');
          expect(meta.magicString.toString()).toBe(
            'prepended;replaced;appended;',
          );

          // Test 4: Verify hasChanged() works across accesses
          expect(meta.magicString.hasChanged()).toBe(true);

          return {
            code: meta.magicString,
          };
        },
      },
    ],
  },
  afterTest: function(output) {
    expect(output.output[0].code).toContain('prepended;\nreplaced;\nappended;');
    expect(output.output[0].map).toBeDefined();
  },
});
