# Source Code Transformations

If a plugin transforms source code, it should generate a sourcemap automatically, unless there's a specific `sourceMap: false` option. Rolldown only cares about the `mappings` property (everything else is handled automatically). [magic-string](https://github.com/Rich-Harris/magic-string) provides a simple way to generate such a map for elementary transformations like adding or removing code snippets.

If it doesn't make sense to generate a sourcemap, return an empty sourcemap:

```js
return {
  code: transformedCode,
  map: { mappings: '' },
};
```

If the transformation does not move code, you can preserve existing sourcemaps by returning `null`:

```js
return {
  code: transformedCode,
  map: null,
};
```
