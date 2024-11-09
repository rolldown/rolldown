# Reason
1. different hash algorithm
# Diff
## /out.js
### esbuild
```js
// x.txt
var require_x = __commonJS({
  "x.txt"(exports, module) {
    module.exports = "./x-LSAMBFUD.txt";
  }
});

// y.txt
var y_default = "./y-YE5AYNFB.txt";

// entry.js
var x_url = require_x();
console.log(x_url, y_default);
```
### rolldown
```js


//#region y.txt
var y_default = "assets/y-319pXHJ8.txt";

//#endregion
//#region x.txt
var require_x = __commonJS({ "x.txt"(exports, module) {
	module.exports = "assets/x-10ZlLk_R.txt";
} });

//#endregion
//#region entry.js
const x_url = require_x();
console.log(x_url, y_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
+var y_default = "assets/y-319pXHJ8.txt";
 var require_x = __commonJS({
     "x.txt"(exports, module) {
-        module.exports = "./x-LSAMBFUD.txt";
+        module.exports = "assets/x-10ZlLk_R.txt";
     }
 });
-var y_default = "./y-YE5AYNFB.txt";
 var x_url = require_x();
 console.log(x_url, y_default);

```