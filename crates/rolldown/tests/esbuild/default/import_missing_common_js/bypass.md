# Reason
1. It is safe to remove `default` property access, if there is no `module.exports` used in cjs module
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
console.log(import_foo(import_foo.x, import_foo.y));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -3,5 +3,5 @@
         exports.x = 123;
     }
 });
 var import_foo = __toESM(require_foo());
-console.log((0, import_foo.default)(import_foo.x, import_foo.y));
+console.log(import_foo(import_foo.x, import_foo.y));

```
