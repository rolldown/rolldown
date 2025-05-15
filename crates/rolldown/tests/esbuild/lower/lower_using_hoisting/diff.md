# Diff
## /out/hoist-use-strict.js
### esbuild
```js
"use strict";
function foo() {
  "use strict";
  var _stack2 = [];
  try {
    const a2 = __using(_stack2, b);
  } catch (_2) {
    var _error2 = _2, _hasError2 = true;
  } finally {
    __callDispose(_stack2, _error2, _hasError2);
  }
}
var _stack = [];
try {
  var a = __using(_stack, b);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
//#region hoist-use-strict.js
using a = b;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-use-strict.js
+++ rolldown	hoist-use-strict.js
@@ -1,20 +1,4 @@
-"use strict";
-function foo() {
-    "use strict";
-    var _stack2 = [];
-    try {
-        const a2 = __using(_stack2, b);
-    } catch (_2) {
-        var _error2 = _2, _hasError2 = true;
-    } finally {
-        __callDispose(_stack2, _error2, _hasError2);
-    }
-}
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+//#region hoist-use-strict.js
+using a = b;
+
+//#endregion
\ No newline at end of file

```
## /out/hoist-directive.js
### esbuild
```js
"use wtf";
function foo() {
  "use wtf";
  var _stack2 = [];
  try {
    const a2 = __using(_stack2, b);
  } catch (_2) {
    var _error2 = _2, _hasError2 = true;
  } finally {
    __callDispose(_stack2, _error2, _hasError2);
  }
}
var _stack = [];
try {
  var a = __using(_stack, b);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
"use wtf"

//#region hoist-directive.js
using a = b;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-directive.js
+++ rolldown	hoist-directive.js
@@ -1,20 +1,6 @@
-"use wtf";
-function foo() {
-    "use wtf";
-    var _stack2 = [];
-    try {
-        const a2 = __using(_stack2, b);
-    } catch (_2) {
-        var _error2 = _2, _hasError2 = true;
-    } finally {
-        __callDispose(_stack2, _error2, _hasError2);
-    }
-}
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+"use wtf"
+
+//#region hoist-directive.js
+using a = b;
+
+//#endregion
\ No newline at end of file

```
## /out/hoist-import.js
### esbuild
```js
import "./foo";
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
import "./foo";

//#region hoist-import.js
using a = b;
using c = d;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-import.js
+++ rolldown	hoist-import.js
@@ -1,10 +1,7 @@
 import "./foo";
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+
+//#region hoist-import.js
+using a = b;
+using c = d;
+
+//#endregion
\ No newline at end of file

```
## /out/hoist-export-star.js
### esbuild
```js
export * from "./foo";
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
export * from "./foo"

//#region hoist-export-star.js
using a = b;
using c = d;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-star.js
+++ rolldown	hoist-export-star.js
@@ -1,10 +1,7 @@
-export * from "./foo";
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+export * from "./foo"
+
+//#region hoist-export-star.js
+using a = b;
+using c = d;
+
+//#endregion
\ No newline at end of file

```
## /out/hoist-export-from.js
### esbuild
```js
export { x, y } from "./foo";
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
import { x, y } from "./foo";

//#region hoist-export-from.js
using a = b;
using c = d;

//#endregion
export { x, y };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-from.js
+++ rolldown	hoist-export-from.js
@@ -1,10 +1,8 @@
-export {x, y} from "./foo";
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+import { x, y } from "./foo";
+
+//#region hoist-export-from.js
+using a = b;
+using c = d;
+
+//#endregion
+export { x, y };
\ No newline at end of file

```
## /out/hoist-export-clause.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  a,
  c as "c!"
};
```
### rolldown
```js
//#region hoist-export-clause.js
using a = b;
using c = d;

//#endregion
export { a, c as "c!" };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-clause.js
+++ rolldown	hoist-export-clause.js
@@ -1,10 +1,6 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {a, c as undefined};
+//#region hoist-export-clause.js
+using a = b;
+using c = d;
+
+//#endregion
+export { a, c as "c!" };
\ No newline at end of file

