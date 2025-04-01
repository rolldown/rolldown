# Reason
1. different naming style of `ns`
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
"use strict";


const ext = __toESM(require("ext"));

//#region entry.js
console.log(ext.mustBeUnquoted, ext["mustBeQuoted"]);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var ns = __toESM(require("ext"));
-console.log(ns.mustBeUnquoted, ns["mustBeQuoted"]);
+var ext = __toESM(require("ext"));
+console.log(ext.mustBeUnquoted, ext["mustBeQuoted"]);

```