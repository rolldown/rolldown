# Reason
1. not support import attributes
# Diff
## /out/foo-FYKHFNL2.copy
### esbuild
```js
{}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-FYKHFNL2.copy
+++ rolldown	
@@ -1,1 +0,0 @@
-{}

```
## /out/js-entry.js
### esbuild
```js
// foo.json
var foo_default = {};

// js-entry.js
import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
use(foo_default, copy, foo_default, void 0);
export {
  foo_default as default
};
```
### rolldown
```js
import { __toESM, foo_default, require_foo } from "./foo.js";

//#region js-entry.js
var import_foo = __toESM(require_foo());
use(foo_default, import_foo.default, foo_default, void 0);

//#endregion
export { foo_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/js-entry.js
+++ rolldown	js-entry.js
@@ -1,9 +1,8 @@
-// foo.json
-var foo_default = {};
+import { __toESM, foo_default, require_foo } from "./foo.js";
 
-// js-entry.js
-import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
-use(foo_default, copy, foo_default, void 0);
-export {
-  foo_default as default
-};
\ No newline at end of file
+//#region js-entry.js
+var import_foo = __toESM(require_foo());
+use(foo_default, import_foo.default, foo_default, void 0);
+
+//#endregion
+export { foo_default as default };
\ No newline at end of file

```
## /out/ts-entry.js
### esbuild
```js
// foo.json
var foo_default = {};

// ts-entry.ts
import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
use(foo_default, copy, foo_default, void 0);
export {
  foo_default as default
};
```
### rolldown
```js
import { __toESM, foo_default, require_foo } from "./foo.js";

//#region ts-entry.ts
var import_foo = __toESM(require_foo());
use(foo_default, import_foo.default, foo_default, void 0);

//#endregion
export { foo_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/ts-entry.js
+++ rolldown	ts-entry.js
@@ -1,9 +1,8 @@
-// foo.json
-var foo_default = {};
+import { __toESM, foo_default, require_foo } from "./foo.js";
 
-// ts-entry.ts
-import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
-use(foo_default, copy, foo_default, void 0);
-export {
-  foo_default as default
-};
\ No newline at end of file
+//#region ts-entry.ts
+var import_foo = __toESM(require_foo());
+use(foo_default, import_foo.default, foo_default, void 0);
+
+//#endregion
+export { foo_default as default };
\ No newline at end of file

```