```
## /out/hoist-export-local-direct.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var ac1 = [a, c], { x: [x1] } = foo;
  var a1 = a, { y: [y1] } = foo;
  var c1 = c, { z: [z1] } = foo;
  var ac2 = [a, c], { x: [x2] } = foo;
  var a2 = a, { y: [y2] } = foo;
  var c2 = c, { z: [z2] } = foo;
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  ac1,
  x1,
  a1,
  y1,
  c1,
  z1
};
```
### rolldown
```js
//#region hoist-export-local-direct.js
using a = b;
var ac1 = [a, c];
var { x: [x1] } = foo;
let a1 = a;
let { y: [y1] } = foo;
const c1 = c;
const { z: [z1] } = foo;
var ac2 = [a, c], { x: [x2] } = foo;
let a2 = a, { y: [y2] } = foo;
const c2 = c, { z: [z2] } = foo;
using c = d;

//#endregion
export { a1, ac1, c1, x1, y1, z1 };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-local-direct.js
+++ rolldown	hoist-export-local-direct.js
@@ -1,16 +1,15 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var ac1 = [a, c], {x: [x1]} = foo;
-    var a1 = a, {y: [y1]} = foo;
-    var c1 = c, {z: [z1]} = foo;
-    var ac2 = [a, c], {x: [x2]} = foo;
-    var a2 = a, {y: [y2]} = foo;
-    var c2 = c, {z: [z2]} = foo;
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {ac1, x1, a1, y1, c1, z1};
+//#region hoist-export-local-direct.js
+using a = b;
+var ac1 = [a, c];
+var { x: [x1] } = foo;
+let a1 = a;
+let { y: [y1] } = foo;
+const c1 = c;
+const { z: [z1] } = foo;
+var ac2 = [a, c], { x: [x2] } = foo;
+let a2 = a, { y: [y2] } = foo;
+const c2 = c, { z: [z2] } = foo;
+using c = d;
+
+//#endregion
+export { a1, ac1, c1, x1, y1, z1 };
\ No newline at end of file

```
## /out/hoist-export-local-indirect.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var ac1 = [a, c], { x: [x1] } = foo;
  var a1 = a, { y: [y1] } = foo;
  var c1 = c, { z: [z1] } = foo;
  var ac2 = [a, c], { x: [x2] } = foo;
  var a2 = a, { y: [y2] } = foo;
  var c2 = c, { z: [z2] } = foo;
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  x1,
  y1,
  z1
};
```
### rolldown
```js
//#region hoist-export-local-indirect.js
using a = b;
var ac1 = [a, c], { x: [x1] } = foo;
let a1 = a, { y: [y1] } = foo;
const c1 = c, { z: [z1] } = foo;
var ac2 = [a, c], { x: [x2] } = foo;
let a2 = a, { y: [y2] } = foo;
const c2 = c, { z: [z2] } = foo;
using c = d;

//#endregion
export { x1, y1, z1 };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-local-indirect.js
+++ rolldown	hoist-export-local-indirect.js
@@ -1,16 +1,12 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var ac1 = [a, c], {x: [x1]} = foo;
-    var a1 = a, {y: [y1]} = foo;
-    var c1 = c, {z: [z1]} = foo;
-    var ac2 = [a, c], {x: [x2]} = foo;
-    var a2 = a, {y: [y2]} = foo;
-    var c2 = c, {z: [z2]} = foo;
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {x1, y1, z1};
+//#region hoist-export-local-indirect.js
+using a = b;
+var ac1 = [a, c], { x: [x1] } = foo;
+let a1 = a, { y: [y1] } = foo;
+const c1 = c, { z: [z1] } = foo;
+var ac2 = [a, c], { x: [x2] } = foo;
+let a2 = a, { y: [y2] } = foo;
+const c2 = c, { z: [z2] } = foo;
+using c = d;
+
+//#endregion
+export { x1, y1, z1 };
\ No newline at end of file

```
## /out/hoist-export-class-direct.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var Foo1 = class {
    ac = [a, c];
  };
  var Bar1 = class _Bar1 {
    ac = [a, c, _Bar1];
  };
  var Foo2 = class {
    ac = [a, c];
  };
  var Bar2 = class _Bar2 {
    ac = [a, c, _Bar2];
  };
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  Foo1,
  Bar1
};
```
### rolldown
```js
//#region hoist-export-class-direct.js
using a = b;
var Foo1 = class {
	ac = [a, c];
};
var Bar1 = class Bar1 {
	ac = [
		a,
		c,
		Bar1
	];
};
using c = d;

//#endregion
export { Bar1, Foo1 };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-class-direct.js
+++ rolldown	hoist-export-class-direct.js
@@ -1,22 +1,16 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var Foo1 = class {
-        ac = [a, c];
-    };
-    var Bar1 = class _Bar1 {
-        ac = [a, c, _Bar1];
-    };
-    var Foo2 = class {
-        ac = [a, c];
-    };
-    var Bar2 = class _Bar2 {
-        ac = [a, c, _Bar2];
-    };
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {Foo1, Bar1};
+//#region hoist-export-class-direct.js
+using a = b;
+var Foo1 = class {
+	ac = [a, c];
+};
+var Bar1 = class Bar1 {
+	ac = [
+		a,
+		c,
+		Bar1
+	];
+};
+using c = d;
+
+//#endregion
+export { Bar1, Foo1 };
\ No newline at end of file

```
## /out/hoist-export-class-indirect.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var Foo1 = class {
    ac = [a, c];
  };
  var Bar1 = class _Bar1 {
    ac = [a, c, _Bar1];
  };
  var Foo2 = class {
    ac = [a, c];
  };
  var Bar2 = class _Bar2 {
    ac = [a, c, _Bar2];
  };
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  Foo1,
  Bar1
};
```
### rolldown
```js
//#region hoist-export-class-indirect.js
using a = b;
var Foo1 = class {
	ac = [a, c];
};
var Bar1 = class Bar1 {
	ac = [
		a,
		c,
		Bar1
	];
};
using c = d;

