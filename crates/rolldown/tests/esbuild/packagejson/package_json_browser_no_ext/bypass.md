# Reason
1. different fs
2. different naming style
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/demo-pkg/no-ext-browser.js
var browser = "browser";

// Users/user/project/src/demo-pkg/no-ext.js
var node = "node";

// Users/user/project/src/demo-pkg/ext-browser.js
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

//#region src/demo-pkg/no-ext-browser.js
let browser$1 = "browser";

//#endregion
//#region src/demo-pkg/no-ext.js
let node = "node";

//#endregion
//#region src/demo-pkg/ext-browser.js
let browser = "browser";

//#endregion
//#region src/entry.js
assert.equal(browser$1, "browser");
assert.equal(node, "node");
assert.equal(browser, "browser");
assert.equal(browser, "browser");

//#endregion
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