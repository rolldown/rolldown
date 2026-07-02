import type { AddressInfo } from 'node:net';
import nodeNet from 'node:net';
import type { InlineConfig, Logger as ViteLogger, Plugin as VitePlugin, ViteDevServer } from 'vite';
import { createServer } from 'vite';
import type { DevServerHandle } from './dev-server.js';
import type { Logger } from './types/logger.js';
import type { DevConfig } from './utils/define-dev-config.js';

/**
 * Browser-platform dev server backed by Vite's full bundle mode
 * (`experimental.bundledDev`).
 *
 * The previous implementation was a hand-written port of Vite's `BundledDev`
 * (in-memory serving, HMR fan-out, lazy-bundling endpoint, error overlay,
 * fallback spinner). All of that now comes from Vite itself — the vendored
 * submodule at `packages/test-dev-server/vite` resolves `rolldown` to the
 * workspace's `packages/rolldown` via a node_modules symlink swap (see
 * `scripts/setup-vite.mjs`; the submodule itself stays pristine), so running
 * these tests exercises the local rolldown binding through the real Vite
 * integration instead of a parallel re-implementation.
 *
 * What this file adds on top of Vite is only the test-harness surface the
 * specs rely on and Vite doesn't provide:
 * - the `/_dev/status` endpoint (`buildSeq` / `moduleRegistrationSeq` /
 *   engine bundle state) used by `waitForBuildStable` and friends,
 * - the `Logger` → Vite `customLogger` adapter so `serverLogs` capture works,
 * - the old `createDevServer` contract that a resolved promise means the
 *   initial build has settled (Vite's `listen()` kicks the first build off
 *   without awaiting it).
 */

interface HarnessCounters {
  /**
   * Bumped when build activity is observed: a `buildStart` from the rolldown
   * pipeline (fires for every full (re)build, including ones that later fail)
   * and every broadcast `update` / `full-reload` hot payload (completion-side
   * signal). Deliberately NOT bumped for `error` payloads: the server replays
   * the cached build error to every newly-connected client, and the specs
   * assert that a page refresh on a failed build does not move `buildSeq`
   * (conservative rebuilds — dev-engine design principle 1).
   */
  buildSeq: number;
  /** Bumped per `vite:module-loaded` report from the client runtime. */
  moduleRegistrationSeq: number;
}

/**
 * Reserve an OS-assigned port. Vite treats `port: 0` as "use the default
 * (5173, auto-incrementing)", so the old harness contract — every server gets
 * its own OS-assigned port, letting specs run in parallel — needs the port
 * picked before Vite sees it. The tiny bind→close→rebind window is racy in
 * principle; `strictPort: false` lets Vite walk forward if it ever loses it.
 */
async function getFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = nodeNet.createServer();
    probe.once('error', reject);
    probe.listen(0, () => {
      const port = (probe.address() as AddressInfo).port;
      probe.close(() => resolve(port));
    });
  });
}

/** Adapt the harness logger to Vite's `customLogger` interface. */
function toViteLogger(logger: Logger): ViteLogger {
  const warnedMessages = new Set<string>();
  const viteLogger: ViteLogger = {
    hasWarned: false,
    info(msg) {
      logger.info(msg);
    },
    warn(msg) {
      viteLogger.hasWarned = true;
      logger.warn(msg);
    },
    warnOnce(msg) {
      if (warnedMessages.has(msg)) return;
      warnedMessages.add(msg);
      viteLogger.hasWarned = true;
      logger.warn(msg);
    },
    error(msg) {
      logger.error(msg);
    },
    clearScreen() {},
    hasErrorLogged() {
      return false;
    },
  };
  return viteLogger;
}

/**
 * Test-only instrumentation plugin: owns the `/_dev/status` endpoint and the
 * counters behind it. The bundle state (`hasStaleOutput` / `lastBuildErrored`)
 * is read live from the dev engine owned by Vite's `BundledDev` — reached via
 * the public `environments.client.bundledDev` field (its `devEngine` member is
 * `private` in TS only, so a cast is the whole integration cost).
 */