//#endregion
export { Bar1, Foo1 };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-class-indirect.js
+++ rolldown	hoist-export-class-indirect.js
@@ -1,22 +1,16 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var Foo1 = class {
-        ac = [a, c];
-    };
-    var Bar1 = class _Bar1 {
-        ac = [a, c, _Bar1];
-    };
-    var Foo2 = class {
-        ac = [a, c];
-    };
-    var Bar2 = class _Bar2 {
-        ac = [a, c, _Bar2];
-    };
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {Foo1, Bar1};
+//#region hoist-export-class-indirect.js
+using a = b;
+var Foo1 = class {
+	ac = [a, c];
+};
+var Bar1 = class Bar1 {
+	ac = [
+		a,
+		c,
+		Bar1
+	];
+};
+using c = d;
+
+//#endregion
+export { Bar1, Foo1 };
\ No newline at end of file

```
## /out/hoist-export-function-direct.js
### esbuild
```js
export function foo1() {
  return [a, c];
}
export function bar1() {
  return [a, c, bar1];
}
function foo2() {
  return [a, c];
}
function bar2() {
  return [a, c, bar2];
}
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
//#region hoist-export-function-direct.js
using a = b;
function foo1() {
	return [a, c];
}
function bar1() {
	return [
		a,
		c,
		bar1
	];
}
using c = d;

//#endregion
export { bar1, foo1 };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-function-direct.js
+++ rolldown	hoist-export-function-direct.js
@@ -1,21 +1,16 @@
-export function foo1() {
-    return [a, c];
+//#region hoist-export-function-direct.js
+using a = b;
+function foo1() {
+	return [a, c];
 }
-export function bar1() {
-    return [a, c, bar1];
+function bar1() {
+	return [
+		a,
+		c,
+		bar1
+	];
 }
-function foo2() {
-    return [a, c];
-}
-function bar2() {
-    return [a, c, bar2];
-}
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+using c = d;
+
+//#endregion
+export { bar1, foo1 };
\ No newline at end of file

```
## /out/hoist-export-function-indirect.js
### esbuild
```js
function foo1() {
  return [a, c];
}
function bar1() {
  return [a, c, bar1];
}
function foo2() {
  return [a, c];
}
function bar2() {
  return [a, c, bar2];
}
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  foo1,
  bar1
};
```
### rolldown
```js
//#region hoist-export-function-indirect.js
using a = b;
function foo1() {
	return [a, c];
}
function bar1() {
	return [
		a,
		c,
		bar1
	];
}
using c = d;

//#endregion
export { bar1, foo1 };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-function-indirect.js
+++ rolldown	hoist-export-function-indirect.js
@@ -1,22 +1,16 @@
+//#region hoist-export-function-indirect.js
+using a = b;
 function foo1() {
-    return [a, c];
+	return [a, c];
 }
 function bar1() {
-    return [a, c, bar1];
+	return [
+		a,
+		c,
+		bar1
+	];
 }
-function foo2() {
-    return [a, c];
-}
-function bar2() {
-    return [a, c, bar2];
-}
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {foo1, bar1};
+using c = d;
+
+//#endregion
+export { bar1, foo1 };
\ No newline at end of file

```
## /out/hoist-export-default-class-name-unused.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var Foo = class {
    ac = [a, c];
  };
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  Foo as default
};
```
### rolldown
```js
//#region hoist-export-default-class-name-unused.js
using a = b;
var Foo = class {
	ac = [a, c];
};
using c = d;

//#endregion
export { Foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-class-name-unused.js
+++ rolldown	hoist-export-default-class-name-unused.js
@@ -1,13 +1,9 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var Foo = class {
-        ac = [a, c];
-    };
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {Foo as default};
+//#region hoist-export-default-class-name-unused.js
+using a = b;
+var Foo = class {
+	ac = [a, c];
+};
+using c = d;
+
+//#endregion
+export { Foo as default };
\ No newline at end of file

```
## /out/hoist-export-default-class-name-used.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var Foo = class _Foo {
    ac = [a, c, _Foo];
  };
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  Foo as default
};
```
### rolldown
```js
//#region hoist-export-default-class-name-used.js
using a = b;
var Foo = class Foo {
	ac = [
		a,
		c,
		Foo
	];
};
using c = d;

