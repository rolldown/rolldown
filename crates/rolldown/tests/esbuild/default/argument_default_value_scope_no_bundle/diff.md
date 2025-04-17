# Reason
1. lowering class
# Diff
## /out.js
### esbuild
```js
export function a(o = foo) {
  var r;
  return o;
}
export class b {
  fn(r = foo) {
    var f;
    return r;
  }
}
export let c = [
  function(o = foo) {
    var r;
    return o;
  },
  (o = foo) => {
    var r;
    return o;
  },
  { fn(o = foo) {
    var r;
    return o;
  } },
  class {
    fn(o = foo) {
      var r;
      return o;
    }
  }
];
```
### rolldown
```js

//#region entry.js
function a(x = foo) {
	var foo$1;
	return x;
}
var b = class {
	fn(x = foo) {
		var foo$1;
		return x;
	}
};
let c = [
	function(x = foo) {
		var foo$1;
		return x;
	},
	(x = foo) => {
		var foo$1;
		return x;
	},
	{ fn(x = foo) {
		var foo$1;
		return x;
	} },
	class {
		fn(x = foo) {
			var foo$1;
			return x;
		}
	}
];

export { a, b, c };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,27 +1,28 @@
-export function a(o = foo) {
-    var r;
-    return o;
+function a(x = foo) {
+    var foo$1;
+    return x;
 }
-export class b {
-    fn(r = foo) {
-        var f;
-        return r;
+var b = class {
+    fn(x = foo) {
+        var foo$1;
+        return x;
     }
-}
-export let c = [function (o = foo) {
-    var r;
-    return o;
-}, (o = foo) => {
-    var r;
-    return o;
+};
+var c = [function (x = foo) {
+    var foo$1;
+    return x;
+}, (x = foo) => {
+    var foo$1;
+    return x;
 }, {
-    fn(o = foo) {
-        var r;
-        return o;
+    fn(x = foo) {
+        var foo$1;
+        return x;
     }
 }, class {
-    fn(o = foo) {
-        var r;
-        return o;
+    fn(x = foo) {
+        var foo$1;
+        return x;
     }
 }];
+export {a, b, c};

```