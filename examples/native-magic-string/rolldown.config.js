import { defineConfig } from 'rolldown';

export default defineConfig({
  input: './index.js',
  experimental: {
    // Enable native Rust implementation of MagicString
    // This provides better performance for source map generation
    nativeMagicString: true,
  },
  output: {
    sourcemap: true,
  },
  plugins: [
    {
      name: 'example-transform',
      // This plugin demonstrates using magicString in the transform hook
      transform(code, id, meta) {
        // Skip node_modules
        if (id.includes('node_modules')) {
          return null;
        }

        if (meta?.magicString) {
          const { magicString } = meta;
          // Example 1: Replace 'Hello' with 'Hi'
          if (code.includes('Hello')) {
            magicString.replace('Hello', 'Hi');
          }

          // Example 2: Prepend a comment to each file
          magicString.prepend(
            '/* Transformed by example-transform plugin */\n',
          );

          // Example 3: Append a timestamp comment
          magicString.append(
            `\n/* Transformed at: ${new Date().toISOString()} */`,
          );

          // Return the modified magicString
          // The native implementation will generate source maps efficiently
          return {
            code: magicString,
          };
        } else {
          // Put the logic here when nativeMagicString is not available(Compatible with rollup or older versions of rolldown)
          return null;
        }
      },
    },
  ],
});
