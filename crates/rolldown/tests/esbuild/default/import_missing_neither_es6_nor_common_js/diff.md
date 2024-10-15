# Reason
1. rolldown extract common module
# Diff
## /out/named.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// named.js
var import_foo = __toESM(require_foo());
console.log((0, import_foo.default)(void 0, void 0));
```
### rolldown
```js
import { __toESM, require_foo } from "./foo2.js";

//#region named.js
var import_foo = __toESM(require_foo());
console.log((0, import_foo.default)(import_foo.x, import_foo.y));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/named.js
+++ rolldown	named.js
@@ -1,7 +1,3 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
+import {__toESM, require_foo} from "./foo2.js";
 var import_foo = __toESM(require_foo());
-console.log((0, import_foo.default)(void 0, void 0));
+console.log((0, import_foo.default)(import_foo.x, import_foo.y));

```
## /out/star.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// star.js
var ns = __toESM(require_foo());
console.log(ns.default(void 0, void 0));
```
### rolldown
```js
import { __toESM, require_foo } from "./foo2.js";

//#region star.js
var import_foo = __toESM(require_foo());
console.log(import_foo.default(import_foo.x, import_foo.y));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/star.js
+++ rolldown	star.js
@@ -1,7 +1,3 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-var ns = __toESM(require_foo());
-console.log(ns.default(void 0, void 0));
+import {__toESM, require_foo} from "./foo2.js";
+var import_foo = __toESM(require_foo());
+console.log(import_foo.default(import_foo.x, import_foo.y));

```
## /out/star-capture.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// star-capture.js
var ns = __toESM(require_foo());
console.log(ns);
```
### rolldown
```js
import { __toESM, require_foo } from "./foo2.js";

//#region star-capture.js
var import_foo = __toESM(require_foo());
console.log(import_foo);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/star-capture.js
+++ rolldown	star-capture.js
@@ -1,7 +1,3 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-var ns = __toESM(require_foo());
-console.log(ns);
+import {__toESM, require_foo} from "./foo2.js";
+var import_foo = __toESM(require_foo());
+console.log(import_foo);

```
## /out/bare.js
### esbuild
```js
// foo.js
console.log("no exports here");
```
### rolldown
```js
import { __toESM, require_foo } from "./foo2.js";

//#region bare.js
var import_foo = __toESM(require_foo());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/bare.js
+++ rolldown	bare.js
@@ -1,1 +1,2 @@
-console.log("no exports here");
+import {__toESM, require_foo} from "./foo2.js";
+var import_foo = __toESM(require_foo());

```
## /out/require.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// require.js
console.log(require_foo());
```
### rolldown
```js
import { require_foo } from "./foo2.js";

//#region require.js
console.log(require_foo());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/require.js
+++ rolldown	require.js
@@ -1,6 +1,2 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
+import {require_foo} from "./foo2.js";
 console.log(require_foo());

```
## /out/import.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// import.js
console.log(Promise.resolve().then(() => __toESM(require_foo())));
```
### rolldown
```js

//#region import.js
console.log(import("./foo.js"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/import.js
+++ rolldown	import.js
@@ -1,6 +1,1 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-console.log(Promise.resolve().then(() => __toESM(require_foo())));
+console.log(import("./foo.js"));

```