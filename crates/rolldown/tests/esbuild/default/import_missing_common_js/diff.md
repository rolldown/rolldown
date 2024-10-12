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
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.x = 123;
} });

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -3,5 +3,4 @@
         exports.x = 123;
     }
 });
 var import_foo = __toESM(require_foo());
-console.log((0, import_foo.default)(import_foo.x, import_foo.y));

```