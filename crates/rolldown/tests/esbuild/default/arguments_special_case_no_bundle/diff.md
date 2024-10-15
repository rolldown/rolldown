# Reason
1. related to minifier
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
	(function(x = arguments) {
		return arguments;
	});
	({ foo(x = arguments) {
		return arguments;
	} });
	class Foo {
		foo(x = arguments) {
			return arguments;
		}
	}
	(class {
		foo(x = arguments) {
			return arguments;
		}
	});
	function foo(x = arguments) {
		var arguments$1;
		return arguments$1;
	}
	(function(x = arguments) {
		var arguments$1;
		return arguments$1;
	});
	({ foo(x = arguments) {
		var arguments$1;
		return arguments$1;
	} });
	(x) => arguments;
	() => arguments;
	async () => arguments;
	(x = arguments) => arguments;
	async (x = arguments) => arguments;
	(x) => arguments;
	() => arguments;
	async () => arguments;
	(x = arguments) => arguments;
	async (x = arguments) => arguments;
	(x) => {
		return arguments;
	};
	() => {
		return arguments;
	};
	async () => {
		return arguments;
	};
	(x = arguments) => {
		return arguments;
	};
	async (x = arguments) => {
		return arguments;
	};
	(x) => {
		return arguments;
	};
	() => {
		return arguments;
	};
	async () => {
		return arguments;
	};
	(x = arguments) => {
		return arguments;
	};
	async (x = arguments) => {
		return arguments;
	};
})();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,74 +1,78 @@
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
+	(function(x = arguments) {
+		return arguments;
+	});
+	({ foo(x = arguments) {
+		return arguments;
+	} });
+	class Foo {
+		foo(x = arguments) {
+			return arguments;
+		}
+	}
+	(class {
+		foo(x = arguments) {
+			return arguments;
+		}
+	});
+	function foo(x = arguments) {
+		var arguments$1;
+		return arguments$1;
+	}
+	(function(x = arguments) {
+		var arguments$1;
+		return arguments$1;
+	});
+	({ foo(x = arguments) {
+		var arguments$1;
+		return arguments$1;
+	} });
+	(x) => arguments;
+	() => arguments;
+	async () => arguments;
+	(x = arguments) => arguments;
+	async (x = arguments) => arguments;
+	(x) => arguments;
+	() => arguments;
+	async () => arguments;
+	(x = arguments) => arguments;
+	async (x = arguments) => arguments;
+	(x) => {
+		return arguments;
+	};
+	() => {
+		return arguments;
+	};
+	async () => {
+		return arguments;
+	};
+	(x = arguments) => {
+		return arguments;
+	};
+	async (x = arguments) => {
+		return arguments;
+	};
+	(x) => {
+		return arguments;
+	};
+	() => {
+		return arguments;
+	};
+	async () => {
+		return arguments;
+	};
+	(x = arguments) => {
+		return arguments;
+	};
+	async (x = arguments) => {
+		return arguments;
+	};
+})();
+
+//#endregion
\ No newline at end of file

```