# Rolldown + 持久化 Cache 之后的内存问题与路线探索

> 日期：2026-10-31。基于 rolldown `main`（0960acb4）、h-a-n-a/cache `main` 与 PR #1/#2/#4/#5、rolldown issue #9516 / #9330 / #11189 / #10984 / #6995 / PR #10554。
> **未能读取 microsoft/developer-bundler**：当前会话凭证返回 404（私有仓库或已改名），公开搜索也无结果。本文对它的讨论只能基于"开发者 bundler"这一类设计的通用形态，第 5 节给出了评估框架，拿到设计稿后可以逐条对照。

---

## 0. 一句话结论

- **h-a-n-a/cache 当前的四个里程碑完全没有把内存当目标**，M1 的"把现有 `ScanStageCache` 序列化到磁盘"如果照直做，不会减少常驻内存，反而会在加载期提高峰值。
- 但持久化 cache 是解决内存问题的**前置条件**：一旦 scan 产物可以从磁盘/源码廉价重建，内存里的 AST 就可以从"必须常驻的状态"降级为"可逐出的派生物"。**正确的组合不是"cache + 内存优化"两条线，而是同一个原语：把 AST 从缓存内容中剔除，只缓存 transformed code + 轻量元数据，AST 按需重 parse。**
- 现有架构里最大的两个内存倍增器都和 cache 设计直接相关：
  1. `ScanStageCache.snapshot.index_ecma_ast` 让每个模块的 oxc arena（bench 里占峰值 62%）在 dev/incremental 模式下永久常驻；
  2. 每次增量构建 `create_output()` 会把**全部模块的 AST** `clone_with_another_arena` 一遍（`crates/rolldown/src/types/scan_stage_cache.rs:338`），构建峰值 ≈ 2× AST。
- 是否要走"developer bundler"这条独立路线：从 Rolldown 的现状看，`rolldown_dev`（FBM + HMR + lazy compilation + 增量 scan）**已经是一个 developer bundler 的内核**，缺的是持久化、内存模型和（可能的）进程模型。除非 developer-bundler 的设计提供了 Rolldown 结构上做不到的东西（第 5 节列了判定标准），否则另起炉灶的成本大于收益。

---

## 1. Rolldown 现在的内存到底花在哪

### 1.1 常驻部分（dev / incremental 模式，构建之间）

`Bundler` 跨构建持有 `ScanStageCache`（见 `internal-docs/cache/implementation.md`、`internal-docs/bundler-data-lifecycle/implementation.md`）：

| 数据 | 位置 | 为什么常驻 | 内存量级 |
| --- | --- | --- | --- |
| 每模块完整 AST（oxc bumpalo arena） | `snapshot.index_ecma_ast` | 增量 link 不重 parse；HMR patch / lazy chunk 渲染要 clone 它 | **峰值的 ~62%**（rome：87 MiB / 1193 模块，~75 KiB/模块；three.js：~2 MiB/模块，760 MiB）— #9516 |
| `Scoping`（oxc_semantic 符号表） | `snapshot.symbol_ref_db[*].ast_scopes` | link/rename 需要 | ~10%（rome 13.7 MiB，`Scoping::reserve` 过度预留） |
| `EcmaView` + `NormalModule`（import records、各种 FxHashMap、`source: ArcStr`） | `snapshot.module_table` | 增量 link 的输入 | ~4%，`NormalModule` 768 B + 哈希表 |
| `stmt_infos` | `snapshot.stmt_infos` | tree-shaking 输入 | ~1% |
| `ModuleInfo.code: ArcStr`（transform 后代码） | `PluginDriverFactory.module_infos_for_incremental_build` | `this.getModuleInfo()` | 等于 transformed source 总量，和 `EcmaView.source` 是同一 ArcStr 则不重复 |
| `transform_dependencies` / `importers` / 各索引 map | `ScanStageCache` | 失效追踪 | 小 |

关键点：**link 阶段真正需要的是右边三行（符号表、EcmaView、stmt_infos）；AST 只在 generate（finalize + codegen）和 HMR 渲染时需要。** 但它是最大的一块，而且是不可压缩的 arena 指针结构，既不能序列化也不能 mmap。

### 1.2 构建期峰值（每次 rebuild 额外叠加的部分）

`crates/rolldown/src/bundle/bundle.rs:292-297`：

```
全量 scan:   cache.set_snapshot(output.make_copy())    // 全量 clone 一份进 cache
增量 scan:   cache.merge(output); cache.create_output() // 从 cache 全量 clone 一份出来
```

