# tsconfig

- **Type:** `string`
- **Optional:** Yes ✅

Specifies the path to a TypeScript configuration file. You may provide a relative path (resolved relative to [`cwd`](./cwd.md)) or an absolute path.

## Examples

### Use default tsconfig.json

```js
export default {
  tsconfig: './tsconfig.json',
};
```

### Use custom config

```js
export default {
  tsconfig: './tsconfig.build.json',
};
```

### Use absolute path

```js
export default {
  tsconfig: '/absolute/path/to/tsconfig.json',
};
```

## In-depth

When a tsconfig path is specified, Rolldown will:

### 1. Use compiler options for transpilation:

- `target`: ECMAScript version to compile to
- `jsx`: JSX transformation mode
- `experimentalDecorators`: Enable decorator support
- `emitDecoratorMetadata`: Emit decorator metadata

### 2. Use paths for module resolution:

- `compilerOptions.paths`: Path mapping for module resolution
- `compilerOptions.baseUrl`: Base directory for path resolution

### 3. Merge with transform options:

The tsconfig options will be merged with the top-level `transform` options, with `transform` options taking precedence.

### Example tsconfig.json:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "jsx": "react-jsx",
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"],
      "@components/*": ["src/components/*"]
    }
  }
}
```

With this configuration:

- TypeScript will be transpiled to ES2020
- JSX will use React's automatic runtime
- Path aliases like `@/utils` will resolve to `src/utils`

### Priority

Options specified directly in Rolldown configuration take precedence over `tsconfig.json` settings:

```js
export default {
  tsconfig: './tsconfig.json', // Has jsx: 'react-jsx'
  transform: {
    jsx: {
      mode: 'classic', // This takes precedence
    },
  },
};
```

### Automatic Discovery

Rolldown will NOT automatically look for `tsconfig.json` if this option is not specified. You must explicitly provide the path.

:::tip
For TypeScript projects, it's recommended to specify `tsconfig` to ensure consistent compilation behavior and enable path mapping.
:::
