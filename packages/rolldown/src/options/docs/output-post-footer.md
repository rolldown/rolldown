#### Examples

##### Build timestamp

```js
export default {
  output: {
    minify: true,
    postFooter: `/* built: ${Date.now()} */`,
  },
};
```
