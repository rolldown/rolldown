// Boundary lock-in for #9263: the RuntimeHelper meta bit only applies to
// `\0@oxc-project+runtime@.../helpers/*` resolved ids. A user-authored
// default-only ESM required from a CJS-classified caller must keep the
// existing esbuild/webpack-aligned namespace shape — `.default` must NOT
// be appended by the wrap path.
await import('./dist/main.js');
