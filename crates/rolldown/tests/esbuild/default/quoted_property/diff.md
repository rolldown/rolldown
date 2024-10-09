# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var ns = __toESM(require("ext"));
console.log(ns.mustBeUnquoted, ns["mustBeQuoted"]);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var ns = __toESM(require("ext"));
-console.log(ns.mustBeUnquoted, ns["mustBeQuoted"]);

```