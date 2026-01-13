To migrate from `onwarn` to `onLog`, check the `level` parameter to filter for warnings:

```js
// Before: Using `onwarn`
export default {
  onwarn(warning, defaultHandler) {
    // Suppress certain warnings
    if (warning.code === 'CIRCULAR_DEPENDENCY') return;
    // Handle other warnings with default behavior
    defaultHandler(warning);
  },
};
```

```js
// After: Using `onLog`
export default {
  onLog(level, log, defaultHandler) {
    // Handle only warnings (same behavior as `onwarn`)
    if (level === 'warn') {
      // Suppress certain warnings
      if (log.code === 'CIRCULAR_DEPENDENCY') return;
      // Handle other warnings with default behavior
      defaultHandler(level, log);
    } else {
      // Let other log levels pass through
      defaultHandler(level, log);
    }
  },
};
```
