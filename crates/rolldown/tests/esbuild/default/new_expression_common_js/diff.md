# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports, module) {
    var Foo = class {
    };
    module.exports = { Foo };
  }
});

// entry.js
new (require_foo()).Foo();
```
### rolldown
```js


//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports, module) {
	class Foo {}
	module.exports = { Foo };
} });

//#endregion
//#region entry.js
new (require_foo()).Foo();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
 var require_foo = __commonJS({
     "foo.js"(exports, module) {
-        var Foo = class {};
+        class Foo {}
         module.exports = {
             Foo
         };
     }

```