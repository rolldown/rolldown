// Unused imports are automatically removed in TypeScript files (this
// is a mis-feature of the TypeScript language). However, injected
// imports are an esbuild feature so we get to decide what the
// semantics are. We do not want injected imports to disappear unless
// they have been explicitly marked as having no side effects.
console.log('must be present')