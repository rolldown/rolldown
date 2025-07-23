# Reason
1. could be done in minifier
2. for `__require` diff, we don't have ModePassThrough
# Diff
## /out/esm.js
### esbuild
```js
export let foo_ = 123;
import { bar_ } from "xyz";
```
### rolldown
```js
import "xyz";

//#region esm.js
let foo_ = 123;

//#endregion
export { foo_ };
```
### diff
```diff
===================================================================
--- esbuild	/out/esm.js
+++ rolldown	esm.js
@@ -1,2 +1,3 @@
-export let foo_ = 123;
-import {bar_} from "xyz";
+import "xyz";
+var foo_ = 123;
+export {foo_};

```
## /out/cjs.js
### esbuild
```js
exports.a = 123;
let bar_ = require("xyz").b;
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region cjs.js
var require_cjs = /* @__PURE__ */ __commonJS({ "cjs.js"(exports) {
	exports.foo_ = 123;
	let bar_ = __require("xyz").bar_;
} });

//#endregion
export default require_cjs();

```
### diff
```diff
===================================================================
--- esbuild	/out/cjs.js
+++ rolldown	cjs.js
@@ -1,2 +1,7 @@
-exports.a = 123;
-let bar_ = require("xyz").b;
+var require_cjs = __commonJS({
+    "cjs.js"(exports) {
+        exports.foo_ = 123;
+        let bar_ = __require("xyz").bar_;
+    }
+});
+export default require_cjs();

```