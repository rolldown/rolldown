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
import * as ns from "ext";

//#region entry.js
console.log(ns.mustBeUnquoted, ns["mustBeQuoted"]);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-var ns = __toESM(require("ext"));
+import * as ns from "ext";
 console.log(ns.mustBeUnquoted, ns["mustBeQuoted"]);

```