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
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-(() => {
+(function () {
     var keep_1 = typeof x <= "u" ? y : null;
     var keep_1 = typeof x < "u" ? y : null;
     var keep_1 = typeof x >= "u" ? null : y;
     var keep_1 = typeof x > "u" ? null : y;

```