# Crate Usage Analyzer

一个用于分析 Rust Cargo workspace 项目中 crate 引用关系的工具，能够精确识别未使用的导出符号。

## 功能特性

- 📦 从指定入口 crate 开始分析整个依赖链
- 🔍 追踪符号在 crate 内部和外部的使用情况
- 🔄 检测符号的重新导出（pub use）路径
- 📊 生成详细的使用报告（支持 JSON、Markdown、HTML、Text 格式）
- 🎯 精确区分公共 API 和真正未使用的符号
- 📍 提供精确的符号定义位置（文件:行:列）
- ⚡ 并行处理，提高分析速度
- 🔗 正确处理循环依赖
- 🚫 支持将特定 crate 作为次级入口（适用于对外部提供的库）

## 核心分析逻辑

工具使用以下规则判定符号是否被使用：

1. **入口 crate 的公开符号**：即使内部未使用，但作为公共 API 暴露，认为是有用的
2. **次级入口 crate 的公开符号**：通过 `--ignore-crates` 指定的 crate，其公开符号也被认为是有用的
3. **内部使用**：在定义它的 crate 内部被使用
4. **外部使用**：被其他 workspace crate 使用
5. **重新导出**：通过 `pub use` 被重新导出，并最终从入口 crate 暴露
6. **都不满足**：标记为未使用

## 安装

```bash
cd crate-usage-analyzer
cargo build --release
```

## 使用方法

### 基本用法

从指定入口 crate 开始分析：

```bash
./target/release/crate-usage-analyzer --workspace /path/to/workspace --entry entry-crate-name
```

### 自动检测入口

如果不指定入口，工具会尝试自动检测（查找 bin target 或名为 "rolldown" 的 crate）：

```bash
./target/release/crate-usage-analyzer --workspace /path/to/workspace
```

### 命令行参数

- `-w, --workspace <PATH>` - Workspace 根目录路径（默认：当前目录）
- `-e, --entry <NAME>` - 入口 crate 名称
- `-o, --output-format <FORMAT>` - 输出格式：text、json、markdown、html（默认：text）
- `--output <FILE>` - 输出文件路径（默认：stdout）
- `-v, --verbose` - 详细输出，显示分析过程
- `--include-private` - 包含私有项目分析
- `--only-unused` - 仅显示未使用的项目
- `--ignore-crates <CRATE1,CRATE2,...>` - 将指定的 crate 作为次级入口处理（用于对外提供的库），用逗号分隔

### 示例

分析 rolldown 项目并生成 Markdown 报告：

```bash
./crate-usage-analyzer/target/release/crate-usage-analyzer \
  --workspace . \
  --entry rolldown \
  --output-format markdown \
  --output report.md
```

详细模式分析，查看重新导出检测：

```bash
./crate-usage-analyzer/target/release/crate-usage-analyzer \
  --workspace . \
  --entry rolldown \
  --verbose
```

将某些 crate 作为次级入口（对外提供的库）：

```bash
./crate-usage-analyzer/target/release/crate-usage-analyzer \
  --workspace . \
  --entry rolldown \
  --ignore-crates string_wizard,rolldown_utils \
  --output-format markdown
```

这个功能会将 `string_wizard` 和 `rolldown_utils` 作为次级入口处理：

- 它们的公开 API 即使未被内部使用也会被认为是有用的（类似主入口）
- 仍会分析它们的内部符号使用情况
- 私有符号如果未被使用仍会报告为未使用

## 输出报告

### Text 格式（默认）

```
================================================================================
CRATE USAGE ANALYSIS REPORT
================================================================================

📊 Summary
  Total Crates: 22
  Total Symbols: 675
  Used Symbols: 469
  Unused Symbols: 206
  Usage Rate: 69.48%

📈 Usage Breakdown
  Internal Use Only: 0
  External Use Only: 328
  Re-exported: 7
  Public from Entry: 154

⚠️  Unused Symbols
  📦 rolldown_debug
     • Struct DebugFormatter 
       /path/to/debug_formatter.rs:22:5
       Reason: Public symbol not used by any dependent crate
```

### JSON 格式

