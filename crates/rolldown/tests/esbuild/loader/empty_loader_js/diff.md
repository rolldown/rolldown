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

```
### diff
```diff
===================================================================
--- esbuild	entry.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_b = __commonJS({
-    "b.empty"() {}
-});
-var require_c = __commonJS({
-    "c.empty"() {}
-});
-var ns = __toESM(require_b());
-var import_c = __toESM(require_c());
-console.log(ns, import_c.default, void 0);

```