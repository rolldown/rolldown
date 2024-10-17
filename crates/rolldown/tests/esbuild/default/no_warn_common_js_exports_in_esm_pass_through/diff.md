# Reason
1. not align
# Diff
## /out/cjs-in-esm.js
### esbuild
```js
export let foo = 1;
exports.foo = 2;
module.exports = 3;
```
### rolldown
```js

//#region cjs-in-esm.js
let foo = 1;
exports.foo = 2;
module.exports = 3;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/cjs-in-esm.js
+++ rolldown	cjs-in-esm.js
@@ -1,3 +1,4 @@
-export let foo = 1;
+var foo = 1;
 exports.foo = 2;
 module.exports = 3;
+export {foo};

```
## /out/import-in-cjs.js
### esbuild
```js
import { foo } from "bar";
exports.foo = foo;
module.exports = foo;
```
### rolldown
```js
import { __commonJS } from "./chunk.js";
import { foo } from "bar";

//#region import-in-cjs.js
var require_import_in_cjs = __commonJS({ "import-in-cjs.js"(exports, module) {
	exports.foo = foo;
	module.exports = foo;
} });

//#endregion
export default require_import_in_cjs();

```
### diff
```diff
===================================================================
--- esbuild	/out/import-in-cjs.js
+++ rolldown	import-in-cjs.js
@@ -1,3 +1,9 @@
+import {__commonJS} from "./chunk.js";
 import {foo} from "bar";
-exports.foo = foo;
-module.exports = foo;
+var require_import_in_cjs = __commonJS({
+    "import-in-cjs.js"(exports, module) {
+        exports.foo = foo;
+        module.exports = foo;
+    }
+});
+export default require_import_in_cjs();

```
## /out/no-warnings-here.js
### esbuild
```js
console.log(module, exports);
```
### rolldown
```js
import { __commonJS } from "./chunk.js";

//#region no-warnings-here.js
var require_no_warnings_here = __commonJS({ "no-warnings-here.js"(exports, module) {
	console.log(module, exports);
} });

//#endregion
export default require_no_warnings_here();

```
### diff
```diff
===================================================================
--- esbuild	/out/no-warnings-here.js
+++ rolldown	no-warnings-here.js
@@ -1,1 +1,7 @@
-console.log(module, exports);
+import {__commonJS} from "./chunk.js";
+var require_no_warnings_here = __commonJS({
+    "no-warnings-here.js"(exports, module) {
+        console.log(module, exports);
+    }
+});
+export default require_no_warnings_here();

```