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
import { default as assert } from "node:assert";


//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = function() {
		return "foo";
	};
} });

//#endregion
//#region bar.js
var require_bar = __commonJS({ "bar.js"(exports) {
	exports.bar = function() {
		return "bar";
	};
} });

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());
var import_bar = __toESM(require_bar());
assert((0, import_foo.foo)() === "foo" && (0, import_bar.bar)() === "bar");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -13,5 +13,5 @@
     }
 });
 var import_foo = __toESM(require_foo());
 var import_bar = __toESM(require_bar());
-console.log((0, import_foo.foo)(), (0, import_bar.bar)());
+assert((0, import_foo.foo)() === "foo" && (0, import_bar.bar)() === "bar");

```