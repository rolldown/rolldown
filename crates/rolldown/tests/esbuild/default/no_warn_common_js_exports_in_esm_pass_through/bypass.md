# Reason
1. We don't have pass through mode, we just have same output as esbuild if 
in `bundle + esm`, https://hyrious.me/esbuild-repl/?version=0.23.0&b=e%00entry.js%00import+%7B+foo+%7D+from+%27bar%27%0Aexports.foo+%3D+foo%0Amodule.exports+%3D+foo%0A&b=%00%2Ffoo%2Ftest.js%00export+let+foo+%3D+123&b=%00%2Fbar%2Ftest.js%00export+let+bar+%3D+123&o=%7B%0A++treeShaking%3A+true%2C%0A++bundle%3A+true%2C%0A++format%3A+%22esm%22%2C%0A%0A++external%3A+%5B%27*%27%5D%0A+%0A%7D
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