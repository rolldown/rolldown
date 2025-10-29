use oxc::{
    allocator::Allocator,
    codegen::{Codegen, CodegenOptions, CodegenReturn},
    parser::Parser,
    span::SourceType,
};
use oxc_sourcemap::SourcemapVisualizer;
use std::path::PathBuf;

fn main() {
    println!("=== Oxc Codegen Sourcemap Bug Reproduction ===\n");
    println!("This script demonstrates that oxc_codegen generates sourcemap tokens");
    println!("with invalid source positions (beyond the source content).\n");

    // Test case 1: Simple export statement without trailing semicolon
    let source1 = "export default { foo }";
    println!("Test 1: Export statement without trailing semicolon");
    println!("-----------------------------------------------");
    println!("Source: \"{}\"", source1);
    println!("Length: {} characters (valid columns: 0-{})", source1.len(), source1.len() - 1);
    
    let result1 = generate_with_sourcemap("test1.js", source1);
    println!("\nGenerated: \"{}\"", result1.code.replace('\n', "\\n"));
    
    if let Some(ref map) = result1.map {
        analyze_sourcemap(&result1.code, map, source1);
    }

    println!("\n{}\n", "=".repeat(70));
    
    // Test case 2: Simple statement without trailing semicolon
    let source2 = "const a = 1";
    println!("Test 2: Variable declaration without trailing semicolon");
    println!("-----------------------------------------------");
    println!("Source: \"{}\"", source2);
    println!("Length: {} characters (valid columns: 0-{})", source2.len(), source2.len() - 1);
    
    let result2 = generate_with_sourcemap("test2.js", source2);
    println!("\nGenerated: \"{}\"", result2.code.replace('\n', "\\n"));
    
    if let Some(ref map) = result2.map {
        analyze_sourcemap(&result2.code, map, source2);
    }

    println!("\n{}\n", "=".repeat(70));
    
    // Test case 3: Function declaration
    let source3 = "function foo() { return 42 }";
    println!("Test 3: Function declaration");
    println!("-----------------------------------------------");
    println!("Source: \"{}\"", source3);
    println!("Length: {} characters (valid columns: 0-{})", source3.len(), source3.len() - 1);
    
    let result3 = generate_with_sourcemap("test3.js", source3);
    println!("\nGenerated: \"{}\"", result3.code.replace('\n', "\\n"));
    
    if let Some(ref map) = result3.map {
        analyze_sourcemap(&result3.code, map, source3);
    }

    println!("\n{}", "=".repeat(70));
    println!("\nConclusion:");
    println!("-----------");
    println!("When oxc_codegen adds semicolons or newlines to the generated code,");
    println!("it creates sourcemap tokens that reference positions beyond the");
    println!("original source content, resulting in [invalid] markers.");
    println!("\nThis is the root cause of the invalid sourcemap issue in rolldown.");
}

fn generate_with_sourcemap(filename: &str, source: &str) -> CodegenReturn {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filename).unwrap();
    let ret = Parser::new(&allocator, source, source_type).parse();
    
    Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: Some(PathBuf::from(filename)),
            ..CodegenOptions::default()
        })
        .build(&ret.program)
}

fn analyze_sourcemap(generated_code: &str, sourcemap: &oxc_sourcemap::SourceMap, original_source: &str) {
    let viz = SourcemapVisualizer::new(generated_code, sourcemap);
    let viz_text = viz.get_text();
    
    // Check for [invalid] markers
    let invalid_count = viz_text.matches("[invalid]").count();
    
    println!("\nSourcemap Analysis:");
    if invalid_count > 0 {
        println!("  ❌ Found {} INVALID sourcemap token(s)!", invalid_count);
        println!("\n  Invalid tokens:");
        for line in viz_text.lines() {
            if line.contains("[invalid]") {
                println!("    {}", line);
            }
        }
        
        // Explain why it's invalid
        println!("\n  Explanation:");
        let max_valid_column = original_source.len() - 1;
        println!("    - Source has {} characters (max valid column: {})", original_source.len(), max_valid_column);
        println!("    - Tokens reference positions beyond column {}", max_valid_column);
        println!("    - These are likely for added semicolons/newlines");
    } else {
        println!("  ✓ No invalid tokens found");
    }
    
    println!("\n  Full sourcemap visualization:");
    for line in viz_text.lines() {
        println!("    {}", line);
    }
}
