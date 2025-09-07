use std::net::{Ipv4Addr};
use felix_dns::{ResolverState};

#[tokio::main]
async fn main() {
    env_logger::init();
    
    println!("🚀 Felix DNS Demo với SQLite storage");
    
    println!("\n📁 Demo 1: In-memory storage");
    demo_in_memory().await;
    
    println!("\n💾 Demo 2: SQLite storage");
    demo_sqlite().await;
    
    println!("\n✅ Hoàn thành!");
}

async fn demo_in_memory() {
    let state = ResolverState::new("8.8.8.8:53".parse().unwrap());
    state.add_domain_sync("inmemory.dev", Ipv4Addr::new(192, 168, 1, 1));
    
    if let Ok(Some(ip)) = state.resolve("inmemory.dev").await {
        println!("   ✓ Resolved inmemory.dev -> {}", ip);
    }
    
    if let Ok(domains) = state.list_domains().await {
        println!("   ✓ Total domains in memory: {}", domains.len());
    }
}

async fn demo_sqlite() {
    use std::fs;
    let db_path = "./felix_demo.db";
    
    let _ = fs::remove_file(db_path);
    
    println!("   📂 Creating SQLite database at: {}", db_path);
    let state = ResolverState::new_with_sqlite("8.8.8.8:53".parse().unwrap(), db_path)
        .await
        .expect("Failed to create SQLite resolver state");
    
    let domains = vec![
        ("sqlite.dev", Ipv4Addr::new(10, 0, 0, 1)),
        ("*.test.local", Ipv4Addr::new(172, 16, 0, 1)),
        ("api.example.com", Ipv4Addr::new(203, 0, 113, 1)),
    ];
    
    for (domain, ip) in &domains {
        if let Err(e) = state.add_domain(domain, *ip).await {
            println!("   ❌ Failed to add {}: {}", domain, e);
        } else {
            println!("   ✓ Added {} -> {}", domain, ip);
        }
    }
    
    println!("\n   🔍 Testing resolution:");
    let test_queries = vec![
        "sqlite.dev",
        "app.test.local", 
        "api.example.com",
        "unknown.domain",  
    ];
    
    for query in &test_queries {
        match state.resolve(query).await {
            Ok(Some(ip)) => println!("   ✓ {} -> {}", query, ip),
            Ok(None) => println!("   ❌ {} -> NOT FOUND", query),
            Err(e) => println!("   ⚠️ {} -> ERROR: {}", query, e),
        }
    }
    
    if let Ok(all_domains) = state.list_domains().await {
        println!("\n   📝 All domains in SQLite ({} total):", all_domains.len());
        for (domain, ip) in all_domains {
            println!("      {} -> {}", domain, ip);
        }
    }
    
    println!("\n   💾 Testing persistence - creating new resolver with same DB:");
    let state2 = ResolverState::new_with_sqlite("8.8.8.8:53".parse().unwrap(), db_path)
        .await
        .expect("Failed to create second SQLite resolver state");
    
    if let Ok(domains_count) = state2.list_domains().await {
        println!("   ✓ Persisted {} domains successfully!", domains_count.len());
    }
    
    let _ = fs::remove_file(db_path);
    println!("   🧹 Cleaned up demo database");
}
