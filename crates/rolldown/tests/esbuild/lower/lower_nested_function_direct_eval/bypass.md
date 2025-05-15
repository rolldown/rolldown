# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out/1.js
### esbuild
```js
if (foo) {
  let x2 = function() {
  };
  var x = x2;
}
```
### rolldown
```js
//#region 1.js
if (foo) {
	function x() {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/1.js
+++ rolldown	1.js
@@ -1,4 +1,3 @@
 if (foo) {
-    let x2 = function () {};
-    var x = x2;
+    function x() {}
 }

```
## /out/3.js
### esbuild
```js
if (foo) {
  function x() {
  }
  if (bar) {
    eval("");
  }
}
```
### rolldown
```js
//#region 3.js
if (foo) {
	function x() {}
	if (bar) eval("");
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/3.js
+++ rolldown	3.js
@@ -1,6 +1,4 @@
 if (foo) {
     function x() {}
-    if (bar) {
-        eval("");
-    }
+    if (bar) eval("");
 }

```
## /out/4.js
### esbuild
```js
if (foo) {
  function x() {
  }
  eval("");
}
```
### rolldown
```js
//#region 4.js
if (foo) {
	eval("");
	function x() {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/4.js
+++ rolldown	4.js
@@ -1,4 +1,4 @@
 if (foo) {
-    function x() {}
     eval("");
+    function x() {}
 }

```
## /out/5.js
### esbuild
```js
"use strict";
if (foo) {
  let x = function() {
  };
}
```
### rolldown
```js
'use strict';


//#region 5.js
if (foo) {
	function x() {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/5.js
+++ rolldown	5.js
@@ -1,4 +1,3 @@
-"use strict";
 if (foo) {
-    let x = function () {};
+    function x() {}
 }

```
## /out/6.js
### esbuild
```js
"use strict";
if (foo) {
  function x() {
  }
  eval("");
}
```
### rolldown
```js
'use strict';


//#region 6.js
if (foo) {
	function x() {}
	eval("");
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/6.js
+++ rolldown	6.js
@@ -1,5 +1,4 @@
-"use strict";
 if (foo) {
     function x() {}
     eval("");
 }

```
## /out/7.js
### esbuild
```js
"use strict";
if (foo) {
  function x() {
  }
  if (bar) {
    eval("");
  }
}
```
### rolldown
```js
'use strict';


//#region 7.js
if (foo) {
	function x() {}
	if (bar) eval("");
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/7.js
+++ rolldown	7.js
@@ -1,7 +1,4 @@
-"use strict";
 if (foo) {
     function x() {}
-    if (bar) {
-        eval("");
-    }
+    if (bar) eval("");
 }

```
## /out/8.js
### esbuild
```js
"use strict";
if (foo) {
  function x() {
  }
  eval("");
}
```
### rolldown
```js
'use strict';


//#region 8.js
if (foo) {
	eval("");
	function x() {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/8.js
+++ rolldown	8.js
@@ -1,5 +1,4 @@
-"use strict";
 if (foo) {
-    function x() {}
     eval("");
+    function x() {}
 }

```