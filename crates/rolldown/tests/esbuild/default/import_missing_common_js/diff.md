# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.x = 123;
  }
});

// entry.js
var import_foo = __toESM(require_foo());
console.log((0, import_foo.default)(import_foo.x, import_foo.y));
```
### rolldown
```js


//#region foo.js
var import_foo;
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.x = 123;
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region entry.js
require_foo();
console.log((0, import_foo.default)(import_foo.x, import_foo.y));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,9 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.x = 123;
+        import_foo = __toESM(require_foo());
     }
 });
-var import_foo = __toESM(require_foo());
+require_foo();
 console.log((0, import_foo.default)(import_foo.x, import_foo.y));

```