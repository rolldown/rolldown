For example, this warning happens with the following config:

```js [rolldown.config.js]
export default {
  input: ['src/entry1.js', 'src/entry2.js'],
  output: {
    // Both entries will try to use the same filename
    entryFileNames: 'bundle.js',
  },
};
```
