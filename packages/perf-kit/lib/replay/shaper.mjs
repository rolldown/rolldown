// Shared bandwidth for the replay server. Every response body goes through ONE
// link, so parallel downloads share the recorded bandwidth the way they share a
// real access link. A delay per request cannot do this: it lets every parallel
// download run at full speed, and a page with 80 chunks loads too fast.
//
// Two stages, like a server on a slow link:
//   scheduler - lower urgency first (the RFC 9218 `priority` header that Chrome
//               sends, u=0 is the most urgent), first come first served within
//               one urgency. This is how a CDN picks the next DATA frame.
//   wire      - a FIFO of `wireBytes` (send buffer + bytes in flight) that drains
//               at the link rate. Response headers join its tail and wait behind
//               what is already there: on a busy slow link a response's first
//               byte comes late because of the page's OWN downloads. The body
//               follows its headers into the scheduler at once, so a response
//               waits for the wire one time, not once for the headers and again
//               for the body. With wireBytes 0 the wire holds one tick of data.

/** `u=1, i` -> 1. Chrome's default for a request without the header is u=3. */
export function urgencyOf(priorityHeader) {
  const m = /(?:^|[,\s])u=([0-7])/.exec(priorityHeader ?? '');
  return m ? Number(m[1]) : 3;
}

const SLICE = 16_384;

export function createShaper({
  bytesPerSec,
  wireBytes = 0,
  tickMs = 5,
  now = () => performance.now(),
  schedule = (fn, ms) => setTimeout(() => fn(), ms),
}) {
  const unlimited = !(bytesPerSec > 0) || bytesPerSec === Infinity;
  const queue = [];
  const wire = []; // { bytes, sent, chunk, write } or a marker { bytes: 0, fire }
  let wireLoad = 0;
  let seq = 0;
  let credit = 0;
  let last = now();
  let timer = null;
  let closed = false;

  const idle = () => !queue.length && !wire.length;

  // Scheduler -> wire, up to the window (or up to what this tick can send).
  function fill(limit) {
    queue.sort((a, b) => a.urgency - b.urgency || a.seq - b.seq);
    while (queue.length && wireLoad < limit) {
      const item = queue[0];
      const n = Math.min(item.data.length - item.offset, limit - wireLoad, SLICE);
      if (n > 0) {
        wire.push({
          bytes: n,
          sent: 0,
          chunk: item.data.subarray(item.offset, item.offset + n),
          write: item.write,
        });
        wireLoad += n;
        item.offset += n;
      }
      if (item.offset >= item.data.length) {
        queue.shift();
        wire.push({
          bytes: 0,
          fire: () => {
            item.end();
            item.resolve();
          },
        });
      }
    }
  }

  // Wire -> sockets, in FIFO order, at the link rate.
  function drain() {
    while (wire.length) {
      const head = wire[0];
      if (head.bytes === 0) {
        wire.shift();
        head.fire();
        continue;
      }
      if (credit < 1) break;
      const n = Math.min(head.bytes - head.sent, Math.floor(credit));
      head.write(head.chunk.subarray(head.sent, head.sent + n));
      head.sent += n;
      credit -= n;
      wireLoad -= n;
      if (head.sent >= head.bytes) wire.shift();
    }
  }

  function pump() {
    timer = null;
    if (closed) return;
    const t = now();
    // The link was busy for all of this time, so all of it is usable: a late timer
    // must not lose bandwidth. Idle time never banks a burst: start() drops it.
    credit += ((t - last) * bytesPerSec) / 1000;
    last = t;
    fill(Math.max(wireBytes, Math.floor(credit)));
    drain();
    if (!idle()) timer = schedule(pump, tickMs);
  }

  function start() {
    if (idle() && !timer) {
      // Start the clock now: credit from the idle time before is not usable.
      last = now();
      credit = 0;
    }
  }

  return {
    /**
     * Sends `data` through the shared link; resolves after `end()` is called.
     * `head()` writes the response headers: they join the wire's tail now, and
     * the body enters the scheduler behind them.
     */
    send({ data, urgency = 3, write, end, head = null }) {
      if (unlimited || data.length === 0) {
        head?.();
        if (data.length) write(data);
        end();
        return Promise.resolve();
      }
      return new Promise((resolve) => {
        start();
        if (head && wire.length) wire.push({ bytes: 0, fire: head });
        else head?.();
        queue.push({ data, offset: 0, urgency, seq: seq++, write, end, resolve });
        if (!timer) timer = schedule(pump, 0);
      });
    },
    /** Resolves when response headers sent now would reach the client: behind the wire's content. */
    header() {
      if (unlimited || !wire.length) return Promise.resolve();
      return new Promise((resolve) => wire.push({ bytes: 0, fire: resolve }));
    },
    /** Drops a stream the client cancelled. */
    cancel(write) {
      const i = queue.findIndex((item) => item.write === write);
      if (i !== -1) queue.splice(i, 1)[0].resolve();
      for (let j = wire.length - 1; j >= 0; j--) {
        if (wire[j].write === write) {
          wireLoad -= wire[j].bytes - wire[j].sent;
          wire.splice(j, 1);
        }
      }
    },
    pending: () => queue.length + wire.length,
    close() {
      closed = true;
      for (const item of queue.splice(0)) item.resolve();
      for (const entry of wire.splice(0)) if (entry.bytes === 0) entry.fire();
    },
  };
}