`make_copy` / `create_output` 都对 **每一个模块** 调 `EcmaAst::clone_with_another_arena`（深拷贝整棵树到新 arena），因为 `finalize_modules` 会原地改写 AST（`finalize_modules.rs:59` 的 `par_iter_mut_enumerated`）。所以：

```
构建峰值 ≈ 常驻 snapshot (1× AST + 1× Scoping)
         + 工作副本     (1× AST + 1× Scoping 的 clone_without_scoping 部分)
         + link 元数据  (metas, chunk graph …)
         + 输出字符串 + sourcemap（#9516：rome 有 ~33 MiB 在 15 个 ≥1 MiB 的大 buffer 里）
         + minify 重 parse 的新 arena
```

编辑一个文件，哪怕只有一个模块变了，仍然要把整个图的 AST 复制一遍。这是 `rome` 规模以上项目 rebuild 内存抖动的主因，也是 #11189（"每次 rebuild RSS 增长直到 full GC"）里 mimalloc 看到的大块分配/释放模式的来源。

### 1.3 已知的泄漏/保留类问题（与 cache 无关，但会被 cache 放大）

- #10984：`BindingBundler.bundle_handles` 保留每次 build 的 `PluginDriver` → `DashMap<ArcStr, Arc<ModuleInfo>>` → 所有模块源码，直到 `close()`。
- #11189：bundled dev 下每次 rebuild 增长，只有 V8 full GC 才回收，说明有 native 内存挂在 JS 对象（napi external / `BindingOutputs`）上，V8 感知不到压力。
- #9330 的后续数据：mimalloc 3 的 purge/retain 策略 + 27 个 `rolldown-worker` 线程各自的 heap，让 RSS 在 burst 之后不回落（`MIMALLOC_PURGE_DELAY=0` 和缩线程池各省 ~150 MB）。
- #10554（已合）：finalize 阶段同时物化所有 chunk 的传递闭包，二次方增长。

这些是 L0 级别的修复，不需要任何 cache 设计，但如果不先修，cache 加进来后会被归咎于 cache。

---

## 2. h-a-n-a/cache 的设计实际覆盖了什么

仓库只有文档，没有代码。四个里程碑（PR #2，仍 open）：

| 里程碑 | 内容 | 对内存的影响 |
| --- | --- | --- |
| M1 持久化 scan cache | 把现有 `ScanStageCache` 序列化到磁盘；重启后复用；增量写；损坏/版本不兼容时降级 | **零或负**：加载时需要反序列化 + 重建 AST；如果序列化 AST 本身，磁盘体积和加载峰值都很差 |
| M2 跨 worktree / 机器共享 | project-local + project-global 两级目录；路径重写；CI 上传下载 | 省 CPU/磁盘，不省内存；多个 dev server 实例各自一份内存 |
| M3 插件计算缓存粒度 | 整模块 vs 单 hook vs 插件给 cacheKey；JS↔Rust 低通信的 key 计算 | 与内存无关 |
| M4 link / generate 增量 | 先测量再定范围 | 如果做成"保留 link 产物跨构建复用"，内存会再涨一层 |

`goals.md` 四条目标：减少重复计算、跨运行/目录复用、自动保证正确、损坏可恢复。`principles.md` 两条：正确性优先、省的必须比花的多。**没有任何一条提到内存。** M1 的 deliverable 里"Choose which scan data to save based on performance measurements"是唯一能把内存塞进去的口子。

值得肯定的设计约束（和内存路线兼容）：
- M1 明确"不改变现有增量模型"，所以它必须接受 `ScanStageCache` 的 module-level 粒度（`ModuleIdx` 永不复用、dense IndexVec 等不变量）。
- scratchpad 里关于路径的讨论（存 project-root 相对路径、restore 时重写）已经是正确方向。
- 原则 2 的"1 ms 计算 vs 3 ms 读缓存就不要缓存"直接适用于"要不要缓存 AST"：oxc parse 一个 75 KiB arena 的模块大约 0.1–0.3 ms，并行 1000 个模块几十 ms；反序列化 + 修指针不会比这快多少，而内存/磁盘代价大得多。**结论：AST 不应该是缓存的内容。**

---

## 3. 加上 cache 之后如何解决内存：Cache-as-backing-store

核心思想：**内存里只保留 working set，cache（磁盘 + 可重算）是 backing store。** 分四层，前两层不依赖 cache 落地，可以先做。

### L0 止血（不改架构）

