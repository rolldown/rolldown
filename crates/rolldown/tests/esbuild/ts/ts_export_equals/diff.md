# Diff
## /out.js
### esbuild
```js
// b.ts
var require_b = __commonJS({
  "b.ts"(exports, module) {
    function foo() {
    }
    module.exports = [123, foo];
  }
});

// a.ts
var import_b = __toESM(require_b());
console.log(import_b.default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var require_b = __commonJS({
-    "b.ts"(exports, module) {
-        function foo() {}
-        module.exports = [123, foo];
-    }
-});
-var import_b = __toESM(require_b());
-console.log(import_b.default);

```