function createHarnessPlugin(counters: HarnessCounters): VitePlugin {
  // Whether the last *full build* errored, tracked by observing broadcast
  // `error` payloads. Backs the two error-recovery workarounds below — see
  // the `configureServer`/`generateBundle` comments. Vite's own bundled-dev
  // state (`BundledDev.lastBuildError`) is `private`, so the workarounds
  // reach it with a cast; the vendored submodule pins the Vite version, so
  // this stays stable until the submodule is bumped.
  let sawBroadcastError = false;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let getBundledDev: () => any = () => undefined;
  let sendFullReload: () => void = () => {};

  return {
    name: 'test-dev-server:harness',

    buildStart() {
      counters.buildSeq++;
    },

    // Runs only when bundle generation succeeded (this plugin is appended
    // last, so a user plugin throwing in `generateBundle` skips it).
    //
    // WORKAROUND (upstream gap): Vite's bundled dev only full-reloads after a
    // successful build when a reload was already pending from HMR. A build
    // that recovers from an *error* state sends nothing — clients stuck on
    // the error overlay or the "Bundling in progress" fallback page (whose
    // one-time ready signal was consumed by the failed initial build) never
    // learn the build is healthy again. Reload them here, and drop the cached
    // error so reconnecting clients don't get it replayed.
    generateBundle() {
      if (!sawBroadcastError) return;
      sawBroadcastError = false;
      const bundledDev = getBundledDev();
      if (!bundledDev) return;
      bundledDev.lastBuildError = null;
      // Fire-and-forget: `ensureLatestBuildOutput()` waits for the build this
      // very hook is part of, so awaiting it here would deadlock.
      void bundledDev.devEngine.ensureLatestBuildOutput().then(sendFullReload);
    },

    configureServer(server) {
      const clientEnv = server.environments.client;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      getBundledDev = () => (clientEnv as any).bundledDev;
      sendFullReload = () => clientEnv.hot.send({ type: 'full-reload', path: '*' });

      clientEnv.hot.on('vite:module-loaded', () => {
        counters.moduleRegistrationSeq++;
      });

      // WORKAROUND (upstream gap): Vite never clears `lastBuildError` when a
      // recovery arrives as an HMR patch (only `onOutput` clears it), and the
      // client hard-reloads when its first update meets an existing error
      // overlay — so the reconnect can get a stale error replayed and the
      // overlay never clears. This listener registers before Vite's own
      // `vite:client:connect` replay listener (configureServer runs before
      // `listen()`), so dropping the stale error here wins the race. A live
      // error (`sawBroadcastError`) is deliberately kept — replaying it to
      // fresh clients is correct behavior the specs assert.
      clientEnv.hot.on('vite:client:connect', () => {
        const bundledDev = getBundledDev();
        if (!sawBroadcastError && bundledDev?.lastBuildError) {
          bundledDev.lastBuildError = null;
        }
      });

      // Observe broadcast payloads: `update` / `full-reload` mark build
      // activity for `buildSeq`; `error` marks the errored state for the
      // recovery workarounds. Per-client payloads (HMR patches, error
      // replays) intentionally don't count — see `HarnessCounters.buildSeq`.
      const originalSend = clientEnv.hot.send.bind(clientEnv.hot);
      clientEnv.hot.send = ((...args: unknown[]) => {
        const payload = args[0];
        if (typeof payload === 'object' && payload !== null && 'type' in payload) {
          if (payload.type === 'update' || payload.type === 'full-reload') {
            counters.buildSeq++;
          } else if (payload.type === 'error') {
            sawBroadcastError = true;
          }
        }
        return (originalSend as (...a: unknown[]) => void)(...args);
      }) as typeof clientEnv.hot.send;

      server.middlewares.use(async (req, res, next) => {
        if (req.url?.split('?')[0] !== '/_dev/status') {
          next();
          return;
        }
        let hasStaleOutput = false;
        let lastBuildErrored = false;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const devEngine = (clientEnv as any).bundledDev?.devEngine;
        if (devEngine) {
          try {
            const state = await devEngine.getBundleState();
            hasStaleOutput = state.hasStaleOutput;
            lastBuildErrored = state.lastBuildErrored;
          } catch {
            // Engine mid-teardown; report defaults.
          }
        }
        res.setHeader('Content-Type', 'application/json');
        res.end(
          JSON.stringify({
            hasStaleOutput,
            lastBuildErrored,
            buildSeq: counters.buildSeq,
            connectedClients: server.ws.clients.size,
            moduleRegistrationSeq: counters.moduleRegistrationSeq,
          }),
        );
      });
    },
  };
}

