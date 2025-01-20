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
import { import_foo } from "./foo.js";

//#region entry.js
import("./foo2.js").then(({ default: { bar: b } }) => console.log(import_foo.bar, b));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,3 +1,2 @@
-import {__toESM, require_foo} from "./chunk-X3UWZZCR.js";
-var import_foo = __toESM(require_foo());
-import("./foo-BJYZ44Z3.js").then(({default: {bar: b}}) => console.log(import_foo.bar, b));
+import {import_foo} from "./foo.js";
+import("./foo2.js").then(({default: {bar: b}}) => console.log(import_foo.bar, b));

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


//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.bar = 123;
} });
var import_foo = __toESM(require_foo());

//#endregion
export { import_foo, require_foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/foo-BJYZ44Z3.js
+++ rolldown	foo.js
@@ -1,2 +1,7 @@
-import {require_foo} from "./chunk-X3UWZZCR.js";
-export default require_foo();
+var require_foo = __commonJS({
+    "foo.js"(exports) {
+        exports.bar = 123;
+    }
+});
+var import_foo = __toESM(require_foo());
+export {import_foo, require_foo};

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
import { require_foo } from "./foo.js";

export default require_foo();

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-X3UWZZCR.js
+++ rolldown	foo2.js
@@ -1,11 +1,3 @@
-// foo.js
-var require_foo = __commonJS({
-  "foo.js"(exports) {
-    exports.bar = 123;
-  }
-});
+import { require_foo } from "./foo.js";
 
-export {
-  __toESM,
-  require_foo
-};
\ No newline at end of file
+export default require_foo();

```