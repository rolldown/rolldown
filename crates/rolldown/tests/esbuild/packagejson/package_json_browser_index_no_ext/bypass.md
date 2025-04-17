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
let browser$1 = "browser";

//#region src/demo-pkg/no-ext/index.js
let node = "node";

//#region src/demo-pkg/ext-browser/index.js
let browser = "browser";

//#region src/entry.js
assert.equal(browser$1, "browser");
assert.equal(node, "node");
assert.equal(browser, "browser");
assert.equal(browser, "browser");

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,7 +1,4 @@
-var browser = "browser";
+var browser$1 = "browser";
 var node = "node";
-var browser2 = "browser";
-console.log(browser);
-console.log(node);
-console.log(browser2);
-console.log(browser2);
+var browser = "browser";
+console.log(browser$1, node, browser, browser);

```