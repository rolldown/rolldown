# Memory reclaim design

See [implementation.md](./implementation.md) for how it works.

## Goal

After work finishes, native memory returns close to its live size, without slowing builds that run back to back. That must hold however late V8 collects the output that JS keeps.

## Why memory stays resident

- A free from another thread only queues the block on the page. Only the owner thread settles it, and an idle worker never runs.
- After a page returns to the arena, the purge is only scheduled. Nothing runs it unless a later allocation does.
- Each thread reuses only its own pages. Builds that run back to back spread work over the 21 tokio and 14 rayon threads differently each time, so each thread keeps the most it ever held. In the #10985 test (300 builds back to back), 2 tokio and 2 rayon threads held about 430 MB less.
- On Linux with transparent huge pages, mimalloc 3.6 purges only whole 2 MiB ranges by default, which skips most ranges. khugepaged also fills the purged parts of a range that still holds live data. See the unresolved questions.
- JS wrappers pin outputs until V8 collects them, and a small JS heap gives V8 no reason to.

## Decisions

- Settle each thread with `mi_theap_quiescent_abandon`, not `mi_collect` or `mi_thread_done`. It returns the same memory as `mi_collect(true)`, and it also gives up the pages that still hold live data. The free that later empties such a page returns it on whatever thread runs it. With `mi_collect`, that memory waited for the thread that made it, and a worker that was busy during the round kept about 75 MB of a three.js x10 test until the next round. `mi_thread_done` gives up the pages too, but it tears down the thread's allocator state, and its docs only cover a thread that ends.
- Retry a round that misses a tokio or blocking thread, after 1, 2, 4 and 8 s. A thread that was busy through the round after the builds kept about 400 MB of the three.js x10 test until anything else notified.
- Run again after finalizers. Memory JS holds at the first round cannot be freed by it. This is all a late GC needs. In the three.js x10 test, whether the GC came 0.2, 1.5, 5 or 12 s after JS dropped the output, mimalloc held about 20 MB 2 s after it. When JS had read all the code, maps and module code first, V8 kept another 150 MB of its own heap.
- Do not copy the output out of the build's heap. A copy into pages of its own gave memory back at the GC instead of about 1.5 s later, and held about 35 MB less per three.js x10 build while JS kept it. It cost 1.5% of generate time and did nothing on Windows. A payload of 64 to 512 KiB also left its original in a mimalloc page until the next round, 230 MB with 1000 chunks of 100 KB. Memory after the round was the same with or without it.
- Drop the module maps once the chunk is joined. `RenderedModule` only exposes code, so they were dead weight for as long as the output lived, about 40 MB per three.js x10 build.
- Release the pool threads after every build only when the process opts in with `ROLLDOWN_MEMORY_RECLAIM_BETWEEN_BUILDS`. Builds that run back to back never leave a quiet window. In the #10985 test (300 builds back to back), memory stayed at 615 to 620 MB instead of 860 to 920 MB on macOS, with the same build time of about 21 ms. On Linux it stayed at 650 to 780 MB instead of about 1200 MB, but each build took about 5 ms longer, about 20%. Skipping the purge or turning off THP did not change that, so the release itself costs it there. A purge after every build held 410 to 520 MB but made each build about 30% slower, because the returned pages fault in again. `mi_theap_collect(theap, false)` in place of the abandon cost nothing either, but held 750 to 780 MB.
- Keep that release off by default. Besides the Linux cost above, a Vite build of three environments in one process (client and two SSR) peaked 50 to 70 MB higher with it on macOS, about 4%, with the same build time. Frameworks that build client and server code in one `vite build` run this way. Skipping the release while a build runs, `mi_theap_collect` in place of the abandon, and `MIMALLOC_PURGE_DELAY=0` did not change it. Skipping either the blocking or the rayon part did. On Linux the same build peaked about 4% lower with it.
- While JS holds the output of several builds, a given-up page that is mostly used is not reused until more of it is freed. With the release between builds, six three.js x10 builds back to back held 1050 to 1100 MB with their output alive, against 880 to 920 MB without it. The quiet-window round alone gives the same 1030 MB once builds are 700 ms apart, and main holds about 1800 MB.
- A thread settles only after every thread has taken its task. `mi_theap_quiescent_abandon` takes a global lock, so threads settle one at a time. When each thread waited after settling, every tokio worker was held until the last one was done. With the release between builds, `close()` right after a three.js x10 build then took 7.5 to 8.5 ms instead of 2 ms.
- Purge in 64 KiB slices on Linux. mimalloc 3.6 defaults `allow_thp` to 2, which purges only whole 2 MiB ranges to keep huge pages intact, though its header and option table still say 1. After a late GC in the three.js x10 test, that held 460 to 490 MB instead of 123 to 136 MB. Any size set explicitly avoids it. Note that `MIMALLOC_ARENA_PURGE_MIN_SIZE` reads a number without a unit as bytes.
- Do not change mimalloc options at runtime. Switching `purge_delay` while idle also works but races with readers.
- Do not disable transparent huge pages. That costs about 10% build time.
- Commit pages on demand on Windows. Each thread there commits a whole 4 MiB page per size class up front, which Windows charges as commit even when untouched. On the published binaries `MIMALLOC_PAGE_COMMIT_ON_DEMAND=1` cut commit by 40 to 45% with the same build time. The value 2 does this only where the OS has no overcommit, so macOS and most Linux hosts keep eager commit.
- Settle the JS thread too. Plugin results and output wrappers are allocated there.
- Do not mark threads with `mi_thread_set_in_threadpool`. Since mimalloc 3.6.1 no thread takes another thread's page on free by default, and the mark's other effect only applies to a thread marked before its first allocation. With and without it, every three.js x10 case measured the same.

## Unresolved questions

- `mi_theap_quiescent_abandon` is in the experimental part of the mimalloc header, and upstream tests repeated calls by a live owner very little.
- The first build after a round is about 5% slower in the three.js x10 test (144 to 147 vs 139 to 143 ms), with `mi_collect` too. With purging off it is not, so the cost is the returned pages faulting in again.
- Why the release between builds raises the peak of a multi-environment Vite build on macOS.
- On Linux with transparent huge pages, idle memory grows back over minutes. mimalloc advises its arenas with `MADV_HUGEPAGE`, and khugepaged turns any 2 MiB range that still holds live data into a huge page, filling the free part with zeros. main does the same. It does not depend on the abandon, since `mi_collect(true)` grew back as much. In the three.js x10 test, idle memory went from about 120 MB to 220 to 253 MB and stayed there. Vite dev (#9330) went from 394 to about 900 MB within 10 to 15 minutes and stayed there for 40 minutes, against 2427 MB on main. With mimalloc's default 2 MiB purges, they started at 370 and 789 MB and grew to 457 and 880 MB, so 64 KiB slices are never worse. With THP off for the process, they stayed at 40 and 284 MB, but builds took 4 to 8% longer and the first build after idle about 12% longer. Measured on an arm64 VM with THP `madvise` and the default `max_ptes_none` of 511.
- Windows results are not measured yet.
