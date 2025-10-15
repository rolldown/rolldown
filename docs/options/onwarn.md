# onwarn

- **Type:** `(warning: RollupLog, defaultHandler: (warning: RollupLogWithString | (() => RollupLogWithString)) => void) => void`
- **Optional:** Yes ✅

:::warning Deprecated
This is a legacy API. Consider using [`onLog`](./on-log.md) instead for better control over all log types.
:::

Custom handler for warnings during the build process.
