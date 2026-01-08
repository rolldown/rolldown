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
    components: 'src/components/index.js',
  },
});
```

#### In-depth

`input` allows you to specify one or more [entries](/glossary/entry) with [names](/glossary/entry-name) for the bundling process.

When multiple entries are specified (either as an array or an object), Rolldown will create separate [entry chunks](/glossary/entry-chunk) for each entry.
