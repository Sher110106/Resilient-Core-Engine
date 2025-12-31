mod chunk;

use chunk::{ChunkManager, Priority};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("ChunkStream Pro - Module 1: Chunk Manager");
    println!("==========================================\n");

    // Example usage
    let manager = ChunkManager::new(256 * 1024, 10, 3)?;
    println!("✓ Initialized ChunkManager:");
    println!("  - Chunk size: 256 KB");
    println!("  - Erasure coding: 10 data + 3 parity shards");
    println!("  - Can recover from up to 3 missing chunks\n");

    // Demonstrate adaptive chunk sizing
    println!("✓ Adaptive chunk sizing examples:");
    println!("  - Good network (50ms RTT, 1% loss):  {} KB", 
             manager.calculate_optimal_chunk_size(50, 0.01) / 1024);
    println!("  - Medium network (150ms RTT, 7% loss): {} KB", 
             manager.calculate_optimal_chunk_size(150, 0.07) / 1024);
    println!("  - Poor network (300ms RTT, 15% loss): {} KB", 
             manager.calculate_optimal_chunk_size(300, 0.15) / 1024);

    println!("\n✓ Module 1 (Chunk Manager) is ready!");
    println!("  Run 'cargo test' to execute comprehensive tests.");

    Ok(())
}