1. `bundle_handles` 只保留 timing 数字（#10984 里已经给出 fix 方案）。
2. dev engine 每次 rebuild 后主动归还 mimalloc 页（`mi_collect` / purge delay），并且把 `rolldown-worker` 线程数在 dev 长进程里收敛（#9330 评论里的数据：两项合计约 −30%）。
3. napi 侧给 `BindingOutputs` 等 external 正确上报 `adjustExternalMemory`，让 V8 感知 native 压力（解决 #11189 "只有 full GC 才回收"）。

### L1 去掉"每次构建全量 clone AST"

这是现有架构里最便宜、收益最大的一刀。三个可选实现，按改动从小到大：

- **a. 按 chunk 粒度延迟 clone 并即时释放。** `finalize_modules` 已经按模块并行；改成"从 snapshot 借 `&EcmaAst` → clone → finalize → codegen → drop"在同一个 rayon task 里完成，而不是先把整张表 clone 出来再处理。峰值从 2× AST 降到 1× AST + 并行度 × 单模块。这与 #9516 Tier B-5 "Drop AST per-module during codegen" 是同一件事。
- **b. 只 clone 被 include 的模块。** `finalize_modules` 的 filter 已经跳过 `!is_included` 的模块，但 `create_output` 没有，tree-shake 掉的模块白复制了。
- **c. finalize 不改 AST，改成 edit list。** 这是 esbuild 的模型：printer 直接读只读 AST + 符号重命名表输出代码。Rolldown 已经有 `string_wizard` 和 `EcmaView.mutations: Vec<ArcSourceMutation>` 的雏形；全部改过去工程量很大（`module_finalizers` 是最复杂的模块之一），但做到之后 AST 成为完全只读、可共享的数据，HMR 渲染也不再 clone。建议作为 M4 的研究项，而不是短期目标。

### L2 AST 逐出（真正把常驻降一个数量级）

把 `index_ecma_ast` 从 "必须 `Some`" 改成 `enum { Resident(EcmaAst), Evicted }`，并定义唯一的恢复路径：

```
恢复(module) = parse(transformed_code[module]) → run AstScanner → 得到 AST + NodeId 对齐的 side tables
```

需要满足的条件（都已经有基础）：

1. **transformed code 必须保留**。`EcmaView.source` 已经是 transform hook 之后的源码；但 `transform_ast` hook（`crates/rolldown/src/utils/parse_to_ecma_ast.rs:107`）在 parse 之后改 AST，scan 看到的是改过的树。所以 eviction 的"源"要么是 `transform_ast` 之后 codegen 出来的字符串（首次多一次 codegen，但只在要逐出时做），要么重跑 `transform_ast`（Rust 内置插件便宜，JS 插件贵）。前者更稳。
2. **NodeId / SymbolId 可重现**。`internal-docs/ast-mutation` 把 NodeId 当跨 pass 的身份；oxc parser + semantic 对同一字符串是确定性的，所以"codegen → 再 parse"得到的 id 和首次相同的前提是 scanner 在 **重 parse 后的树** 上跑过，也就是说逐出时要重跑 `AstScanner`（纯 Rust，rome 全量几十 ms 级），或者干脆统一成"scan 之前总是 codegen+reparse 一次"来保证 canonical form。这是一个需要 benchmark 决定的点，但不是阻塞点。
3. **HMR 和 lazy compile 要走恢复路径**。现在 `hmr_stage.rs:276/378/720/855` 直接从 snapshot clone；改为 `cache.ast(idx)` 这种会按需恢复的访问器即可。HMR 只碰变更模块及其 patch 范围，恢复开销可忽略。
4. **逐出策略**。最简单：构建完成后逐出所有 AST（常驻 = 元数据 + transformed code）；下一次构建 generate 阶段按 chunk 恢复。进一步：LRU 保留最近 HMR 触碰过的模块。

预期效果（按 #9516 的比例外推）：rome 常驻从 ~110 MiB 降到 ~25–30 MiB；three.js 从 760 MiB 降到 ~100 MiB 以内；像 #9330 那种数万模块的 monorepo 从 GB 级降到数百 MB。构建峰值由 L1 决定，不再和图规模成 2× 关系。

**这和 cache M1 的关系**：M1 要决定"存哪些 scan 数据"。如果存的是 `transformed code + EcmaView/stmt_infos/symbol 元数据`（而不是 AST），那么：
- 磁盘格式就是 L2 的 Evicted 状态，冷启动 = 全部模块处于 Evicted，和热逐出走同一条恢复路径，没有两套代码；
- 元数据部分是 plain Rust 结构（IndexVec、FxHashMap、CompactStr），可以用 rkyv 之类做 mmap/零拷贝，让 M2 的 global cache 在多个进程间经由 OS page cache 共享物理内存——这是**不引入 daemon 就能跨进程共享内存**的唯一办法；
- Scoping 也不要序列化，由 semantic 重建（它本来就是 parse 的副产物）。

