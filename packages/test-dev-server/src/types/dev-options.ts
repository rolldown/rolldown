// `browser` mirrors Vite full-bundle mode (in-memory serving + spinner).
// `node` is a rolldown-only scenario: the artifact is executed from disk by the
// fixture harness, so it keeps disk serving. Several fixtures set `'node'` in
// their (untypechecked) dev.config.mjs.
export type Platform = 'browser' | 'node';

export interface DevOptions {
  platform?: Platform;
  port?: number;
}
