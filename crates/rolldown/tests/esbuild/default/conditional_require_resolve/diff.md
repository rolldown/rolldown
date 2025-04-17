# Reason
1. not support conditional `require.resolve`
# Diff
## /out.js
### esbuild
```js
// a.js
x ? require.resolve("a") : y ? require.resolve("b") : require.resolve("c");
x ? y ? require.resolve("a") : require.resolve("b") : require.resolve(c);
```
### rolldown
```js

//#region a.js
require.resolve(x ? "a" : y ? "b" : "c");
require.resolve(x ? y ? "a" : "b" : c);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-x ? require.resolve("a") : y ? require.resolve("b") : require.resolve("c");
-x ? y ? require.resolve("a") : require.resolve("b") : require.resolve(c);
+require.resolve(x ? "a" : y ? "b" : "c");
+require.resolve(x ? y ? "a" : "b" : c);

```