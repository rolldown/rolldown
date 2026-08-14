These logs are only processed if the [`logLevel`](/reference/InputOptions.logLevel) option is explicitly set to `"debug"`, otherwise it does nothing. Therefore, it is encouraged to add helpful debug logs to plugins as that can help spot issues while they will be efficiently muted by default.

::: tip Lazily Compute

If you need to do expensive computations to generate the log, make sure to use the function form so that these computations are only performed if the log is actually processed.

```js
function plugin() {
  return {
    name: 'test',
    transform(code, id) {
      this.debug(
        () => `transforming ${id},\n` + `module contains, ${code.split('\n').length} lines`,
      );
    },
  };
}
```

:::
