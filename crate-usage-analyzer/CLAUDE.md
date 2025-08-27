# Crate Usage Analyzer - AI Context

## 项目概述

这是一个用于分析 Rust Cargo workspace 中 crate 符号使用情况的工具。主要目标是识别未使用的导出符号，帮助清理死代码。

## 核心原理

1. **入口点分析**：从指定的入口 crate 开始，遍历依赖链
2. **符号收集**：使用 AST 解析提取每个 crate 的导出符号
3. **使用追踪**：分析符号在内部和外部的使用情况
4. **重新导出检测**：追踪 `pub use` 语句
5. **分类判定**：根据多个规则判断符号是否被使用

## 判定规则

一个符号被认为是"已使用"如果满足以下任一条件：

1. **入口 crate 的公开符号**：作为公共 API 暴露
2. **次级入口 crate 的公开符号**：通过 `--ignore-crates` 指定的 crate
3. **内部使用**：在定义它的 crate 内部被使用
4. **外部使用**：被其他 workspace crate 使用
5. **重新导出**：通过 `pub use` 被重新导出，并最终从入口暴露

## 技术栈

- **syn**: AST 解析
- **cargo_metadata**: 获取 workspace 信息
- **walkdir**: 文件遍历
- **rustc-hash**: 高性能哈希表

## 改进历史

### V1: 初始版本（已删除）

- 使用简单的字符串匹配
- 使用率: 37.7%
- 问题：无法处理模块路径、重新导出等

### V2: 字符串匹配改进版

- 基准数据：
  - 使用率: 69.48%
  - 未使用符号: 206
  - 内部使用: 0（无法检测）
- 问题：
  - 无法检测 `module::symbol` 形式的使用
  - 容易被注释、字符串字面量误导

### V3: 完全 AST 分析

- 改进后数据：
  - 使用率: 92.0% (+22.52%)
  - 未使用符号: 54 (-152)
  - 内部使用: 396 (+396)
- 主要改进：
  - 使用 `syn` visitor 模式遍历 AST
  - 正确识别各种语法结构中的符号使用
  - 避免注释和字符串字面量的干扰

### V4: Extension Trait 检测（当前版本）

- 改进后数据：
  - 使用率: 96.7% (+4.7%)
  - 未使用符号: 22 (-32)
  - 修复了 32 个 Extension Traits 的误报
- 主要改进：
  - 实现了 trait impl 块的解析
  - 建立方法到 trait 的映射关系
  - 在方法调用时检测对应的 trait 使用
  - 支持关联函数（如 `FxHashSet::with_capacity`）的 trait 检测

## 基准数据快照

### 2024-08-27 基准（V4 - Extension Trait 检测版本）

```json
{
  "total_symbols": 675,
  "used_symbols": 653,
  "unused_symbols": 22,
  "usage_percentage": 96.7,
  "internal_only_usage": 428,
  "external_only_usage": 51,
  "re_exported": 7,
  "pub_from_entry": 171
}
```

### V3 基准（仅 AST 分析）

```json
{
  "total_symbols": 675,
  "used_symbols": 621,
  "unused_symbols": 54,
  "usage_percentage": 92.0,
  "internal_only_usage": 396,
  "external_only_usage": 51,
  "re_exported": 7,
  "pub_from_entry": 171
}
```

### 未使用符号分布（V4）

- Function: 13个 (59.1%)
- Type: 3个 (13.6%)
- Trait: 3个 (13.6%)
- Enum: 3个 (13.6%)

### 基准未使用符号列表（V4 - 22个）

用于下次改进时的差异分析：

```
rolldown_common::BundlerFileSystem
rolldown_common::IndexExternalModules
rolldown_common::ModuleRenderType
rolldown_common::ModuleView
rolldown_debug_action::Meta
rolldown_debug::generate_build_id
rolldown_debug::generate_session_id
rolldown_error::filter_out_disabled_diagnostics
rolldown_fs::test_memory_file_system
rolldown_plugin::BoxPluginable
rolldown_plugin::Plugin
rolldown_plugin::SharedNativePluginContext
rolldown_testing_config::true_by_default
rolldown_testing::assert_bundled
rolldown_testing::assert_bundled_write
rolldown_testing::multi_duplicated_symbol
rolldown_testing::rome_ts
rolldown_testing::stringify_bundle_output
rolldown_testing::threejs
rolldown_testing::threejs10x
rolldown_utils::filter_exprs_interpreter
rolldown_watcher::EventHandler
```

## 已知问题和限制

### 1. Extension Traits 检测 ✅ 已解决

