# Reason
1. related to minifier
2. should be same as esbuild in `bundle` mode no minifier, https://hyrious.me/esbuild-repl/?version=0.23.0&b=e%00entry.js%00%28%28%29+%3D%3E+%7B%0A%09var+arguments%3B%0A%0A%09function+foo%28x+%3D+arguments%29+%7B+return+arguments+%7D%0A%09%28function%28x+%3D+arguments%29+%7B+return+arguments+%7D%29%3B%0A%09%28%7Bfoo%28x+%3D+arguments%29+%7B+return+arguments+%7D%7D%29%3B%0A%09class+Foo+%7B+foo%28x+%3D+arguments%29+%7B+return+arguments+%7D+%7D%0A%09%28class+%7B+foo%28x+%3D+arguments%29+%7B+return+arguments+%7D+%7D%29%3B%0A%0A%09function+foo%28x+%3D+arguments%29+%7B+var+arguments%3B+return+arguments+%7D%0A%09%28function%28x+%3D+arguments%29+%7B+var+arguments%3B+return+arguments+%7D%29%3B%0A%09%28%7Bfoo%28x+%3D+arguments%29+%7B+var+arguments%3B+return+arguments+%7D%7D%29%3B%0A%0A%09%28x+%3D%3E+arguments%29%3B%0A%09%28%28%29+%3D%3E+arguments%29%3B%0A%09%28async+%28%29+%3D%3E+arguments%29%3B%0A%09%28%28x+%3D+arguments%29+%3D%3E+arguments%29%3B%0A%09%28async+%28x+%3D+arguments%29+%3D%3E+arguments%29%3B%0A%0A%09x+%3D%3E+arguments%3B%0A%09%28%29+%3D%3E+arguments%3B%0A%09async+%28%29+%3D%3E+arguments%3B%0A%09%28x+%3D+arguments%29+%3D%3E+arguments%3B%0A%09async+%28x+%3D+arguments%29+%3D%3E+arguments%3B%0A%0A%09%28x+%3D%3E+%7B+return+arguments+%7D%29%3B%0A%09%28%28%29+%3D%3E+%7B+return+arguments+%7D%29%3B%0A%09%28async+%28%29+%3D%3E+%7B+return+arguments+%7D%29%3B%0A%09%28%28x+%3D+arguments%29+%3D%3E+%7B+return+arguments+%7D%29%3B%0A%09%28async+%28x+%3D+arguments%29+%3D%3E+%7B+return+arguments+%7D%29%3B%0A%0A%09x+%3D%3E+%7B+return+arguments+%7D%3B%0A%09%28%29+%3D%3E+%7B+return+arguments+%7D%3B%0A%09async+%28%29+%3D%3E+%7B+return+arguments+%7D%3B%0A%09%28x+%3D+arguments%29+%3D%3E+%7B+return+arguments+%7D%3B%0A%09async+%28x+%3D+arguments%29+%3D%3E+%7B+return+arguments+%7D%3B%0A%7D%29%28%29%3B%0A&b=%00file.js%00&o=%7B%0A++treeShaking%3A+false%2C%0A+%0A%22bundle%22%3A+true%2C%0Aformat%3A+%22cjs%22%2C%0Aminify%3A+false%0A%7D
# Diff
## /out.js
### esbuild
```js
/* @__PURE__ */ (() => {
  var r;
  function t(n = arguments) {
    return arguments;
  }
  (function(n = arguments) {
    return arguments;
  });
  ({ foo(n = arguments) {
    return arguments;
  } });
  class u {
    foo(e = arguments) {
      return arguments;
    }
  }
  (class {
    foo(n = arguments) {
      return arguments;
    }
  });
  function t(n = arguments) {
    var arguments;
    return arguments;
  }
  (function(n = arguments) {
    var arguments;
    return arguments;
  });
  ({ foo(n = arguments) {
    var arguments;
    return arguments;
  } });
  (n) => r;
  () => r;
  async () => r;
  (n = r) => r;
  async (n = r) => r;
  (n) => r;
  () => r;
  async () => r;
  (n = r) => r;
  async (n = r) => r;
  (n) => {
    return r;
  };
  () => {
    return r;
  };
  async () => {
    return r;
  };
  (n = r) => {
    return r;
  };
  async (n = r) => {
    return r;
  };
  (n) => {
    return r;
  };
  () => {
    return r;
  };
  async () => {
    return r;
  };
  (n = r) => {
    return r;
  };
  async (n = r) => {
    return r;
  };
})();
```
### rolldown
```js

//#region entry.js
(() => {
	var arguments;
	function foo(x = arguments) {
		return arguments;
	}
	class Foo {
		foo(x = arguments) {
			return arguments;
		}
	}
	function foo(x = arguments) {
		var arguments;
		return arguments;
	}
})();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,74 +1,19 @@
-/* @__PURE__ */ (() => {
-  var r;
-  function t(n = arguments) {
-    return arguments;
-  }
-  (function(n = arguments) {
-    return arguments;
-  });
-  ({ foo(n = arguments) {
-    return arguments;
-  } });
-  class u {
-    foo(e = arguments) {
-      return arguments;
-    }
-  }
-  (class {
-    foo(n = arguments) {
-      return arguments;
-    }
-  });
-  function t(n = arguments) {
-    var arguments;
-    return arguments;
-  }
-  (function(n = arguments) {
-    var arguments;
-    return arguments;
-  });
-  ({ foo(n = arguments) {
-    var arguments;
-    return arguments;
-  } });
-  (n) => r;
-  () => r;
-  async () => r;
-  (n = r) => r;
-  async (n = r) => r;
-  (n) => r;
-  () => r;
-  async () => r;
-  (n = r) => r;
-  async (n = r) => r;
-  (n) => {
-    return r;
-  };
-  () => {
-    return r;
-  };
-  async () => {
-    return r;
-  };
-  (n = r) => {
-    return r;
-  };
-  async (n = r) => {
-    return r;
-  };
-  (n) => {
-    return r;
-  };
-  () => {
-    return r;
-  };
-  async () => {
-    return r;
-  };
-  (n = r) => {
-    return r;
-  };
-  async (n = r) => {
-    return r;
-  };
-})();
\ No newline at end of file
+
+//#region entry.js
+(() => {
+	var arguments;
+	function foo(x = arguments) {
+		return arguments;
+	}
+	class Foo {
+		foo(x = arguments) {
+			return arguments;
+		}
+	}
+	function foo(x = arguments) {
+		var arguments;
+		return arguments;
+	}
+})();
+
+//#endregion
\ No newline at end of file

```