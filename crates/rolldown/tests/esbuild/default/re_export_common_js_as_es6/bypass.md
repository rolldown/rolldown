# Reason
1. naming style
# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.bar = 123;
  }
});

// entry.js
var import_foo = __toESM(require_foo());
var export_bar = import_foo.bar;
export {
  export_bar as bar
};
```
### rolldown
```js


//#region foo.js
var import_foo;
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.bar = 123;
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region entry.js
require_foo();

//#endregion
var bar = import_foo.bar;
export { bar };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,10 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.bar = 123;
+        import_foo = __toESM(require_foo());
     }
 });
-var import_foo = __toESM(require_foo());
-var export_bar = import_foo.bar;
-export {export_bar as bar};
+require_foo();
+var bar = import_foo.bar;
+export {bar};

```