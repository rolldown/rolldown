# Reason
1. We don't consider `require($expr)` as a record
# Diff
## /out.js
### esbuild
```js
// b.js
var require_b = __commonJS({
  "b.js"(exports) {
    exports.foo = 213;
  }
});

// a.js
x ? __require("a") : y ? require_b() : __require("c");
x ? y ? __require("a") : require_b() : __require(c);
```
### rolldown
```js


//#region b.js
var require_b = __commonJS({ "b.js"(exports) {
	exports.foo = 213;
} });

//#endregion
//#region a.js
x ? __require("a") : y ? require_b() : __require("c");
x ? y ? __require("a") : require_b() : require(c);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	a.js
@@ -3,5 +3,5 @@
         exports.foo = 213;
     }
 });
 x ? __require("a") : y ? require_b() : __require("c");
-x ? y ? __require("a") : require_b() : __require(c);
+x ? y ? __require("a") : require_b() : require(c);

```
