# Reason
1. different iife wrapper, esbuild use arrow function
# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  var keep_1 = typeof x <= "u" ? y : null;
  var keep_1 = typeof x < "u" ? y : null;
  var keep_1 = typeof x >= "u" ? null : y;
  var keep_1 = typeof x > "u" ? null : y;
  var keep_1 = typeof x <= "u" && y;
  var keep_1 = typeof x < "u" && y;
  var keep_1 = typeof x >= "u" || y;
  var keep_1 = typeof x > "u" || y;
  var keep_1 = "u" >= typeof x ? y : null;
  var keep_1 = "u" > typeof x ? y : null;
  var keep_1 = "u" <= typeof x ? null : y;
  var keep_1 = "u" < typeof x ? null : y;
  var keep_1 = "u" >= typeof x && y;
  var keep_1 = "u" > typeof x && y;
  var keep_1 = "u" <= typeof x || y;
  var keep_1 = "u" < typeof x || y;
  var keep_2 = typeof x <= "u" ? null : x;
  var keep_2 = typeof x < "u" ? null : x;
  var keep_2 = typeof x >= "u" ? x : null;
  var keep_2 = typeof x > "u" ? x : null;
  var keep_2 = typeof x <= "u" || x;
  var keep_2 = typeof x < "u" || x;
  var keep_2 = typeof x >= "u" && x;
  var keep_2 = typeof x > "u" && x;
  var keep_2 = "u" >= typeof x ? null : x;
  var keep_2 = "u" > typeof x ? null : x;
  var keep_2 = "u" <= typeof x ? x : null;
  var keep_2 = "u" < typeof x ? x : null;
  var keep_2 = "u" >= typeof x || x;
  var keep_2 = "u" > typeof x || x;
  var keep_2 = "u" <= typeof x && x;
  var keep_2 = "u" < typeof x && x;
})();
```
### rolldown
```js
(function() {


//#region entry.js
typeof x <= "u" ? y : null;
typeof x < "u" ? y : null;
typeof x >= "u" ? null : y;
typeof x > "u" ? null : y;
typeof x <= "u" && y;
typeof x < "u" && y;
typeof x >= "u" || y;
typeof x > "u" || y;
"u" >= typeof x ? y : null;
"u" > typeof x ? y : null;
"u" <= typeof x ? null : y;
"u" < typeof x ? null : y;
"u" >= typeof x && y;
"u" > typeof x && y;
"u" <= typeof x || y;
"u" < typeof x || y;
typeof x <= "u" ? null : x;
typeof x < "u" ? null : x;
typeof x >= "u" ? x : null;
typeof x > "u" ? x : null;
typeof x <= "u" || x;
typeof x < "u" || x;
typeof x >= "u" && x;
typeof x > "u" && x;
"u" >= typeof x ? null : x;
"u" > typeof x ? null : x;
"u" <= typeof x ? x : null;
"u" < typeof x ? x : null;
"u" >= typeof x || x;
"u" > typeof x || x;
"u" <= typeof x && x;
"u" < typeof x && x;

//#endregion
})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,34 +1,34 @@
-(() => {
-    var keep_1 = typeof x <= "u" ? y : null;
-    var keep_1 = typeof x < "u" ? y : null;
-    var keep_1 = typeof x >= "u" ? null : y;
-    var keep_1 = typeof x > "u" ? null : y;
-    var keep_1 = typeof x <= "u" && y;
-    var keep_1 = typeof x < "u" && y;
-    var keep_1 = typeof x >= "u" || y;
-    var keep_1 = typeof x > "u" || y;
-    var keep_1 = "u" >= typeof x ? y : null;
-    var keep_1 = "u" > typeof x ? y : null;
-    var keep_1 = "u" <= typeof x ? null : y;
-    var keep_1 = "u" < typeof x ? null : y;
-    var keep_1 = "u" >= typeof x && y;
-    var keep_1 = "u" > typeof x && y;
-    var keep_1 = "u" <= typeof x || y;
-    var keep_1 = "u" < typeof x || y;
-    var keep_2 = typeof x <= "u" ? null : x;
-    var keep_2 = typeof x < "u" ? null : x;
-    var keep_2 = typeof x >= "u" ? x : null;
-    var keep_2 = typeof x > "u" ? x : null;
-    var keep_2 = typeof x <= "u" || x;
-    var keep_2 = typeof x < "u" || x;
-    var keep_2 = typeof x >= "u" && x;
-    var keep_2 = typeof x > "u" && x;
-    var keep_2 = "u" >= typeof x ? null : x;
-    var keep_2 = "u" > typeof x ? null : x;
-    var keep_2 = "u" <= typeof x ? x : null;
-    var keep_2 = "u" < typeof x ? x : null;
-    var keep_2 = "u" >= typeof x || x;
-    var keep_2 = "u" > typeof x || x;
-    var keep_2 = "u" <= typeof x && x;
-    var keep_2 = "u" < typeof x && x;
+(function () {
+    typeof x <= "u" ? y : null;
+    typeof x < "u" ? y : null;
+    typeof x >= "u" ? null : y;
+    typeof x > "u" ? null : y;
+    typeof x <= "u" && y;
+    typeof x < "u" && y;
+    typeof x >= "u" || y;
+    typeof x > "u" || y;
+    "u" >= typeof x ? y : null;
+    "u" > typeof x ? y : null;
+    "u" <= typeof x ? null : y;
+    "u" < typeof x ? null : y;
+    "u" >= typeof x && y;
+    "u" > typeof x && y;
+    "u" <= typeof x || y;
+    "u" < typeof x || y;
+    typeof x <= "u" ? null : x;
+    typeof x < "u" ? null : x;
+    typeof x >= "u" ? x : null;
+    typeof x > "u" ? x : null;
+    typeof x <= "u" || x;
+    typeof x < "u" || x;
+    typeof x >= "u" && x;
+    typeof x > "u" && x;
+    "u" >= typeof x ? null : x;
+    "u" > typeof x ? null : x;
+    "u" <= typeof x ? x : null;
+    "u" < typeof x ? x : null;
+    "u" >= typeof x || x;
+    "u" > typeof x || x;
+    "u" <= typeof x && x;
+    "u" < typeof x && x;
 })();

```