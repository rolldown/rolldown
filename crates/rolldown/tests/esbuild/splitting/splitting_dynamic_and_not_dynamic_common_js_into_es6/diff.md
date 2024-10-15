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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {__toESM, require_foo} from "./chunk-X3UWZZCR.js";
-var import_foo = __toESM(require_foo());
-import("./foo-BJYZ44Z3.js").then(({default: {bar: b}}) => console.log(import_foo.bar, b));

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

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-BJYZ44Z3.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {require_foo} from "./chunk-X3UWZZCR.js";
-export default require_foo();

```
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

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-BJYZ44Z3.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {require_foo} from "./chunk-X3UWZZCR.js";
-export default require_foo();

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

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-X3UWZZCR.js
+++ rolldown	
@@ -1,11 +0,0 @@
-// foo.js
-var require_foo = __commonJS({
-  "foo.js"(exports) {
-    exports.bar = 123;
-  }
-});
-
-export {
-  __toESM,
-  require_foo
-};
\ No newline at end of file

```