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

```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	
@@ -1,25 +0,0 @@
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
-export {A, B};

```
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

```
### diff
```diff
===================================================================
--- esbuild	/out/index.js
+++ rolldown	
@@ -1,25 +0,0 @@
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
-export {A, B};

```