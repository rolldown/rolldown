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
    B = async () => (await Promise.resolve().then(() => (init_index(), index_exports))).A;
  }
});

// src/index.js
var index_exports = {};
__export(index_exports, {
  A: () => A,
  B: () => B
});
var init_index = __esm({
  "src/index.js"() {
    init_a();
    init_b();
  }
});
init_index();
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
-        B = async () => (await Promise.resolve().then(() => (init_index(), index_exports))).A;
-    }
-});
-var index_exports = {};
-__export(index_exports, {
-    A: () => A,
-    B: () => B
-});
-var init_index = __esm({
-    "src/index.js"() {
-        init_a();
-        init_b();
-    }
-});
-init_index();
+var A = 42;
+var B = async () => (await import("./index.js")).A;
 export {A, B};

```