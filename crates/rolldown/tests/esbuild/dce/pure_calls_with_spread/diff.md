# Diff
## /out.js
### esbuild
```js
// entry.js
[...args];
[...args];
```
### rolldown
```js

//#region entry.js
/* @__PURE__ */ foo(...args);
/* @__PURE__ */ new foo(...args);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-[...args];
-[...args];
+foo(...args);
+new foo(...args);

```