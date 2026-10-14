// Request source locations and browser priorities only. Never retain headers,
// cookies, or request bodies from CDP events.
export function recordNetworkEvidence(page) {
  const byUrl = new Map();
  const byId = new Map();
  const withoutFragment = (url) => url.split('#')[0];
  const stopRequests = page.on('Network.requestWillBeSent', (event) => {
    const initiator = event.initiator ?? {};
    let stack = initiator.stack;
    let frame;
    while (stack && !frame) {
      frame = stack.callFrames?.find((f) => f.url && f.url !== 'about:client');
      stack = stack.parent;
    }
    const row = {
      start: event.timestamp,
      redirected: Boolean(event.redirectResponse),
      initiator: {
        type: initiator.type,
        url: frame?.url ?? initiator.url ?? null,
        lineNumber: frame?.lineNumber ?? initiator.lineNumber ?? null,
        columnNumber: frame?.columnNumber ?? initiator.columnNumber ?? null,
      },
      requestPriority: {
        initial: event.request.initialPriority ?? null,
        last: event.request.initialPriority ?? null,
        changes: [],
      },
    };
    if (event.redirectResponse && byId.has(event.requestId)) {
      byId.get(event.requestId).redirected = true;
    }
    byId.set(event.requestId, row);
    const key = withoutFragment(event.request.url);
    const rows = byUrl.get(key) ?? [];
    rows.push(row);
    byUrl.set(key, rows);
  });
  const stopPriorities = page.on('Network.resourceChangedPriority', (event) => {
    const row = byId.get(event.requestId);
    if (!row) return;
    row.requestPriority.last = event.newPriority;
    row.requestPriority.changes.push({
      priority: event.newPriority,
      afterStartMs: Math.round((event.timestamp - row.start) * 1000),
    });
  });
  return {
    attach(resources, origin) {
      const keyOf = (resource) =>
        withoutFragment(
          resource.name.startsWith('/') ? `${origin}${resource.name}` : resource.name,
        );
      const counts = new Map();
      for (const resource of resources) {
        const key = keyOf(resource);
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }
      for (const resource of resources) {
        const key = keyOf(resource);
        const rows = byUrl.get(key) ?? [];
        // URL alone cannot distinguish repeated requests or redirect hops.
        // Omit ambiguous evidence instead of attaching the last request to all.
        if (rows.length !== 1 || counts.get(key) !== 1 || rows[0].redirected) {
          resource.networkEvidence = rows.length ? 'ambiguous' : 'missing';
          continue;
        }
        const row = rows[0];
        resource.networkEvidence = 'matched';
        resource.initiator = {
          ...row.initiator,
          url: row.initiator.url?.startsWith(`${origin}/`)
            ? row.initiator.url.slice(origin.length)
            : row.initiator.url,
        };
        resource.requestPriority = row.requestPriority;
      }
    },
    stop() {
      stopRequests();
      stopPriorities();
    },
  };
}
