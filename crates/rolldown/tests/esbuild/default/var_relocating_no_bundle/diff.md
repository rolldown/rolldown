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

//#region top-level.js
for (var b; 0;);
for (var { c, x: [d] } = {}; 0;);
for (var e of []);
for (var { f, x: [g] } of []);
for (var h in {});
for (var i = 1 in {});
for (var { j, x: [k] } in {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/top-level.js
+++ rolldown	top-level.js
@@ -1,10 +1,11 @@
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
+
+//#region top-level.js
+for (var b; 0;);
+for (var { c, x: [d] } = {}; 0;);
+for (var e of []);
+for (var { f, x: [g] } of []);
+for (var h in {});
+for (var i = 1 in {});
+for (var { j, x: [k] } in {});
+
+//#endregion
\ No newline at end of file

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

//#region nested.js
{
	var a;
	for (var b; 0;);
	for (var { c, x: [d] } = {}; 0;);
	for (var e of []);
	for (var { f, x: [g] } of []);
	for (var h in {});
	for (var i = 1 in {});
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
@@ -1,13 +1,15 @@
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
+
+//#region nested.js
+{
+	var a;
+	for (var b; 0;);
+	for (var { c, x: [d] } = {}; 0;);
+	for (var e of []);
+	for (var { f, x: [g] } of []);
+	for (var h in {});
+	for (var i = 1 in {});
+	for (var { j, x: [k] } in {});
+	function l() {}
 }
+
+//#endregion
\ No newline at end of file

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

//#region let.js
{
	let a;
	for (let b; 0;);
	for (let { c, x: [d] } = {}; 0;);
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
@@ -1,5 +1,5 @@
-if (true) {
+{
     let a;
     for (let b; 0; ) ;
     for (let {c, x: [d]} = {}; 0; ) ;
     for (let e of []) ;

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

//#region function.js
function x() {
	var a;
	for (var b; 0;);
	for (var { c, x: [d] } = {}; 0;);
	for (var e of []);
	for (var { f, x: [g] } of []);
	for (var h in {});
	for (var i = 1 in {});
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
@@ -1,13 +1,16 @@
+
+//#region function.js
 function x() {
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
+	var a;
+	for (var b; 0;);
+	for (var { c, x: [d] } = {}; 0;);
+	for (var e of []);
+	for (var { f, x: [g] } of []);
+	for (var h in {});
+	for (var i = 1 in {});
+	for (var { j, x: [k] } in {});
+	function l() {}
 }
 x();
+
+//#endregion
\ No newline at end of file

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

//#region function-nested.js
function x() {
	{
		var a;
		for (var b; 0;);
		for (var { c, x: [d] } = {}; 0;);
		for (var e of []);
		for (var { f, x: [g] } of []);
		for (var h in {});
		for (var i = 1 in {});
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
@@ -1,16 +1,18 @@
+
+//#region function-nested.js
 function x() {
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
+	{
+		var a;
+		for (var b; 0;);
+		for (var { c, x: [d] } = {}; 0;);
+		for (var e of []);
+		for (var { f, x: [g] } of []);
+		for (var h in {});
+		for (var i = 1 in {});
+		for (var { j, x: [k] } in {});
+		function l() {}
+	}
 }
 x();
+
+//#endregion
\ No newline at end of file

```