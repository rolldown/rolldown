import { walk } from 'oxc-walker';
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
      transform: {
        filter: {
          exclude: /node_modules/,
        },
        handler(code, id, meta) {
          if (meta?.magicString) {
            const { magicString } = meta;
            // Example 1: Replace 'Hello' with 'Hi'
            if (code.includes('Hello')) {
              magicString.replace('Hello', 'Hi');
            }

            // Example 2: Prepend a comment to each file
            magicString.prepend('/* Transformed by example-transform plugin */\n');

            // Example 3: Append a timestamp comment
            magicString.append(`\n/* Transformed at: ${new Date().toISOString()} */`);

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
    },
    {
      name: 'ast-magicstring-example',
      // This plugin demonstrates using both meta.ast and meta.magicString together
      // Transforms: fn(() => import(url)) -> fn(() => import(url), url)
      transform: {
        filter: {
          id: {
            include: /lazy-loader\.js$/,
          },
        },
        handler(code, id, meta) {
          // Both meta.ast and meta.magicString must be available
          if (!meta?.ast || !meta?.magicString) {
            return null;
          }

          const { ast, magicString } = meta;
          let transformed = false;

          // Use oxc-walker to traverse the AST
          walk(ast, {
            enter(node) {
              // Look for CallExpression nodes: fn(...)
              if (node.type === 'CallExpression' && node.arguments?.length === 1) {
                const arg = node.arguments[0];

                // Check if the argument is an arrow function: () => ...
                if (arg.type === 'ArrowFunctionExpression' && arg.params?.length === 0) {
                  // Check if the body is an import() call
                  let importCall = null;
                  if (arg.body.type === 'ImportExpression') {
                    importCall = arg.body;
                  } else if (
                    arg.body.type === 'BlockStatement' &&
                    arg.body.body.length === 1 &&
                    arg.body.body[0].type === 'ReturnStatement' &&
                    arg.body.body[0].argument?.type === 'ImportExpression'
                  ) {
                    importCall = arg.body.body[0].argument;
                  }

                  if (importCall && importCall.source) {
                    // Extract the import URL from the source code
                    const importSource = importCall.source;
                    const start = importSource.start;
                    const end = importSource.end;
                    const url = code.slice(start, end);

                    // Find the position to insert the second argument
                    // Insert after the closing parenthesis of the arrow function, before the closing parenthesis of the outer call
                    const insertPos = arg.end;

                    // Use magicString to insert the URL as a second argument
                    magicString.appendLeft(insertPos, `, ${url}`);
                    transformed = true;
                  }
                }
              }
            },
          });

          // Return the modified code only if we made transformations
          if (transformed) {
            return {
              code: magicString,
            };
          }

          return null;
        },
      },
    },
  ],
});
