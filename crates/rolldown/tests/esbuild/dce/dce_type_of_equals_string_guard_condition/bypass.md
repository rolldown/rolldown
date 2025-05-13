# Reason
1. different iife wrapper, esbuild use arrow function
# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  var keep_1 = typeof x !== "object" ? x : null;
  var keep_1 = typeof x != "object" ? x : null;
  var keep_1 = typeof x === "object" ? null : x;
  var keep_1 = typeof x == "object" ? null : x;
  var keep_1 = typeof x !== "object" && x;
  var keep_1 = typeof x != "object" && x;
  var keep_1 = typeof x === "object" || x;
  var keep_1 = typeof x == "object" || x;
  var keep_1 = "object" !== typeof x ? x : null;
  var keep_1 = "object" != typeof x ? x : null;
  var keep_1 = "object" === typeof x ? null : x;
  var keep_1 = "object" == typeof x ? null : x;
  var keep_1 = "object" !== typeof x && x;
  var keep_1 = "object" != typeof x && x;
  var keep_1 = "object" === typeof x || x;
  var keep_1 = "object" == typeof x || x;
  var keep_2 = typeof x !== "undefined" ? y : null;
  var keep_2 = typeof x != "undefined" ? y : null;
  var keep_2 = typeof x === "undefined" ? null : y;
  var keep_2 = typeof x == "undefined" ? null : y;
  var keep_2 = typeof x !== "undefined" && y;
  var keep_2 = typeof x != "undefined" && y;
  var keep_2 = typeof x === "undefined" || y;
  var keep_2 = typeof x == "undefined" || y;
  var keep_2 = "undefined" !== typeof x ? y : null;
  var keep_2 = "undefined" != typeof x ? y : null;
  var keep_2 = "undefined" === typeof x ? null : y;
  var keep_2 = "undefined" == typeof x ? null : y;
  var keep_2 = "undefined" !== typeof x && y;
  var keep_2 = "undefined" != typeof x && y;
  var keep_2 = "undefined" === typeof x || y;
  var keep_2 = "undefined" == typeof x || y;
  var keep_3 = typeof x !== "undefined" ? null : x;
  var keep_3 = typeof x != "undefined" ? null : x;
  var keep_3 = typeof x === "undefined" ? x : null;
  var keep_3 = typeof x == "undefined" ? x : null;
  var keep_3 = typeof x !== "undefined" || x;
  var keep_3 = typeof x != "undefined" || x;
  var keep_3 = typeof x === "undefined" && x;
  var keep_3 = typeof x == "undefined" && x;
  var keep_3 = "undefined" !== typeof x ? null : x;
  var keep_3 = "undefined" != typeof x ? null : x;
  var keep_3 = "undefined" === typeof x ? x : null;
  var keep_3 = "undefined" == typeof x ? x : null;
  var keep_3 = "undefined" !== typeof x || x;
  var keep_3 = "undefined" != typeof x || x;
  var keep_3 = "undefined" === typeof x && x;
  var keep_3 = "undefined" == typeof x && x;
})();
```
### rolldown
```js
(function() {


//#region entry.js
// Everything here should be kept as live code because it has side effects
var keep_1 = typeof x !== "object" ? x : null;
var keep_1 = typeof x != "object" ? x : null;
var keep_1 = typeof x === "object" ? null : x;
var keep_1 = typeof x == "object" ? null : x;
var keep_1 = typeof x !== "object" && x;
var keep_1 = typeof x != "object" && x;
var keep_1 = typeof x === "object" || x;
var keep_1 = typeof x == "object" || x;
var keep_1 = "object" !== typeof x ? x : null;
var keep_1 = "object" != typeof x ? x : null;
var keep_1 = "object" === typeof x ? null : x;
var keep_1 = "object" == typeof x ? null : x;
var keep_1 = "object" !== typeof x && x;
var keep_1 = "object" != typeof x && x;
var keep_1 = "object" === typeof x || x;
var keep_1 = "object" == typeof x || x;
// Everything here should be kept as live code because it has side effects
var keep_2 = typeof x !== "undefined" ? y : null;
var keep_2 = typeof x != "undefined" ? y : null;
var keep_2 = typeof x === "undefined" ? null : y;
var keep_2 = typeof x == "undefined" ? null : y;
var keep_2 = typeof x !== "undefined" && y;
var keep_2 = typeof x != "undefined" && y;
var keep_2 = typeof x === "undefined" || y;
var keep_2 = typeof x == "undefined" || y;
var keep_2 = "undefined" !== typeof x ? y : null;
var keep_2 = "undefined" != typeof x ? y : null;
var keep_2 = "undefined" === typeof x ? null : y;
var keep_2 = "undefined" == typeof x ? null : y;
var keep_2 = "undefined" !== typeof x && y;
var keep_2 = "undefined" != typeof x && y;
var keep_2 = "undefined" === typeof x || y;
var keep_2 = "undefined" == typeof x || y;
// Everything here should be kept as live code because it has side effects
var keep_3 = typeof x !== "undefined" ? null : x;
var keep_3 = typeof x != "undefined" ? null : x;
var keep_3 = typeof x === "undefined" ? x : null;
var keep_3 = typeof x == "undefined" ? x : null;
var keep_3 = typeof x !== "undefined" || x;
var keep_3 = typeof x != "undefined" || x;
var keep_3 = typeof x === "undefined" && x;
var keep_3 = typeof x == "undefined" && x;
var keep_3 = "undefined" !== typeof x ? null : x;
var keep_3 = "undefined" != typeof x ? null : x;
var keep_3 = "undefined" === typeof x ? x : null;
var keep_3 = "undefined" == typeof x ? x : null;
var keep_3 = "undefined" !== typeof x || x;
var keep_3 = "undefined" != typeof x || x;
var keep_3 = "undefined" === typeof x && x;
var keep_3 = "undefined" == typeof x && x;

//#endregion
})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-(() => {
+(function () {
     var keep_1 = typeof x !== "object" ? x : null;
     var keep_1 = typeof x != "object" ? x : null;
     var keep_1 = typeof x === "object" ? null : x;
     var keep_1 = typeof x == "object" ? null : x;

```