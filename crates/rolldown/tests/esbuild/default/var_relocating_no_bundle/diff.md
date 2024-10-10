# Diff
## /out/top-level.js
### esbuild
```js
var a;
for (var b; 0; ) ;
for (var { c, x: [d] } = {}; 0; ) ;
for (var e of []) ;
for (var { f, x: [g] } of []) ;
for (var h in {}) ;
i = 1;
for (var i in {}) ;
for (var { j, x: [k] } in {}) ;
function l() {
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/top-level.js
+++ rolldown	
@@ -1,10 +0,0 @@
-var a;
-for (var b; 0; ) ;
-for (var {c, x: [d]} = {}; 0; ) ;
-for (var e of []) ;
-for (var {f, x: [g]} of []) ;
-for (var h in {}) ;
-i = 1;
-for (var i in {}) ;
-for (var {j, x: [k]} in {}) ;
-function l() {}

```
## /out/nested.js
### esbuild
```js
if (true) {
  let l = function() {
  };
  var l2 = l;
  var a;
  for (var b; 0; ) ;
  for (var { c, x: [d] } = {}; 0; ) ;
  for (var e of []) ;
  for (var { f, x: [g] } of []) ;
  for (var h in {}) ;
  i = 1;
  for (var i in {}) ;
  for (var { j, x: [k] } in {}) ;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested.js
+++ rolldown	
@@ -1,13 +0,0 @@
-if (true) {
-    let l = function () {};
-    var l2 = l;
-    var a;
-    for (var b; 0; ) ;
-    for (var {c, x: [d]} = {}; 0; ) ;
-    for (var e of []) ;
-    for (var {f, x: [g]} of []) ;
-    for (var h in {}) ;
-    i = 1;
-    for (var i in {}) ;
-    for (var {j, x: [k]} in {}) ;
-}

```
## /out/let.js
### esbuild
```js
if (true) {
  let a;
  for (let b; 0; ) ;
  for (let { c, x: [d] } = {}; 0; ) ;
  for (let e of []) ;
  for (let { f, x: [g] } of []) ;
  for (let h in {}) ;
  for (let { j, x: [k] } in {}) ;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/let.js
+++ rolldown	
@@ -1,9 +0,0 @@
-if (true) {
-    let a;
-    for (let b; 0; ) ;
-    for (let {c, x: [d]} = {}; 0; ) ;
-    for (let e of []) ;
-    for (let {f, x: [g]} of []) ;
-    for (let h in {}) ;
-    for (let {j, x: [k]} in {}) ;
-}

```
## /out/function.js
### esbuild
```js
function x() {
  var a;
  for (var b; 0; ) ;
  for (var { c, x: [d] } = {}; 0; ) ;
  for (var e of []) ;
  for (var { f, x: [g] } of []) ;
  for (var h in {}) ;
  i = 1;
  for (var i in {}) ;
  for (var { j, x: [k] } in {}) ;
  function l() {
  }
}
x();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/function.js
+++ rolldown	
@@ -1,13 +0,0 @@
-function x() {
-    var a;
-    for (var b; 0; ) ;
-    for (var {c, x: [d]} = {}; 0; ) ;
-    for (var e of []) ;
-    for (var {f, x: [g]} of []) ;
-    for (var h in {}) ;
-    i = 1;
-    for (var i in {}) ;
-    for (var {j, x: [k]} in {}) ;
-    function l() {}
-}
-x();

```
## /out/function-nested.js
### esbuild
```js
function x() {
  if (true) {
    let l2 = function() {
    };
    var l = l2;
    var a;
    for (var b; 0; ) ;
    for (var { c, x: [d] } = {}; 0; ) ;
    for (var e of []) ;
    for (var { f, x: [g] } of []) ;
    for (var h in {}) ;
    i = 1;
    for (var i in {}) ;
    for (var { j, x: [k] } in {}) ;
  }
}
x();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/function-nested.js
+++ rolldown	
@@ -1,16 +0,0 @@
-function x() {
-    if (true) {
-        let l2 = function () {};
-        var l = l2;
-        var a;
-        for (var b; 0; ) ;
-        for (var {c, x: [d]} = {}; 0; ) ;
-        for (var e of []) ;
-        for (var {f, x: [g]} of []) ;
-        for (var h in {}) ;
-        i = 1;
-        for (var i in {}) ;
-        for (var {j, x: [k]} in {}) ;
-    }
-}
-x();

```