# Reason
1. different chunk naming style
# Diff
## /out/entry.js
### esbuild
```js
import {
  __toESM,
  require_foo
} from "./chunk-X3UWZZCR.js";

// entry.js
var import_foo = __toESM(require_foo());
import("./foo-BJYZ44Z3.js").then(({ default: { bar: b } }) => console.log(import_foo.bar, b));
```
### rolldown
```js
import { __toESM, require_foo } from "./foo2.js";

//#region entry.js
var import_foo = __toESM(require_foo());
import("./foo.js").then(({ default: { bar: b } }) => console.log(import_foo.bar, b));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-import {__toESM, require_foo} from "./chunk-X3UWZZCR.js";
+import {__toESM, require_foo} from "./foo2.js";
 var import_foo = __toESM(require_foo());
-import("./foo-BJYZ44Z3.js").then(({default: {bar: b}}) => console.log(import_foo.bar, b));
+import("./foo.js").then(({default: {bar: b}}) => console.log(import_foo.bar, b));

```
## /out/foo-BJYZ44Z3.js
### esbuild
```js
import {
  require_foo
} from "./chunk-X3UWZZCR.js";
export default require_foo();
```
### rolldown
```js
import { require_foo } from "./foo2.js";

export default require_foo();

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-BJYZ44Z3.js
+++ rolldown	foo.js
@@ -1,2 +1,2 @@
-import {require_foo} from "./chunk-X3UWZZCR.js";
+import {require_foo} from "./foo2.js";
 export default require_foo();

```
## /out/chunk-X3UWZZCR.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.bar = 123;
  }
});

export {
  __toESM,
  require_foo
};
```
### rolldown
```js


//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.bar = 123;
} });

//#endregion
export { __toESM, require_foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-X3UWZZCR.js
+++ rolldown	foo2.js
@@ -1,11 +1,9 @@
-// foo.js
-var require_foo = __commonJS({
-  "foo.js"(exports) {
-    exports.bar = 123;
-  }
-});
 
-export {
-  __toESM,
-  require_foo
-};
\ No newline at end of file
+
+//#region foo.js
+var require_foo = __commonJS({ "foo.js"(exports) {
+	exports.bar = 123;
+} });
+
+//#endregion
+export { __toESM, require_foo };
\ No newline at end of file

```