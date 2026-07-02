import type { PreparedError } from '../utils/prepare-error.js';

export interface HmrUpdateMessage {
  type: 'hmr:update';
  url: string;
  path: string;
}

export interface ConnectedMessage {
  type: 'connected';
}

/**
 * A build error is pushed to every connected client and replayed to
 * freshly-connected ones, so the error survives a client restart.
 * See `internal-docs/dev-engine/design.md` — principle 2.
 *
 * `err` is the ANSI-stripped, serializable payload produced by `prepareError`,
 * mirroring the `err` field of Vite's `ErrorPayload`.
 */
export interface ErrorMessage {
  type: 'error';
  err: PreparedError;
}

/**
 * Broadcast when a build recovers (errored → ok). HMR patches are delivered
 * per-client (only to the client that registered the changed module), so this
 * broadcast is how every other client learns the build is healthy again.
 */
export interface BuildOkMessage {
  type: 'build:ok';
}
