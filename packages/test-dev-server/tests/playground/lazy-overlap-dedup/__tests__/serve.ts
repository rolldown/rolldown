import type { DevServerHandle, ServeContext } from '~utils';

// Create the server but do NOT navigate: the ship-map-driven dedup is about the
// very first deliveries to a fresh client, so the spec does its own
// `page.goto` and controls the click order.
export async function serve(ctx: ServeContext): Promise<DevServerHandle> {
  return ctx.createServer();
}
