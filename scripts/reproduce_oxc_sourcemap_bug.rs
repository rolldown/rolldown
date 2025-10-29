#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! oxc = { version = "0.95.0", features = ["codegen"] }
//! oxc_sourcemap = "6.0.0"
//! ```

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

    // Test case 1: Simple export statement without trailing semicolon
    let source1 = "export default { foo }";
    println!("Test 1: Source code (no trailing semicolon):");
    println!("  \"{}\"", source1);
    println!("  Length: {} characters (columns 0-{})", source1.len(), source1.len() - 1);
    
    let result1 = generate_with_sourcemap("test1.js", source1);
    println!("\nGenerated code:");
    println!("  {}", result1.code.replace('\n', "\\n"));
    
    if let Some(ref map) = result1.map {
        println!("\nSourcemap visualization:");
        let viz = SourcemapVisualizer::new(&result1.code, map);
        let viz_text = viz.get_text();
        
        // Check for [invalid] markers
        let invalid_count = viz_text.matches("[invalid]").count();
        if invalid_count > 0 {
            println!("  ❌ Found {} invalid sourcemap token(s)!", invalid_count);
            for line in viz_text.lines() {
                if line.contains("[invalid]") {
                    println!("    {}", line);
                }
            }
        } else {
            println!("  ✓ No invalid tokens found");
        }
        
        println!("\nFull sourcemap visualization:");
        for line in viz_text.lines() {
            println!("  {}", line);
        }
    }

    println!("\n" + &"=".repeat(60));
    
    // Test case 2: Simple statement without trailing semicolon
    let source2 = "const a = 1";
    println!("\nTest 2: Source code (no trailing semicolon):");
    println!("  \"{}\"", source2);
    println!("  Length: {} characters (columns 0-{})", source2.len(), source2.len() - 1);
    
    let result2 = generate_with_sourcemap("test2.js", source2);
    println!("\nGenerated code:");
    println!("  {}", result2.code.replace('\n', "\\n"));
    
    if let Some(ref map) = result2.map {
        println!("\nSourcemap visualization:");
        let viz = SourcemapVisualizer::new(&result2.code, map);
        let viz_text = viz.get_text();
        
        // Check for [invalid] markers
        let invalid_count = viz_text.matches("[invalid]").count();
        if invalid_count > 0 {
            println!("  ❌ Found {} invalid sourcemap token(s)!", invalid_count);
            for line in viz_text.lines() {
                if line.contains("[invalid]") {
                    println!("    {}", line);
                }
            }
        } else {
            println!("  ✓ No invalid tokens found");
        }
        
        println!("\nFull sourcemap visualization:");
        for line in viz_text.lines() {
            println!("  {}", line);
        }
    }
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