### L3 generate 阶段流式化

#9516 Tier B-4：chunk 输出和 sourcemap 边产出边写/边交给 JS，而不是攒到 `finalize_assets`。对内存峰值的收益在 rome 上约 19%，对 WXT/Cloudflare Docs 那种"输出 100+ MiB inline sourcemap"的场景是决定性的（#9330 评论：sourcemap 占输出 70%）。这与 cache 无关，但与 "dev server 把输出留在内存里给浏览器" 的模式相关，应该一起设计。

---

## 4. 缓存复用的 key 与粒度建议（对 M1/M3 的具体意见）

- **单位**：`ModuleId`（resolved path + query）对应一条记录，这是 `ScanStageCache` 的粒度，M1 不该改。
- **key**：`hash(transformed_code)` 不够，因为 transformed code 是输出不是输入。正确的 key 是 `hash(原始文件内容, resolve 结果, 影响该模块的插件链 cacheKey, rolldown 版本, 相关 options)`；`transform_dependencies`（`addWatchFile`）里的文件也要进 key。M3 的"插件提供 cacheKey"正是为了让 JS 插件不用每次被调用就能判定命中——这是 cache 真正能跳过 JS 工作的前提（goal 1 的 check 项）。
- **value**：`transformed_code`（必要）+ scan 元数据（可选，可从 code 重算，存不存看 M1 的 benchmark）+ `ModuleInfo` 里插件写入的 `meta`。**不存 AST、不存 Scoping。**
- **路径**：value 里所有 `ModuleId` 存 project-root 相对形式，恢复时拼回。绝对路径出现在插件生成的代码里时，该记录标记为 `path_dependent`，只进 local cache 不进 global（scratchpad 的方案已经对）。
- **一致性**：M1 的"增量写"要和 `revert_partial_scan` 的语义一致：失败的 partial scan 不能把半成品写进磁盘，否则重启后 `pending_rescans` 丢失。建议磁盘 cache 只在 `merge` 成功之后、以整次构建为事务写入。
- **建议新增一条 principle**：*"缓存不得增加稳态常驻内存；缓存内容应允许替代内存中的等价数据。"* 没有这条，M1 很容易做成"磁盘上多一份、内存里一份不少"。

---

## 5. developer-bundler 该不该成为未来路线

读不到设计稿，只能给判定框架。"developer bundler" 这一类设计（面向开发期而非产物的 bundler）通常由四个部件组成，Rolldown 对应的现状：

| 部件 | Rolldown 现状 | 差距 |
| --- | --- | --- |
| 按需 / 懒编译 | `rolldown_plugin_lazy_compilation` + FBM，动态 import 边界 | 已有，静态 import 子图仍全量 |
| 增量图 + HMR | `ScanStageCache` + `rolldown_dev` + 客户端 HMR | 已有，粒度为模块级 |
| 持久化 / 共享缓存 | 无，h-a-n-a/cache 在规划 | M1–M3 |
| 长驻进程 / daemon、多客户端共享一份图 | 无，bundler 跟随 Vite 的 node 进程 | 这是唯一 Rolldown 结构上没有的东西 |

判定标准——如果 developer-bundler 的设计满足下面任意两条，它值得作为 Rolldown 之上的产品层（而不是替代品）：

1. 它定义了一个**与 Rolldown 内部图解耦的持久化格式**（例如 content-addressed 的 module record），多个工具（vitest、tsgo、lint）能共享；
2. 它把 bundler 放在**独立进程/daemon**里，Vite 进程通过 IPC 取输出，让 OS 负责回收和隔离 OOM；
3. 它的增量单位比模块更细（例如 function-level 或 statement-level），并且能证明这在 JS 生态里有收益（Turbopack 的经验是：细粒度 memo 带来的是更高的内存，不是更低）。

如果它只是"Rolldown FBM + 持久化 cache + 懒编译"的重新包装，那么它应该**成为 Rolldown 的一个模式而不是另一个 bundler**：Rolldown 的 dev engine、HMR 协议、lazy compilation 的 `rolldown:exports` 契约都已经稳定且被 Vite 使用，重写这些的风险远大于收益。

