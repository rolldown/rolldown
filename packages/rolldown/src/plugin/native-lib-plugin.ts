// Marker type for plugins backed by a native shared library that implements
// the rolldown_native_plugin_abi C ABI. The host loads the library via dlopen
// and dispatches transforms directly on its worker threads — no napi callback,
// no JS thread.

export type NativeLibPlugin = {
  _nativeLib: {
    name: string;
    path: string;
  };
};

export function defineNativeLibPlugin(opts: {
  name: string;
  path: string;
}): NativeLibPlugin {
  if (import.meta.browserBuild) {
    throw new Error('`defineNativeLibPlugin` is not supported in browser build');
  }
  return { _nativeLib: { name: opts.name, path: opts.path } };
}
