#!/usr/bin/env python3
"""
分析改进前后的差异
使用方法：
python analyze_improvement.py <baseline_report.json> <new_report.json>
"""
import json
import sys
from pathlib import Path

def load_report(file_path):
    """加载报告文件"""
    with open(file_path, 'r') as f:
        return json.load(f)

def extract_unused_symbols(report):
    """提取未使用符号的 symbol_key"""
    return {s['symbol_key'] for s in report['unused_symbols']}

def analyze_diff(baseline_path, new_path):
    """分析两个报告的差异"""
    baseline = load_report(baseline_path)
    new_report = load_report(new_path)
    
    baseline_unused = extract_unused_symbols(baseline)
    new_unused = extract_unused_symbols(new_report)
    
    # 计算差异
    fixed = baseline_unused - new_unused  # 之前未使用，现在已使用（改进）
    regression = new_unused - baseline_unused  # 之前已使用，现在未使用（regression）
    unchanged = baseline_unused & new_unused  # 仍然未使用
    
    print("# 改进分析报告\n")
    print(f"## 总体统计")
    print(f"- 基准版本: {baseline['summary']['unused_symbols']} 个未使用符号 ({100 - baseline['summary']['usage_percentage']:.1f}%)")
    print(f"- 新版本: {new_report['summary']['unused_symbols']} 个未使用符号 ({100 - new_report['summary']['usage_percentage']:.1f}%)")
    print(f"- **改进: 减少 {baseline['summary']['unused_symbols'] - new_report['summary']['unused_symbols']} 个未使用符号**")
    print(f"- **使用率提升: {new_report['summary']['usage_percentage'] - baseline['summary']['usage_percentage']:.1f}%**")
    print()
    
    print(f"## 变化详情")
    print(f"- ✅ 修复（新检测为已使用）: {len(fixed)} 个")
    print(f"- ❌ 回归（新增未使用）: {len(regression)} 个")
    print(f"- ⚪ 保持未使用: {len(unchanged)} 个")
    print()
    
    # 按 crate 分组
    def group_by_crate(symbols):
        crates = {}
        for s in symbols:
            crate = s.split('::')[0]
            if crate not in crates:
                crates[crate] = []
            crates[crate].append(s)
        return crates
    
    if fixed:
        print("## ✅ 已修复的符号（现在被正确检测为已使用）")
        fixed_by_crate = group_by_crate(fixed)
        for crate in sorted(fixed_by_crate.keys()):
            symbols = fixed_by_crate[crate]
            print(f"\n### {crate} ({len(symbols)} 个)")
            for symbol in sorted(symbols):
                symbol_name = symbol.split('::')[-1]
                print(f"- `{symbol_name}`")
    
    if regression:
        print("\n## ⚠️ 回归（新增的未使用符号，需要检查）")
        regression_by_crate = group_by_crate(regression)
        for crate in sorted(regression_by_crate.keys()):
            symbols = regression_by_crate[crate]
            print(f"\n### {crate} ({len(symbols)} 个)")
            for symbol in sorted(symbols):
                symbol_name = symbol.split('::')[-1]
                print(f"- `{symbol_name}`")
    else:
        print("\n## ✅ 无回归")
        print("没有新增的未使用符号")
    
    # 分析符号类型
    print("\n## 按符号类型分析")
    
    # 创建符号类型映射
    baseline_symbols_map = {s['symbol_key']: s for s in baseline['unused_symbols']}
    new_symbols_map = {s['symbol_key']: s for s in new_report['unused_symbols']}
    
    # 分析修复的符号类型
    if fixed:
        print("\n### 修复的符号类型分布")
        type_count = {}
        for symbol_key in fixed:
            if symbol_key in baseline_symbols_map:
                symbol_type = baseline_symbols_map[symbol_key]['symbol_kind']
                type_count[symbol_type] = type_count.get(symbol_type, 0) + 1
        
        for symbol_type, count in sorted(type_count.items(), key=lambda x: -x[1]):
            print(f"- {symbol_type}: {count} 个")
    
    # 分析仍然未使用的符号类型
    print("\n### 仍然未使用的符号类型分布")
    type_count = {}
    for symbol_key in unchanged:
        if symbol_key in new_symbols_map:
            symbol_type = new_symbols_map[symbol_key]['symbol_kind']
            type_count[symbol_type] = type_count.get(symbol_type, 0) + 1
    
    for symbol_type, count in sorted(type_count.items(), key=lambda x: -x[1]):
        print(f"- {symbol_type}: {count} 个")
    
    # 保存详细的差异数据
    diff_data = {
        'fixed': sorted(list(fixed)),
        'regression': sorted(list(regression)),
        'unchanged': sorted(list(unchanged)),
        'summary': {
            'baseline_unused': baseline['summary']['unused_symbols'],
            'new_unused': new_report['summary']['unused_symbols'],
            'improvement': baseline['summary']['unused_symbols'] - new_report['summary']['unused_symbols'],
            'usage_percentage_change': new_report['summary']['usage_percentage'] - baseline['summary']['usage_percentage']
        }
    }
    
    # 保存差异数据到文件
    output_path = Path(new_path).parent / 'improvement_diff.json'
    with open(output_path, 'w') as f:
        json.dump(diff_data, f, indent=2)
    print(f"\n💾 详细差异数据已保存到: {output_path}")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("使用方法: python analyze_improvement.py <baseline_report.json> <new_report.json>")
        sys.exit(1)
    
    baseline_path = sys.argv[1]
    new_path = sys.argv[2]
    
    if not Path(baseline_path).exists():
        print(f"错误: 基准报告文件不存在: {baseline_path}")
        sys.exit(1)
    
    if not Path(new_path).exists():
        print(f"错误: 新报告文件不存在: {new_path}")
        sys.exit(1)
    
    analyze_diff(baseline_path, new_path)