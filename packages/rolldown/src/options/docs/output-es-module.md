#### Interaction with Consuming Tools

Different tools handle the `__esModule` marker differently when importing your bundle:

- **Rolldown**: Use heuristics based on Node.js's behavior. See the [Bundling CJS](/in-depth/bundling-cjs#ambiguous-default-import-from-cjs-modules) guide for more details.
- **esbuild**: Use heuristics based on Node.js's behavior.
- **Node.js**: Does not respect `__esModule`. The default export is the `module.exports` value.
- **Babel**: Respects `__esModule`.
