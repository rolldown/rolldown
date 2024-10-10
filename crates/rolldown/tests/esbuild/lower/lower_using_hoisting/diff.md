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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-use-strict.js
+++ rolldown	
@@ -1,20 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-directive.js
+++ rolldown	
@@ -1,20 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-import.js
+++ rolldown	
@@ -1,10 +0,0 @@
-import "./foo";
-var _stack = [];
-try {
-    var a = __using(_stack, b);
-    var c = __using(_stack, d);
-} catch (_) {
-    var _error = _, _hasError = true;
-} finally {
-    __callDispose(_stack, _error, _hasError);
-}

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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-star.js
+++ rolldown	
@@ -1,10 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-from.js
+++ rolldown	
@@ -1,10 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-clause.js
+++ rolldown	
@@ -1,10 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-local-direct.js
+++ rolldown	
@@ -1,16 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-local-indirect.js
+++ rolldown	
@@ -1,16 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-class-direct.js
+++ rolldown	
@@ -1,22 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-class-indirect.js
+++ rolldown	
@@ -1,22 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-function-direct.js
+++ rolldown	
@@ -1,21 +0,0 @@
-export function foo1() {
-    return [a, c];
-}
-export function bar1() {
-    return [a, c, bar1];
-}
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-function-indirect.js
+++ rolldown	
@@ -1,22 +0,0 @@
-function foo1() {
-    return [a, c];
-}
-function bar1() {
-    return [a, c, bar1];
-}
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-class-name-unused.js
+++ rolldown	
@@ -1,13 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-class-name-used.js
+++ rolldown	
@@ -1,13 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-class-anonymous.js
+++ rolldown	
@@ -1,13 +0,0 @@
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-function-name-unused.js
+++ rolldown	
@@ -1,12 +0,0 @@
-export default function foo() {
-    return [a, c];
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-function-name-used.js
+++ rolldown	
@@ -1,12 +0,0 @@
-export default function foo() {
-    return [a, c, foo];
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-function-anonymous.js
+++ rolldown	
@@ -1,12 +0,0 @@
-export default function () {
-    return [a, c];
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

```
### diff
```diff
===================================================================
--- esbuild	/out/hoist-export-default-expr.js
+++ rolldown	
@@ -1,11 +0,0 @@
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

```