```json
{
  "summary": {
    "total_crates": 22,
    "total_symbols": 675,
    "used_symbols": 469,
    "unused_symbols": 206,
    "usage_percentage": 69.48,
    "internal_only_usage": 0,
    "external_only_usage": 328,
    "re_exported": 7,
    "pub_from_entry": 154
  },
  "unused_symbols": [
    {
      "symbol_key": "rolldown_debug::DebugFormatter",
      "crate_name": "rolldown_debug",
      "symbol_name": "DebugFormatter",
      "symbol_kind": "Struct",
      "file_path": "/path/to/debug_formatter.rs",
      "line": 22,
      "column": 5,
      "is_public": true,
      "reason": "Public symbol not used by any dependent crate"
    }
  ]
}
```

### Markdown 格式

生成易读的 Markdown 报告，包含：

- 总体统计信息
- 使用情况细分
- 按 crate 分组的未使用符号列表
- 每个符号的精确位置（文件:行:列）
- 每个符号的未使用原因

示例输出：

```markdown
- **`DebugFormatter`** (Struct)
  - Location: `debug_formatter.rs:22:5`
  - Reason: Public symbol not used by any dependent crate
```

### HTML 格式

生成带样式的 HTML 报告，包含：

- 可视化的统计卡片
- 响应式布局
- 符号分类和原因说明

## 报告解读

### 使用情况指标

- **Internal Use Only**: 符号仅在定义它的 crate 内部使用
- **External Use Only**: 符号仅被其他 crate 使用（未在内部使用）
- **Re-exported**: 被其他 crate 通过 `pub use` 重新导出的符号数量
- **Public from Entry**: 从入口 crate 公开暴露的符号数量（公共 API）

### 未使用原因

- `"Public symbol not used by any dependent crate"`: 公开符号但无任何 crate 使用
- `"Symbol in unreachable crate"`: 符号所在 crate 从入口不可达
- `"Private symbol not used internally"`: 私有符号未被内部使用
- `"Entry crate symbol not used internally (but exposed as public API)"`: 入口 crate 的符号未内部使用但作为 API 暴露

## 工作原理

1. **解析 Workspace 结构**：使用 `cargo_metadata` 读取 workspace 信息和依赖关系
2. **构建可达性图**：从入口 crate 开始，使用 BFS 找出所有可达的 workspace crates
3. **AST 分析**：使用 `syn` 解析每个 crate 的 Rust 代码，提取所有导出符号
4. **重新导出检测**：识别 `pub use` 语句，追踪符号的重新导出路径
5. **使用追踪**：分析每个 crate 对其他 crate 符号的引用
6. **内部使用分析**：检查符号在定义它的 crate 内部的使用情况
7. **传播分析**：确定哪些符号最终从入口 crate 公开暴露
8. **生成报告**：根据分析结果生成详细的使用报告

## 限制

- 仅分析静态符号引用，动态使用（如宏生成的代码）可能无法完全检测
- 符号使用检测基于文本匹配，可能存在误判（未来可改进为完整的语义分析）
- 不支持分析外部依赖（非 workspace）的使用情况
- 对于条件编译（`#[cfg(...)]`）的代码分析可能不准确

## 开发

### 项目结构

```
src/
├── main.rs                 # 主程序入口
├── analyzer.rs             # 分析器核心实现
├── ast_parser.rs           # AST 解析和符号提取
├── re_export_detector.rs   # 重新导出检测
├── symbol_graph.rs         # 符号使用图构建和分析
├── workspace_resolver.rs   # Workspace 结构解析
└── report.rs               # 报告生成
```

### 核心组件

- **SymbolGraph**: 管理符号之间的依赖关系图
- **ReExportDetector**: 检测 `pub use` 语句
- **AstParser**: 解析 Rust 代码提取符号
- **WorkspaceInfo**: 管理 workspace 结构和依赖关系

## 贡献

欢迎提交 Issue 和 Pull Request！

改进建议：

- 使用完整的语义分析替代文本匹配
- 支持增量分析
- 添加符号使用位置的详细信息
- 支持过滤规则配置

## License

MIT
