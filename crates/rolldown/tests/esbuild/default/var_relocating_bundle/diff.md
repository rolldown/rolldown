# Diff
## /out/top-level.js
### esbuild
```js
// top-level.js
for (; 0; ) ;
var b;
for ({ c, x: [d] } = {}; 0; ) ;
var c;
var d;
for (e of []) ;
var e;
for ({ f, x: [g] } of []) ;
var f;
var g;
for (h in {}) ;
var h;
i = 1;
for (i in {}) ;
var i;
for ({ j, x: [k] } in {}) ;
var j;
var k;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/top-level.js
+++ rolldown	
@@ -1,18 +0,0 @@
-for (; 0; ) ;
-var b;
-for ({c, x: [d]} = {}; 0; ) ;
-var c;
-var d;
-for (e of []) ;
-var e;
-for ({f, x: [g]} of []) ;
-var f;
-var g;
-for (h in {}) ;
-var h;
-i = 1;
-for (i in {}) ;
-var i;
-for ({j, x: [k]} in {}) ;
-var j;
-var k;

```
## /out/nested.js
### esbuild
```js
// nested.js
if (true) {
  let l = function() {
  };
  l2 = l;
  for (; 0; ) ;
  for ({ c, x: [d] } = {}; 0; ) ;
  for (e of []) ;
  for ({ f, x: [g] } of []) ;
  for (h in {}) ;
  i = 1;
  for (i in {}) ;
  for ({ j, x: [k] } in {}) ;
}
var a;
var b;
var c;
var d;
var e;
var f;
var g;
var h;
var i;
var j;
var k;
var l2;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested.js
+++ rolldown	
@@ -1,24 +0,0 @@
-if (true) {
-    let l = function () {};
-    l2 = l;
-    for (; 0; ) ;
-    for ({c, x: [d]} = {}; 0; ) ;
-    for (e of []) ;
-    for ({f, x: [g]} of []) ;
-    for (h in {}) ;
-    i = 1;
-    for (i in {}) ;
-    for ({j, x: [k]} in {}) ;
-}
-var a;
-var b;
-var c;
-var d;
-var e;
-var f;
-var g;
-var h;
-var i;
-var j;
-var k;
-var l2;

```
## /out/let.js
### esbuild
```js
// let.js
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
// function.js
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
// function-nested.js
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