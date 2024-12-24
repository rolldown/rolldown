# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.foo = function() {
      return "foo";
    };
  }
});

// bar.js
var require_bar = __commonJS({
  "bar.js"(exports) {
    exports.bar = function() {
      return "bar";
    };
  }
});

// entry.js
var import_foo = __toESM(require_foo());
var import_bar = __toESM(require_bar());
console.log((0, import_foo.foo)(), (0, import_bar.bar)());
```
### rolldown
```js
import assert from "node:assert";


//#region foo.js
var import_foo;
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = function() {
		return "foo";
	};
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region bar.js
var import_bar;
var require_bar = __commonJS({ "bar.js"(exports) {
	exports.bar = function() {
		return "bar";
	};
	import_bar = __toESM(require_bar());
} });

//#endregion
//#region entry.js
require_foo();
require_bar();
assert.equal((0, import_foo.foo)(), "foo");
assert.equal((0, import_bar.bar)(), "bar");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,17 +1,21 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.foo = function () {
             return "foo";
         };
+        import_foo = __toESM(require_foo());
     }
 });
+var import_bar;
 var require_bar = __commonJS({
     "bar.js"(exports) {
         exports.bar = function () {
             return "bar";
         };
+        import_bar = __toESM(require_bar());
     }
 });
-var import_foo = __toESM(require_foo());
-var import_bar = __toESM(require_bar());
+require_foo();
+require_bar();
 console.log((0, import_foo.foo)(), (0, import_bar.bar)());

```