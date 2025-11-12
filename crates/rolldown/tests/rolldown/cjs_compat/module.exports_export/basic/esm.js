// ESM module that exports module.exports
export const foo = 'foo';
export const bar = 'bar';
export default { default: 'default-value' };
export const __esModule = true;

// This is the new Node.js feature - exporting module.exports
const moduleExports = { foo: 'foo' };
export { moduleExports as 'module.exports' };
