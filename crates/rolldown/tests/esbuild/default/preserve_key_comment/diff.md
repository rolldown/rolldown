# Diff
## /out/entry.js
### esbuild
```js
x(
  /* __KEY__ */
  "notKey",
  /* __KEY__ */
  `notKey`
);
x(/* @__KEY__ */ "key", /* @__KEY__ */ `key`);
x(/* @__KEY__ */ "alsoKey", /* @__KEY__ */ `alsoKey`);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-x("notKey", `notKey`);
-x("key", `key`);
-x("alsoKey", `alsoKey`);

```