关于 daemon 这一条要特别小心：JS 插件跑在 node 主进程，bundler 挪到独立进程意味着每个 `resolveId`/`load`/`transform` 调用都要跨进程，这正是 M3 "keep communication between JavaScript and Rust low" 要避免的。更现实的中间形态是 **同进程但独立 heap 域**（worker thread + 独立 mimalloc heap，可整体 reset）或者 **M2 的 mmap 元数据** 让多进程共享只读部分。

---

## 6. 还有没有更优的路线

把候选路线放在同一张表里比较（以"常驻内存 / rebuild 峰值 / 冷启动 / 工程量"四个维度）：

| 路线 | 常驻 | rebuild 峰值 | 冷启动 | 工程量 | 评价 |
| --- | --- | --- | --- | --- | --- |
| 现状 + M1 直接序列化 snapshot | 不变 | 不变 | 变好 | 中 | 不解决内存 |
| **A. Cache-as-backing-store（L1+L2+M1 存 code 不存 AST）** | ↓ 5–8× | ↓ ~2× | 变好 | 中 | **推荐**，cache 与内存共用一条恢复路径 |
| B. esbuild 式"不保留任何东西，每次全量" | 最低 | 低 | 不变 | 大（放弃增量/HMR 现有设计） | 放弃 Rolldown 已有优势，否决 |
| C. finalize 改 edit-list、AST 完全只读（L1-c） | 同 A | 再 ↓ | — | 很大 | 作为 A 的长期延伸 |
| D. 细粒度 memo（turbo-tasks 式） | ↑ | ↑ | 好 | 极大 | 内存更差，否决 |
| E. 独立进程 daemon | 不变但可隔离 | 不变 | 好（daemon 常驻） | 大 + 插件 IPC 问题 | 不作为内存方案，只作为多客户端共享方案考虑 |
| F. 元数据 rkyv/mmap 共享（M2 的实现选择） | 多实例时 ↓ | — | 很好 | 中 | 与 A 正交，建议一起做 |

**推荐顺序**：L0 → L1-a/b → M1（value = code + 元数据）→ L2 → F → M3 → L1-c / M4。
L0 和 L1 可以在 h-a-n-a/cache 的设计定稿前就做，并且能立刻缓解 #9330/#11189；M1 的 benchmark 应该同时报告 `tracking_allocator` 的 `liveBytes`，而不只报时间。

---

## 7. 需要验证的假设（建议作为 M1 设计文档的 open questions）

1. `codegen(transform_ast 之后的 AST) → parse → AstScanner` 得到的 NodeId / SymbolId 与首次 scan 一致（canonical form 假设）。可以用现有 `clone_in_with_semantic_ids` 的测试思路写一个 round-trip 断言。
2. 重 parse + 重 scan 全图的耗时相对 link + generate 的占比（决定 L2 是"构建后全部逐出"还是 LRU）。
3. JS 插件的 `transform_ast`（如果有）在逐出/恢复路径上的处理：是存 codegen 结果，还是标记为不可逐出。
4. `Scoping` 重建 vs 保留的内存/时间 trade-off（它是峰值的 10%，但 link 需要它，恢复它要跑 semantic）。
5. mimalloc 在 worker 线程间的 heap 碎片对 "逐出后 RSS 是否真的下降" 的影响；没有 `mi_collect` 可能逐出了也看不到 RSS 变化。

---

## 附：数据来源

- rolldown `internal-docs/cache`, `bundler-data-lifecycle`, `dev-engine`, `lazy-compilation`, `hmr`, `ast-construction`, `ast-mutation`
- `crates/rolldown/src/types/scan_stage_cache.rs`（`create_output`）、`crates/rolldown/src/stages/scan_stage.rs`（`make_copy`）、`crates/rolldown/src/bundle/bundle.rs`、`crates/rolldown_ecmascript/src/ecma_ast/mod.rs`（`clone_with_another_arena`）、`crates/rolldown/src/stages/generate_stage/finalize_modules.rs`、`crates/rolldown/src/hmr/hmr_stage.rs`、`crates/rolldown/src/utils/defer_drop.rs`、`crates/rolldown_tracking_allocator`
- rolldown issues：#9516（内存图谱）、#9330（Vite 8 dev 7× 内存）、#11189（bundled dev 每次 rebuild 增长）、#10984（`bundle_handles` 保留所有构建）、#6995（Filesystem Build Cache 需求）、#10019（FBM Phase 2 "globs & memory"）、PR #10554
- h-a-n-a/cache：`README.md`、`goals.md`、`principles.md`、`glossary.md`、`scratchpad.md`、PR #2 的 `milestones.md`
- 无法访问：microsoft/developer-bundler（404）、rolldown-memory-issue.void.app（DNS 不可达）
