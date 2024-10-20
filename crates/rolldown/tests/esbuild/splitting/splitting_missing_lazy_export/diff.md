# Reason
1. should convert missing property to `void 0`
# Diff
## /out/a.js
### esbuild
```js
import {
  foo
} from "./chunk-QVTGQSXT.js";

// a.js
console.log(foo());
```
### rolldown
```js
import { foo } from "./common.js";

//#region a.js
console.log(foo());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {foo} from "./chunk-QVTGQSXT.js";
+import {foo} from "./common.js";
 console.log(foo());

```
## /out/b.js
### esbuild
```js
import {
  bar
} from "./chunk-QVTGQSXT.js";

// b.js
console.log(bar());
```
### rolldown
```js
import { bar } from "./common.js";

//#region b.js
console.log(bar());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {bar} from "./chunk-QVTGQSXT.js";
+import {bar} from "./common.js";
 console.log(bar());

```
## /out/chunk-QVTGQSXT.js
### esbuild
```js
// empty.js
var empty_exports = {};

// common.js
function foo() {
  return [empty_exports, void 0];
}
function bar() {
  return [void 0];
}

export {
  foo,
  bar
};
```
### rolldown
```js


//#region empty.js
var require_empty = __commonJS({ "empty.js"() {} });

//#endregion
//#region common.js
var import_empty = __toESM(require_empty());
function foo() {
	return [import_empty, import_empty.missing];
}
function bar() {
	return [import_empty.missing];
}

//#endregion
export { bar, foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-QVTGQSXT.js
+++ rolldown	common.js
@@ -1,8 +1,11 @@
-var empty_exports = {};
+var require_empty = __commonJS({
+    "empty.js"() {}
+});
+var import_empty = __toESM(require_empty());
 function foo() {
-    return [empty_exports, void 0];
+    return [import_empty, import_empty.missing];
 }
 function bar() {
-    return [void 0];
+    return [import_empty.missing];
 }
-export {foo, bar};
+export {bar, foo};

```