# Reason
1. Currently there is no way to control *quoted* behavior, since we use `oxc` to convert ast to string
2. we just generate same output as esbuild if disable `MinifySyntax`
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var ns = __toESM(require("ext"));
console.log(ns.mustBeUnquoted, ns.mustBeUnquoted2);
```
### rolldown
```js

const ext = __toESM(require("ext"));

//#region entry.js
console.log(ext.mustBeUnquoted, ext["mustBeUnquoted2"]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var ns = __toESM(require("ext"));
-console.log(ns.mustBeUnquoted, ns.mustBeUnquoted2);
+var ext = __toESM(require("ext"));
+console.log(ext.mustBeUnquoted, ext["mustBeUnquoted2"]);

```