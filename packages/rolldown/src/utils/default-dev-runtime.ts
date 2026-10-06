// defined in the build step
declare const __RUNTIME_STRING__: string | undefined;

export function getDefaultDevRuntime(host = 'localhost', port = 3000): string {
  // Only `build.ts` can fill the constant. See internal-docs/dev-engine/implementation.md
  if (typeof __RUNTIME_STRING__ === 'undefined') {
    throw new Error(
      'The default dev runtime exists only in the built package. Run `just build-rolldown` and import `rolldown`, not its source.',
    );
  }
  return __RUNTIME_STRING__.replaceAll('$ADDR', `${host}:${port}`);
}
