# Diff
## /out.js
### esbuild
```js
// c.js
var c_exports = {};
var init_c = __esm({
  async "c.js"() {
    await 0;
  }
});

// b.js
var b_exports = {};
var init_b = __esm({
  async "b.js"() {
    await init_c();
  }
});

// a.js
var a_exports = {};
var init_a = __esm({
  async "a.js"() {
    await init_b();
  }
});

// entry.js
var entry_exports = {};
var init_entry = __esm({
  async "entry.js"() {
    init_a();
    init_b();
    init_c();
    init_entry();
    await 0;
  }
});
await init_entry();
```
### rolldown
```js

//#region entry.js
import("./a.js");
import("./b2.js");
import("./c2.js");
import("./entry.js");
await 0;

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,5 @@
-var c_exports = {};
-var init_c = __esm({
-    async "c.js"() {
-        await 0;
-    }
-});
-var b_exports = {};
-var init_b = __esm({
-    async "b.js"() {
-        await init_c();
-    }
-});
-var a_exports = {};
-var init_a = __esm({
-    async "a.js"() {
-        await init_b();
-    }
-});
-var entry_exports = {};
-var init_entry = __esm({
-    async "entry.js"() {
-        init_a();
-        init_b();
-        init_c();
-        init_entry();
-        await 0;
-    }
-});
-await init_entry();
+import("./a.js");
+import("./b2.js");
+import("./c2.js");
+import("./entry.js");
+await 0;

```