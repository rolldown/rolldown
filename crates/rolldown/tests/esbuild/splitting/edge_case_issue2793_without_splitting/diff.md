# Reason
1. dynamic import with cycle reference
# Diff
## /out/index.js
### esbuild
```js
// src/a.js
var A;
var init_a = __esm({
  "src/a.js"() {
    A = 42;
  }
});

// src/b.js
var B;
var init_b = __esm({
  "src/b.js"() {
    B = async () => (await Promise.resolve().then(() => (init_src(), src_exports))).A;
  }
});

// src/index.js
var src_exports = {};
__export(src_exports, {
  A: () => A,
  B: () => B
});
var init_src = __esm({
  "src/index.js"() {
    init_a();
    init_b();
  }
});
init_src();
export {
  A,
  B
};
```
### rolldown
```js
//#region a.js
const A = 42;

//#endregion
//#region b.js
const B = async () => (await import("./index.js")).A;

//#endregion
export { A, B };
```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	index.js
@@ -1,25 +1,3 @@
-var A;
-var init_a = __esm({
-    "src/a.js"() {
-        A = 42;
-    }
-});
-var B;
-var init_b = __esm({
-    "src/b.js"() {
-        B = async () => (await Promise.resolve().then(() => (init_src(), src_exports))).A;
-    }
-});
-var src_exports = {};
-__export(src_exports, {
-    A: () => A,
-    B: () => B
-});
-var init_src = __esm({
-    "src/index.js"() {
-        init_a();
-        init_b();
-    }
-});
-init_src();
+var A = 42;
+var B = async () => (await import("./index.js")).A;
 export {A, B};

```