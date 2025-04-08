# Reason
1. different deconflict order
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/demo-pkg/no-ext-browser/index.js
var browser = "browser";

// Users/user/project/src/demo-pkg/no-ext/index.js
var node = "node";

// Users/user/project/src/demo-pkg/ext-browser/index.js
var browser2 = "browser";

// Users/user/project/src/entry.js
console.log(browser);
console.log(node);
console.log(browser2);
console.log(browser2);
```
### rolldown
```js
import assert from "node:assert";

//#region src/demo-pkg/no-ext-browser/index.js
let a = "browser";

//#endregion
//#region src/demo-pkg/no-ext/index.js
let b = "node";

//#endregion
//#region src/demo-pkg/ext-browser/index.js
let d = "browser";

//#endregion
//#region src/entry.js
assert.equal(a, "browser");
assert.equal(b, "node");
assert.equal(d, "browser");
assert.equal(d, "browser");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,7 +1,4 @@
-var browser = "browser";
-var node = "node";
-var browser2 = "browser";
-console.log(browser);
-console.log(node);
-console.log(browser2);
-console.log(browser2);
+var a = "browser";
+var b = "node";
+var d = "browser";
+console.log(a, b, d, d);

```