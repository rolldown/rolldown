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

//#region a.ts
console.log(b);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	a.js
@@ -1,8 +1,1 @@
-var require_b = __commonJS({
-    "b.ts"(exports, module) {
-        function foo() {}
-        module.exports = [123, foo];
-    }
-});
-var import_b = __toESM(require_b());
-console.log(import_b.default);
+console.log(b);

```