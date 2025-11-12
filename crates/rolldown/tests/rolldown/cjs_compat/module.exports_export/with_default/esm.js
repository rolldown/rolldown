// ESM module with both default export and module.exports
export default function myFunction() {
  return 'function value';
}

const moduleExports = { customExport: 'custom' };
export { moduleExports as 'module.exports' };
