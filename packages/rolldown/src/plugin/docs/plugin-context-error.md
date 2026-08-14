In the [`onLog`](/reference/Interface.Plugin#onlog) hook, this function is an easy way to turn warnings into errors while keeping all additional properties of the warning:

```js
function myPlugin() {
  return {
    name: 'my-plugin',
    onLog(level, log) {
      if (level === 'warn' && log.code === 'THIS_IS_NOT_OK') {
        return this.error(log);
      }
    },
  };
}
```
