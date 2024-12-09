# Reason
1. Wrong output
# Diff
## /out/entry-nope.js
### esbuild
```js
// empty.js
var require_empty = __commonJS({
  "empty.js"() {
  }
});

// empty.cjs
var require_empty2 = __commonJS({
  "empty.cjs"() {
  }
});

// entry-nope.js
var js = __toESM(require_empty());
var cjs = __toESM(require_empty2());
console.log(
  void 0,
  void 0,
  void 0
);
```
### rolldown
```js
import { __toESM, require_empty } from "./empty.js";

//#region entry-nope.js
var import_empty = __toESM(require_empty());
console.log(void 0, void 0, import_empty.nope);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry-nope.js
+++ rolldown	entry-nope.js
@@ -1,9 +1,3 @@
-var require_empty = __commonJS({
-    "empty.js"() {}
-});
-var require_empty2 = __commonJS({
-    "empty.cjs"() {}
-});
-var js = __toESM(require_empty());
-var cjs = __toESM(require_empty2());
-console.log(void 0, void 0, void 0);
+import {__toESM, require_empty} from "./empty.js";
+var import_empty = __toESM(require_empty());
+console.log(void 0, void 0, import_empty.nope);

```
## /out/entry-default.js
### esbuild
```js
// empty.js
var require_empty = __commonJS({
  "empty.js"() {
  }
});

// empty.cjs
var require_empty2 = __commonJS({
  "empty.cjs"() {
  }
});

// entry-default.js
var js = __toESM(require_empty());
var cjs = __toESM(require_empty2());
console.log(
  js.default,
  void 0,
  cjs.default
);
```
### rolldown
```js
import { __toESM, require_empty } from "./empty.js";

//#region entry-default.js
var import_empty = __toESM(require_empty());
console.log(void 0, void 0, import_empty.default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry-default.js
+++ rolldown	entry-default.js
@@ -1,9 +1,3 @@
-var require_empty = __commonJS({
-    "empty.js"() {}
-});
-var require_empty2 = __commonJS({
-    "empty.cjs"() {}
-});
-var js = __toESM(require_empty());
-var cjs = __toESM(require_empty2());
-console.log(js.default, void 0, cjs.default);
+import {__toESM, require_empty} from "./empty.js";
+var import_empty = __toESM(require_empty());
+console.log(void 0, void 0, import_empty.default);

```