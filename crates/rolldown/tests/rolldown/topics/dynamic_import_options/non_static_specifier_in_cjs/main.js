// Non-static specifier, one argument: lowered to `require`.
import(moduleName).then(console.log);

// Non-static specifier with attributes: kept as a native `import()`.
import(moduleName, { with: { type: 'json' } }).then(console.log);

// `@vite-ignore` suppresses the import record the same way.
import(/* @vite-ignore */ './ignored.js', { with: { type: 'json' } }).then(console.log);