/** Translate the harness `DevConfig` into a Vite inline config. */
function toViteConfig(
  config: DevConfig,
  counters: HarnessCounters,
  port: number,
  opts?: { logger?: Logger },
): InlineConfig {
  const build = config.build ?? {};
  if (build.plugins != null && !Array.isArray(build.plugins)) {
    throw new Error('Plugins must be an array');
  }

  return {
    // The harness injects the playground copy as `build.cwd`; it becomes the
    // Vite root, and the playground's `index.html` becomes the build input
    // (so the fixture `input: { main: … }` is dropped — Vite's html pipeline
    // discovers the entry from the module script tag).
    root: build.cwd ?? process.cwd(),
    configFile: false,
    envDir: false,
    clearScreen: false,
    logLevel: 'info',
    customLogger: opts?.logger ? toViteLogger(opts.logger) : undefined,
    server: {
      port,
      host: 'localhost',
      open: false,
    },
    experimental: {
      bundledDev: true,
    },
    // Vite 8 runs rolldown natively, so the fixtures' rolldown-style plugins
    // (transform/generateBundle hooks, rolldown builtin plugins) pass through
    // to the bundled-dev build as-is.
    plugins: [...((build.plugins ?? []) as VitePlugin[]), createHarnessPlugin(counters)],
    build: {
      // Parity with the raw-rolldown harness, which never inlined assets: the
      // asset specs assert real asset requests. Same setting Vite's own
      // full-bundle-mode playground uses.
      assetsInlineLimit: 0,
      rollupOptions:
        build.treeshake !== undefined
          ? // eslint-disable-next-line @typescript-eslint/no-explicit-any
            { treeshake: build.treeshake as any }
          : {},
    },
  };
}

/**
 * Start a Vite bundled-dev server for a browser-platform `DevConfig` and wrap
 * it in the harness `DevServerHandle` contract.
 */
export async function createViteDevServer(
  config: DevConfig,
  opts?: { port?: number; logger?: Logger },
): Promise<DevServerHandle> {
  const logger = opts?.logger ?? console;
  const counters: HarnessCounters = { buildSeq: 0, moduleRegistrationSeq: 0 };
  const port = opts?.port || (await getFreePort());

  let server: ViteDevServer;
  try {
    server = await createServer(toViteConfig(config, counters, port, opts));
  } catch (e) {
    logger.error('Failed to create Vite dev server:', e);
    throw e;
  }

  try {
    await server.listen();
    // Old harness contract: a resolved `createDevServer` means the initial
    // build (or its error) has settled, so a navigation right after never
    // races the first bundle. Vite's `listen()` fires the build without
    // awaiting it; wait on the engine directly. A build *error* must not
    // reject here — it is served via the fallback page + error overlay.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const devEngine = (server.environments.client as any).bundledDev?.devEngine;
    await devEngine?.ensureCurrentBuildFinish?.().catch(() => {});
  } catch (e) {
    await server.close().catch(() => {});
    throw e;
  }

  const boundPort = (server.httpServer!.address() as AddressInfo).port;
  logger.info(`Server listening on http://localhost:${boundPort}`);

  return {
    url: `http://localhost:${boundPort}`,
    port: boundPort,
    close: () => server.close(),
  };
}
