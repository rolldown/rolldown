# Diff
## entry.js
### esbuild
```js
// b.empty
var require_b = __commonJS({
  "b.empty"() {
  }
});

// c.empty
var require_c = __commonJS({
  "c.empty"() {
  }
});

// entry.js
var ns = __toESM(require_b());
var import_c = __toESM(require_c());
console.log(ns, import_c.default, void 0);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region b.empty
var b_exports = {};

//#endregion
//#region c.empty
var default$1 = void 0;

//#endregion
//#region d.empty
var named = void 0;

//#endregion
//#region entry.js
console.log(b_exports, default$1, named);
assert.deepEqual(b_exports, {});
assert.equal(default$1, undefined);
assert.equal(named, undefined);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	entry.js
+++ rolldown	entry.js
@@ -1,9 +1,5 @@
-var require_b = __commonJS({
-    "b.empty"() {}
-});
-var require_c = __commonJS({
-    "c.empty"() {}
-});
-var ns = __toESM(require_b());
-var import_c = __toESM(require_c());
-console.log(ns, import_c.default, void 0);
+var b_exports = {};
+var default$1 = void 0;
+var named = void 0;
+console.log(b_exports, default$1, named);
+console.log(b_exports, default$1, named);

```