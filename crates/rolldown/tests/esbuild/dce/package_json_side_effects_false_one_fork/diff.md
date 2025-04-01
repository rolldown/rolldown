# Reason
1. different async module impl
# Diff
## /out.js
### esbuild
```js
// Users/user/project/node_modules/c/index.js
var foo;
var init_c = __esm({
  "Users/user/project/node_modules/c/index.js"() {
    foo = "foo";
  }
});

// Users/user/project/node_modules/d/index.js
var init_d = __esm({
  "Users/user/project/node_modules/d/index.js"() {
  }
});

// Users/user/project/node_modules/b/index.js
var init_b = __esm({
  "Users/user/project/node_modules/b/index.js"() {
    init_c();
    init_d();
  }
});

// Users/user/project/node_modules/a/index.js
var a_exports = {};
__export(a_exports, {
  foo: () => foo
});
var init_a = __esm({
  "Users/user/project/node_modules/a/index.js"() {
    init_b();
  }
});

// Users/user/project/src/entry.js
Promise.resolve().then(() => (init_a(), a_exports)).then((x) => assert(x.foo === "foo"));
```
### rolldown
```js

//#region src/entry.js
import("./a.js").then((x) => assert(x.foo === "foo"));
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	src_entry.js
@@ -1,25 +1,1 @@
-var foo;
-var init_c = __esm({
-    "Users/user/project/node_modules/c/index.js"() {
-        foo = "foo";
-    }
-});
-var init_d = __esm({
-    "Users/user/project/node_modules/d/index.js"() {}
-});
-var init_b = __esm({
-    "Users/user/project/node_modules/b/index.js"() {
-        init_c();
-        init_d();
-    }
-});
-var a_exports = {};
-__export(a_exports, {
-    foo: () => foo
-});
-var init_a = __esm({
-    "Users/user/project/node_modules/a/index.js"() {
-        init_b();
-    }
-});
-Promise.resolve().then(() => (init_a(), a_exports)).then(x => assert(x.foo === "foo"));
+import("./a.js").then(x => assert(x.foo === "foo"));

```