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
import default$1, { init_foo, x, y } from "./foo.js";

//#region named.js
init_foo();
console.log(default$1(x, y));
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
-var import_foo = __toESM(require_foo());
-console.log((0, import_foo.default)(void 0, void 0));
+import default$1, {init_foo, x, y} from "./foo.js";
+init_foo();
+console.log(default$1(x, y));

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
import { init_foo } from "./foo.js";

//#region star.js
init_foo();
console.log((void 0)(void 0, void 0));
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
+import {init_foo} from "./foo.js";
+init_foo();
+console.log((void 0)(void 0, void 0));

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
import { foo_exports, init_foo } from "./foo.js";

//#region star-capture.js
init_foo();
console.log(foo_exports);
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
+import {foo_exports, init_foo} from "./foo.js";
+init_foo();
+console.log(foo_exports);

```
## /out/bare.js
### esbuild
```js
// foo.js
console.log("no exports here");
```
### rolldown
```js
import { init_foo } from "./foo.js";

//#region bare.js
init_foo();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/bare.js
+++ rolldown	bare.js
@@ -1,1 +1,2 @@
-console.log("no exports here");
+import {init_foo} from "./foo.js";
+init_foo();

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
import { __toCommonJS, foo_exports, init_foo } from "./foo.js";

//#region require.js
console.log((init_foo(), __toCommonJS(foo_exports)));
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
-console.log(require_foo());
+import {__toCommonJS, foo_exports, init_foo} from "./foo.js";
+console.log((init_foo(), __toCommonJS(foo_exports)));

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
console.log(import("./foo2.js"));
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
+console.log(import("./foo2.js"));

```