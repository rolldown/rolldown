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
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = function() {
		return "foo";
	};
} });
var import_foo = __toESM(require_foo());
//#endregion

//#region bar.js
var require_bar = __commonJS({ "bar.js"(exports) {
	exports.bar = function() {
		return "bar";
	};
} });
var import_bar = __toESM(require_bar());
//#endregion

//#region entry.js
assert.equal((0, import_foo.foo)(), "foo");
assert.equal((0, import_bar.bar)(), "bar");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -4,14 +4,14 @@
             return "foo";
         };
     }
 });
+var import_foo = __toESM(require_foo());
 var require_bar = __commonJS({
     "bar.js"(exports) {
         exports.bar = function () {
             return "bar";
         };
     }
 });
-var import_foo = __toESM(require_foo());
 var import_bar = __toESM(require_bar());
 console.log((0, import_foo.foo)(), (0, import_bar.bar)());

```