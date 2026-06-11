import type { DevServerHandle, ServeContext } from '~utils';

// Cold-start escape hatch (meta/design/dev-server-test-harness.md), shared by
// every scenario spec in this playground: create the server but DON'T navigate.
// Each lazy scenario only reproduces on the first interaction with a virgin
// server, so each spec fires its own `page.goto` (and basic counts the resulting
// requests). One per-file server is virgin for every scenario because a lazy
// chunk is compiled only when its own dynamic import fires.
export async function serve(ctx: ServeContext): Promise<DevServerHandle> {
  return ctx.createServer();
}
