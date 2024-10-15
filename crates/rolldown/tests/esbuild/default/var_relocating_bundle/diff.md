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

//#region top-level.js
for (var b; 0;);
for (var e of []);
for (var { f, x: [g] } of []);
for (var h in {});
for (var { j, x: [k] } in {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/top-level.js
+++ rolldown	top-level.js
@@ -1,18 +1,5 @@
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
+for (var b; 0; ) ;
+for (var e of []) ;
+for (var {f, x: [g]} of []) ;
+for (var h in {}) ;
+for (var {j, x: [k]} in {}) ;

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

//#region nested.js
{
	var a;
	for (var b; 0;);
	for (var e of []);
	for (var { f, x: [g] } of []);
	for (var h in {});
	for (var { j, x: [k] } in {});
	function l() {}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/nested.js
+++ rolldown	nested.js
@@ -1,24 +1,9 @@
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
+{
+    var a;
+    for (var b; 0; ) ;
+    for (var e of []) ;
+    for (var {f, x: [g]} of []) ;
+    for (var h in {}) ;
+    for (var {j, x: [k]} in {}) ;
+    function l() {}
 }
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

//#region let.js
{
	let a;
	for (let b; 0;);
	for (let e of []);
	for (let { f, x: [g] } of []);
	for (let h in {});
	for (let { j, x: [k] } in {});
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/let.js
+++ rolldown	let.js
@@ -1,8 +1,7 @@
-if (true) {
+{
     let a;
     for (let b; 0; ) ;
-    for (let {c, x: [d]} = {}; 0; ) ;
     for (let e of []) ;
     for (let {f, x: [g]} of []) ;
     for (let h in {}) ;
     for (let {j, x: [k]} in {}) ;

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

//#region function.js
function x() {
	var a;
	for (var b; 0;);
	for (var e of []);
	for (var { f, x: [g] } of []);
	for (var h in {});
	for (var { j, x: [k] } in {});
	function l() {}
}
x();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/function.js
+++ rolldown	function.js
@@ -1,13 +1,10 @@
 function x() {
     var a;
     for (var b; 0; ) ;
-    for (var {c, x: [d]} = {}; 0; ) ;
     for (var e of []) ;
     for (var {f, x: [g]} of []) ;
     for (var h in {}) ;
-    i = 1;
-    for (var i in {}) ;
     for (var {j, x: [k]} in {}) ;
     function l() {}
 }
 x();

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

//#region function-nested.js
function x() {
	{
		var a;
		for (var b; 0;);
		for (var e of []);
		for (var { f, x: [g] } of []);
		for (var h in {});
		for (var { j, x: [k] } in {});
		function l() {}
	}
}
x();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/function-nested.js
+++ rolldown	function-nested.js
@@ -1,16 +1,12 @@
 function x() {
-    if (true) {
-        let l2 = function () {};
-        var l = l2;
+    {
         var a;
         for (var b; 0; ) ;
-        for (var {c, x: [d]} = {}; 0; ) ;
         for (var e of []) ;
         for (var {f, x: [g]} of []) ;
         for (var h in {}) ;
-        i = 1;
-        for (var i in {}) ;
         for (var {j, x: [k]} in {}) ;
+        function l() {}
     }
 }
 x();

```