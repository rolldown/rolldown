import type { DevServerHandle, ServeContext } from '~utils';

// Shared by every spec in this playground: create the server but do NOT
// navigate. The lazy bugs only show on the very first interaction with a
// fresh server, so each spec does its own `page.goto`.
export async function serve(ctx: ServeContext): Promise<DevServerHandle> {
  return ctx.createServer();
}