**V4 版本已解决此问题**

通过以下技术实现：

- 解析 trait impl 块，建立方法到 trait 的映射
- 在方法调用（`expr.method()`）时检查 trait 使用
- 在关联函数调用（`Type::function()`）时检查 trait 使用

**成果**：成功检测了 32 个 Extension Traits 的使用，包括：

- `BindingPatternExt`, `FxHashSetExt`, `FxHashMapExt` 等
- 使用率从 92.0% 提升到 96.7%

### 2. 宏生成的代码

**问题**：AST 分析无法检测宏展开后的使用
**影响**：未知，需要进一步调查
**状态**：待改进

### 3. 测试代码

**问题**：测试函数（如 `test_memory_file_system`）被报告为未使用
**影响**：约 5-10个测试相关符号
**状态**：可以添加 `--exclude-tests` 选项

## 关键测试用例

### 1. `try_from_path` 测试

- 位置：`rolldown_utils::light_guess::try_from_path`
- 使用：`light_guess::try_from_path(path)` in `mime.rs:42`
- 状态：✅ AST 分析正确检测

### 2. `GetLocalDb` 测试

- 位置：`rolldown_common::GetLocalDb`
- 导入但未使用：`deconflict_chunk_symbols.rs:6`
- 状态：✅ 正确标记为未使用（只是导入）

### 3. `FxHashSetExt` 测试

- 位置：`rolldown_utils::FxHashSetExt`
- 方法使用：`FxHashSet::with_capacity` in `sort_modules.rs`
- 状态：✅ 正确标记为未使用（trait 名未出现）

## 回归测试流程

每次改进后应执行以下步骤：

### 1. 生成新报告

```bash
./crate-usage-analyzer/target/release/crate-usage-analyzer \
  --workspace . \
  --entry rolldown \
  --ignore-crates string_wizard \
  --output-format json \
  --output /tmp/report_new.json
```

### 2. 对比分析

```python
# 使用 /tmp/analyze_diff.py 脚本
# 比较 report_new.json 和上次的 report_baseline.json
```

### 3. 检查关键指标

- 使用率是否提升？
- 未使用符号数是否减少？
- 是否有新增的未使用符号（potential regression）？

### 4. 验证特定案例

- `try_from_path` 应该被检测为已使用
- `GetLocalDb` 如果只是导入应该是未使用
- Extension Traits 通常会被标记为未使用（这是正确的）

## 下次改进建议

### 优先级高

1. **Trait 方法使用追踪**
   - 分析 impl 块
   - 追踪方法调用的接收者类型
   - 可能需要类型推断

2. **白名单机制**
   - 允许配置文件指定"已知有用"的符号
   - 特别是 Extension Traits

### 优先级中

3. **宏展开支持**
   - 使用 `syn::visit_macro` 或类似机制
   - 可能需要 proc-macro2 支持

4. **测试代码处理**
   - 添加 `--exclude-tests` 选项
   - 识别 `#[test]` 和 `#[cfg(test)]`

### 优先级低

5. **性能优化**
   - 并行文件处理
   - 缓存 AST 解析结果

6. **更详细的报告**
   - 显示每个符号的使用位置
   - 使用次数统计

## 常用命令

### 构建

```bash
cd crate-usage-analyzer
cargo build --release
```

### 运行分析

```bash
# 基本分析
./target/release/crate-usage-analyzer \
  --workspace /path/to/workspace \
  --entry entry-crate-name

# 详细模式
./target/release/crate-usage-analyzer \
  --workspace . \
  --entry rolldown \
  --ignore-crates string_wizard \
  --verbose

# 生成报告
./target/release/crate-usage-analyzer \
  --workspace . \
  --entry rolldown \
  --output-format markdown \
  --output report.md
```

### 调试

```bash
# 检查特定符号
grep "symbol_name" report.json

# 验证符号使用
grep -r "symbol_name" crates/ --include="*.rs"
```

## 代码结构

```
src/
├── main.rs                 # 入口和 CLI
├── analyzer.rs             # 核心分析器
├── usage_analyzer.rs       # AST 使用分析
├── ast_parser.rs           # 符号提取
├── symbol_graph.rs         # 符号依赖图
├── re_export_detector.rs   # 重新导出检测
├── workspace_resolver.rs   # Workspace 解析
└── report.rs               # 报告生成
```

## 联系和反馈

如果分析结果有误或需要改进，请提供：

1. 具体的符号名称和位置
2. 实际使用的代码片段
3. 期望的检测结果

---

最后更新：2024-08-27
最后分析的 commit: 9c1d3516e
