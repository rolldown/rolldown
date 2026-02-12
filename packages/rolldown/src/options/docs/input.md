#### Examples

##### Single entry

```js
export default defineConfig({
  input: 'src/index.js',
});
```

##### Multiple entries

```js
export default defineConfig({
  input: ['src/index.js', 'src/vendor.js'],
});
```

##### Named multiple entries

```js
export default defineConfig({
  input: {
    index: 'src/index.js',
    utils: 'src/utils/index.js',
    'components/Foo': 'src/components/Foo.js',
  },
});
```

#### In-depth

`input` allows you to specify one or more [entries](/glossary/entry) with [names](/glossary/entry-name) for the bundling process.

When multiple entries are specified (either as an array or an object), Rolldown will create separate [entry chunks](/glossary/entry-chunk) for each entry. If a module is referenced from multiple entries, Rolldown will share the code of that module for those entries.

The generated chunk names will follow the [`output.chunkFileNames`](/reference/OutputOptions.chunkFileNames) option. When using the object form, the `[name]` portion of the file name will be the name of the object property while for the array form, it will be the file name of the entry point. Note that it is possible when using the object form to put entry points into different sub-folders by adding a `/` to the name.
