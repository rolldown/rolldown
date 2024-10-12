# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.foo = 123;
  }
});

// entry.js
var entry_exports = {};
__export(entry_exports, {
  ns: () => ns
});
module.exports = __toCommonJS(entry_exports);
var ns = __toESM(require_foo());
```
### rolldown
```js


//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = 123;
} });

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());

//#endregion
export { import_foo as ns };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -2,10 +2,6 @@
     "foo.js"(exports) {
         exports.foo = 123;
     }
 });
-var entry_exports = {};
-__export(entry_exports, {
-    ns: () => ns
-});
-module.exports = __toCommonJS(entry_exports);
-var ns = __toESM(require_foo());
+var import_foo = __toESM(require_foo());
+export {import_foo as ns};

```