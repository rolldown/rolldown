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
typeof x !== "object" ? x : null;
typeof x != "object" ? x : null;
typeof x === "object" ? null : x;
typeof x == "object" ? null : x;
typeof x !== "object" && x;
typeof x != "object" && x;
typeof x === "object" || x;
typeof x == "object" || x;
"object" !== typeof x ? x : null;
"object" != typeof x ? x : null;
"object" === typeof x ? null : x;
"object" == typeof x ? null : x;
"object" !== typeof x && x;
"object" != typeof x && x;
"object" === typeof x || x;
"object" == typeof x || x;
typeof x !== "undefined" ? y : null;
typeof x != "undefined" ? y : null;
typeof x === "undefined" ? null : y;
typeof x == "undefined" ? null : y;
typeof x !== "undefined" && y;
typeof x != "undefined" && y;
typeof x === "undefined" || y;
typeof x == "undefined" || y;
"undefined" !== typeof x ? y : null;
"undefined" != typeof x ? y : null;
"undefined" === typeof x ? null : y;
"undefined" == typeof x ? null : y;
"undefined" !== typeof x && y;
"undefined" != typeof x && y;
"undefined" === typeof x || y;
"undefined" == typeof x || y;
typeof x !== "undefined" ? null : x;
typeof x != "undefined" ? null : x;
typeof x === "undefined" ? x : null;
typeof x == "undefined" ? x : null;
typeof x !== "undefined" || x;
typeof x != "undefined" || x;
typeof x === "undefined" && x;
typeof x == "undefined" && x;
"undefined" !== typeof x ? null : x;
"undefined" != typeof x ? null : x;
"undefined" === typeof x ? x : null;
"undefined" == typeof x ? x : null;
"undefined" !== typeof x || x;
"undefined" != typeof x || x;
"undefined" === typeof x && x;
"undefined" == typeof x && x;

//#endregion
})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,50 +1,50 @@
-(() => {
-    var keep_1 = typeof x !== "object" ? x : null;
-    var keep_1 = typeof x != "object" ? x : null;
-    var keep_1 = typeof x === "object" ? null : x;
-    var keep_1 = typeof x == "object" ? null : x;
-    var keep_1 = typeof x !== "object" && x;
-    var keep_1 = typeof x != "object" && x;
-    var keep_1 = typeof x === "object" || x;
-    var keep_1 = typeof x == "object" || x;
-    var keep_1 = "object" !== typeof x ? x : null;
-    var keep_1 = "object" != typeof x ? x : null;
-    var keep_1 = "object" === typeof x ? null : x;
-    var keep_1 = "object" == typeof x ? null : x;
-    var keep_1 = "object" !== typeof x && x;
-    var keep_1 = "object" != typeof x && x;
-    var keep_1 = "object" === typeof x || x;
-    var keep_1 = "object" == typeof x || x;
-    var keep_2 = typeof x !== "undefined" ? y : null;
-    var keep_2 = typeof x != "undefined" ? y : null;
-    var keep_2 = typeof x === "undefined" ? null : y;
-    var keep_2 = typeof x == "undefined" ? null : y;
-    var keep_2 = typeof x !== "undefined" && y;
-    var keep_2 = typeof x != "undefined" && y;
-    var keep_2 = typeof x === "undefined" || y;
-    var keep_2 = typeof x == "undefined" || y;
-    var keep_2 = "undefined" !== typeof x ? y : null;
-    var keep_2 = "undefined" != typeof x ? y : null;
-    var keep_2 = "undefined" === typeof x ? null : y;
-    var keep_2 = "undefined" == typeof x ? null : y;
-    var keep_2 = "undefined" !== typeof x && y;
-    var keep_2 = "undefined" != typeof x && y;
-    var keep_2 = "undefined" === typeof x || y;
-    var keep_2 = "undefined" == typeof x || y;
-    var keep_3 = typeof x !== "undefined" ? null : x;
-    var keep_3 = typeof x != "undefined" ? null : x;
-    var keep_3 = typeof x === "undefined" ? x : null;
-    var keep_3 = typeof x == "undefined" ? x : null;
-    var keep_3 = typeof x !== "undefined" || x;
-    var keep_3 = typeof x != "undefined" || x;
-    var keep_3 = typeof x === "undefined" && x;
-    var keep_3 = typeof x == "undefined" && x;
-    var keep_3 = "undefined" !== typeof x ? null : x;
-    var keep_3 = "undefined" != typeof x ? null : x;
-    var keep_3 = "undefined" === typeof x ? x : null;
-    var keep_3 = "undefined" == typeof x ? x : null;
-    var keep_3 = "undefined" !== typeof x || x;
-    var keep_3 = "undefined" != typeof x || x;
-    var keep_3 = "undefined" === typeof x && x;
-    var keep_3 = "undefined" == typeof x && x;
+(function () {
+    typeof x !== "object" ? x : null;
+    typeof x != "object" ? x : null;
+    typeof x === "object" ? null : x;
+    typeof x == "object" ? null : x;
+    typeof x !== "object" && x;
+    typeof x != "object" && x;
+    typeof x === "object" || x;
+    typeof x == "object" || x;
+    "object" !== typeof x ? x : null;
+    "object" != typeof x ? x : null;
+    "object" === typeof x ? null : x;
+    "object" == typeof x ? null : x;
+    "object" !== typeof x && x;
+    "object" != typeof x && x;
+    "object" === typeof x || x;
+    "object" == typeof x || x;
+    typeof x !== "undefined" ? y : null;
+    typeof x != "undefined" ? y : null;
+    typeof x === "undefined" ? null : y;
+    typeof x == "undefined" ? null : y;
+    typeof x !== "undefined" && y;
+    typeof x != "undefined" && y;
+    typeof x === "undefined" || y;
+    typeof x == "undefined" || y;
+    "undefined" !== typeof x ? y : null;
+    "undefined" != typeof x ? y : null;
+    "undefined" === typeof x ? null : y;
+    "undefined" == typeof x ? null : y;
+    "undefined" !== typeof x && y;
+    "undefined" != typeof x && y;
+    "undefined" === typeof x || y;
+    "undefined" == typeof x || y;
+    typeof x !== "undefined" ? null : x;
+    typeof x != "undefined" ? null : x;
+    typeof x === "undefined" ? x : null;
+    typeof x == "undefined" ? x : null;
+    typeof x !== "undefined" || x;
+    typeof x != "undefined" || x;
+    typeof x === "undefined" && x;
+    typeof x == "undefined" && x;
+    "undefined" !== typeof x ? null : x;
+    "undefined" != typeof x ? null : x;
+    "undefined" === typeof x ? x : null;
+    "undefined" == typeof x ? x : null;
+    "undefined" !== typeof x || x;
+    "undefined" != typeof x || x;
+    "undefined" === typeof x && x;
+    "undefined" == typeof x && x;
 })();

```