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

```
### diff
```diff
===================================================================
--- esbuild	/out/1.js
+++ rolldown	
@@ -1,4 +0,0 @@
-if (foo) {
-    let x2 = function () {};
-    var x = x2;
-}

```
## /out/2.js
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

```
### diff
```diff
===================================================================
--- esbuild	/out/2.js
+++ rolldown	
@@ -1,4 +0,0 @@
-if (foo) {
-    function x() {}
-    eval("");
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/3.js
+++ rolldown	
@@ -1,6 +0,0 @@
-if (foo) {
-    function x() {}
-    if (bar) {
-        eval("");
-    }
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/4.js
+++ rolldown	
@@ -1,4 +0,0 @@
-if (foo) {
-    function x() {}
-    eval("");
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/5.js
+++ rolldown	
@@ -1,4 +0,0 @@
-"use strict";
-if (foo) {
-    let x = function () {};
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/6.js
+++ rolldown	
@@ -1,5 +0,0 @@
-"use strict";
-if (foo) {
-    function x() {}
-    eval("");
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/7.js
+++ rolldown	
@@ -1,7 +0,0 @@
-"use strict";
-if (foo) {
-    function x() {}
-    if (bar) {
-        eval("");
-    }
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/8.js
+++ rolldown	
@@ -1,5 +0,0 @@
-"use strict";
-if (foo) {
-    function x() {}
-    eval("");
-}

```