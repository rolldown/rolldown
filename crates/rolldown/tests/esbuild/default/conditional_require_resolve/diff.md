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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,2 +0,0 @@
-x ? require.resolve("a") : y ? require.resolve("b") : require.resolve("c");
-x ? y ? require.resolve("a") : require.resolve("b") : require.resolve(c);

```