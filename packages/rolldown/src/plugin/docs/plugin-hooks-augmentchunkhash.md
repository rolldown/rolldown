#### Example

The following plugin will invalidate the hash of chunk foo with the current timestamp:

```js
function augmentWithDatePlugin() {
  return {
    name: 'augment-with-date',
    augmentChunkHash(chunkInfo) {
      if (chunkInfo.name === 'foo') {
        return Date.now().toString();
      }
    },
  };
}
```