//#endregion
export { Foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-class-name-used.js
+++ rolldown	hoist-export-default-class-name-used.js
@@ -1,13 +1,13 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var Foo = class _Foo {
-        ac = [a, c, _Foo];
-    };
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {Foo as default};
+//#region hoist-export-default-class-name-used.js
+using a = b;
+var Foo = class Foo {
+	ac = [
+		a,
+		c,
+		Foo
+	];
+};
+using c = d;
+
+//#endregion
+export { Foo as default };
\ No newline at end of file

```
## /out/hoist-export-default-class-anonymous.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var hoist_export_default_class_anonymous_default = class {
    ac = [a, c];
  };
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  hoist_export_default_class_anonymous_default as default
};
```
### rolldown
```js
//#region hoist-export-default-class-anonymous.js
using a = b;
var hoist_export_default_class_anonymous_default = class {
	ac = [a, c];
};
using c = d;

//#endregion
export { hoist_export_default_class_anonymous_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-class-anonymous.js
+++ rolldown	hoist-export-default-class-anonymous.js
@@ -1,13 +1,9 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var hoist_export_default_class_anonymous_default = class {
-        ac = [a, c];
-    };
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {hoist_export_default_class_anonymous_default as default};
+//#region hoist-export-default-class-anonymous.js
+using a = b;
+var hoist_export_default_class_anonymous_default = class {
+	ac = [a, c];
+};
+using c = d;
+
+//#endregion
+export { hoist_export_default_class_anonymous_default as default };
\ No newline at end of file

```
## /out/hoist-export-default-function-name-unused.js
### esbuild
```js
export default function foo() {
  return [a, c];
}
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
//#region hoist-export-default-function-name-unused.js
using a = b;
function foo() {
	return [a, c];
}
using c = d;

//#endregion
export { foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-function-name-unused.js
+++ rolldown	hoist-export-default-function-name-unused.js
@@ -1,12 +1,9 @@
-export default function foo() {
-    return [a, c];
+//#region hoist-export-default-function-name-unused.js
+using a = b;
+function foo() {
+	return [a, c];
 }
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+using c = d;
+
+//#endregion
+export { foo as default };
\ No newline at end of file

```
## /out/hoist-export-default-function-name-used.js
### esbuild
```js
export default function foo() {
  return [a, c, foo];
}
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
//#region hoist-export-default-function-name-used.js
using a = b;
function foo() {
	return [
		a,
		c,
		foo
	];
}
using c = d;

//#endregion
export { foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-function-name-used.js
+++ rolldown	hoist-export-default-function-name-used.js
@@ -1,12 +1,13 @@
-export default function foo() {
-    return [a, c, foo];
+//#region hoist-export-default-function-name-used.js
+using a = b;
+function foo() {
+	return [
+		a,
+		c,
+		foo
+	];
 }
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+using c = d;
+
+//#endregion
+export { foo as default };
\ No newline at end of file

```
## /out/hoist-export-default-function-anonymous.js
### esbuild
```js
export default function() {
  return [a, c];
}
var _stack = [];
try {
  var a = __using(_stack, b);
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
```
### rolldown
```js
//#region hoist-export-default-function-anonymous.js
using a = b;
function hoist_export_default_function_anonymous_default() {
	return [a, c];
}
using c = d;

//#endregion
export { hoist_export_default_function_anonymous_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-function-anonymous.js
+++ rolldown	hoist-export-default-function-anonymous.js
@@ -1,12 +1,9 @@
-export default function () {
-    return [a, c];
+//#region hoist-export-default-function-anonymous.js
+using a = b;
+function hoist_export_default_function_anonymous_default() {
+	return [a, c];
 }
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
+using c = d;
+
+//#endregion
+export { hoist_export_default_function_anonymous_default as default };
\ No newline at end of file

```
## /out/hoist-export-default-expr.js
### esbuild
```js
var _stack = [];
try {
  var a = __using(_stack, b);
  var hoist_export_default_expr_default = [a, c];
  var c = __using(_stack, d);
} catch (_) {
  var _error = _, _hasError = true;
} finally {
  __callDispose(_stack, _error, _hasError);
}
export {
  hoist_export_default_expr_default as default
};
```
### rolldown
```js
//#region hoist-export-default-expr.js
using a = b;
var hoist_export_default_expr_default = [a, c];
using c = d;

//#endregion
export { hoist_export_default_expr_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-expr.js
+++ rolldown	hoist-export-default-expr.js
@@ -1,11 +1,7 @@
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var hoist_export_default_expr_default = [a, c];
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}
-export {hoist_export_default_expr_default as default};
+//#region hoist-export-default-expr.js
+using a = b;
+var hoist_export_default_expr_default = [a, c];
+using c = d;
+
+//#endregion
+export { hoist_export_default_expr_default as default };
\ No newline at end of file

```