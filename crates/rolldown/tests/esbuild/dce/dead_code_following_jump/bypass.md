# Reason
1. rolldown tree shaking is not control flow aware, this part could be done in minifier
# Diff
## /out.js
### esbuild
```js
// entry.js
function testReturn() {
  return y + z();
  if (x)
    var y;
  function z() {
    KEEP_ME();
  }
}
function testThrow() {
  throw y + z();
  if (x)
    var y;
  function z() {
    KEEP_ME();
  }
}
function testBreak() {
  for (; ; ) {
    let z2 = function() {
      KEEP_ME();
    };
    var z = z2;
    y + z2();
    break;
    if (x)
      var y;
  }
}
function testContinue() {
  for (; ; ) {
    let z2 = function() {
      KEEP_ME();
    };
    var z = z2;
    y + z2();
    continue;
    if (x)
      var y;
  }
}
function testStmts() {
  return [a, b, c, d, e, f, g, h, i];
  for (; x; )
    var a;
  do
    var b;
  while (x);
  for (var c; ; ) ;
  for (var d in x) ;
  for (var e of x) ;
  if (x)
    var f;
  if (!x) var g;
  var h, i;
}
testReturn();
testThrow();
testBreak();
testContinue();
testStmts();
```
### rolldown
```js
//#region entry.js
function testReturn() {
	return y + z();
	function z() {
		KEEP_ME();
	}
	var y;
}
function testThrow() {
	throw y + z();
	function z() {
		KEEP_ME();
	}
	var y;
}
function testBreak() {
	while (true) {
		y + z();
		break;
		function z() {
			KEEP_ME();
		}
		var y;
	}
}
function testContinue() {
	while (true) {
		y + z();
		continue;
		function z() {
			KEEP_ME();
		}
		var y;
	}
}
function testStmts() {
	return [
		a,
		b,
		c,
		d,
		e,
		f,
		g,
		h,
		i
	];
	var a, b, c, d, e, f, g, h, i;
}
testReturn();
testThrow();
testBreak();
testContinue();
testStmts();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,50 +1,41 @@
 function testReturn() {
     return y + z();
-    if (x) var y;
     function z() {
         KEEP_ME();
     }
+    var y;
 }
 function testThrow() {
     throw y + z();
-    if (x) var y;
     function z() {
         KEEP_ME();
     }
+    var y;
 }
 function testBreak() {
-    for (; ; ) {
-        let z2 = function () {
-            KEEP_ME();
-        };
-        var z = z2;
-        y + z2();
+    while (true) {
+        y + z();
         break;
-        if (x) var y;
+        function z() {
+            KEEP_ME();
+        }
+        var y;
     }
 }
 function testContinue() {
-    for (; ; ) {
-        let z2 = function () {
-            KEEP_ME();
-        };
-        var z = z2;
-        y + z2();
+    while (true) {
+        y + z();
         continue;
-        if (x) var y;
+        function z() {
+            KEEP_ME();
+        }
+        var y;
     }
 }
 function testStmts() {
     return [a, b, c, d, e, f, g, h, i];
-    for (; x; ) var a;
-    do var b; while (x);
-    for (var c; ; ) ;
-    for (var d in x) ;
-    for (var e of x) ;
-    if (x) var f;
-    if (!x) var g;
-    var h, i;
+    var a, b, c, d, e, f, g, h, i;
 }
 testReturn();
 testThrow();
 testBreak();

```