# Diff
## /out/cjs-in-esm.js
### esbuild
```js
var cjs_in_esm_exports = {};
__export(cjs_in_esm_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(cjs_in_esm_exports);
let foo = 1;
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
@@ -1,8 +1,4 @@
-var cjs_in_esm_exports = {};
-__export(cjs_in_esm_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(cjs_in_esm_exports);
-let foo = 1;
+var foo = 1;
 exports.foo = 2;
 module.exports = 3;
+export {foo};

```
## /out/cjs-in-esm2.js
### esbuild
```js
var cjs_in_esm2_exports = {};
__export(cjs_in_esm2_exports, {
  foo: () => foo
});
module.exports = __toCommonJS(cjs_in_esm2_exports);
let foo = 1;
module.exports.bar = 3;
```
### rolldown
```js

//#region cjs-in-esm2.js
let foo = 1;
module.exports.bar = 3;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/cjs-in-esm2.js
+++ rolldown	cjs-in-esm2.js
@@ -1,7 +1,3 @@
-var cjs_in_esm2_exports = {};
-__export(cjs_in_esm2_exports, {
-    foo: () => foo
-});
-module.exports = __toCommonJS(cjs_in_esm2_exports);
-let foo = 1;
+var foo = 1;
 module.exports.bar = 3;
+export {foo};

```
## /out/import-in-cjs.js
### esbuild
```js
var import_bar = require("bar");
exports.foo = import_bar.foo;
module.exports = import_bar.foo;
module.exports.bar = import_bar.foo;
```
### rolldown
```js
import { __commonJS } from "./chunk.js";
import { foo } from "bar";

//#region import-in-cjs.js
var require_import_in_cjs = __commonJS({ "import-in-cjs.js"(exports, module) {
	exports.foo = foo;
	module.exports = foo;
	module.exports.bar = foo;
} });

//#endregion
export default require_import_in_cjs();

```
### diff
```diff
===================================================================
--- esbuild	/out/import-in-cjs.js
+++ rolldown	import-in-cjs.js
@@ -1,4 +1,10 @@
-var import_bar = require("bar");
-exports.foo = import_bar.foo;
-module.exports = import_bar.foo;
-module.exports.bar = import_bar.foo;
+import {__commonJS} from "./chunk.js";
+import {foo} from "bar";
+var require_import_in_cjs = __commonJS({
+    "import-in-cjs.js"(exports, module) {
+        exports.foo = foo;
+        module.exports = foo;
+        module.exports.bar = foo;
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