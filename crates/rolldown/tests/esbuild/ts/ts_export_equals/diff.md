# Reason
1. require `oxc-transformer` support `module type`
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


//#region b.ts
var import_b;
var require_b = __commonJS({ "b.ts"(exports, module) {
	module.exports = [123, foo];
	function foo() {}
	import_b = __toESM(require_b());
} });

//#endregion
//#region a.ts
require_b();
console.log(import_b.default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	a.js
@@ -1,8 +1,10 @@
+var import_b;
 var require_b = __commonJS({
     "b.ts"(exports, module) {
-        function foo() {}
         module.exports = [123, foo];
+        function foo() {}
+        import_b = __toESM(require_b());
     }
 });
-var import_b = __toESM(require_b());
+require_b();
 console.log(import_b.default);

```