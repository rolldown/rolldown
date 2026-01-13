**Multiple chunk groups with priorities**

```js
export default defineConfig({
  output: {
    advancedChunks: {
      groups: [
        {
          name: 'react-vendor',
          test: /node_modules[\\/]react/,
          priority: 20,
        },
        {
          name: 'ui-vendor',
          test: /node_modules[\\/](antd|@mui)/,
          priority: 15,
        },
        {
          name: 'vendor',
          test: /node_modules/,
          priority: 10,
        },
        {
          name: 'common',
          minShareCount: 2,
          minSize: 10000,
          priority: 5,
        },
      ],
    },
  },
});
```

**Size-based splitting**

```js
export default defineConfig({
  output: {
    advancedChunks: {
      groups: [
        {
          name: 'large-libs',
          test: /node_modules/,
          minSize: 100000, // 100KB
          maxSize: 250000, // 250KB
          priority: 10,
        },
      ],
    },
  },
});
```
