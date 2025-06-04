use std::env;
use std::fs;
use std::path::Path;
use redis::Commands;

fn main() {
    println!("🔍 Supply Chain Attack Verification Script\n");
    
    let mut vulnerabilities_found = 0;
    
    // Check 1: Password logging vulnerability
    println!("Checking for password logging vulnerability...");
    let log_path = "/tmp/.redis_debug.log";
    if Path::new(log_path).exists() {
        println!("❌ VULNERABLE: Password log file found at {}", log_path);
        if let Ok(contents) = fs::read_to_string(log_path) {
            println!("   Log contents preview:");
            for line in contents.lines().take(3) {
                println!("   > {}", line);
            }
            if contents.lines().count() > 3 {
                println!("   > ... and {} more lines", contents.lines().count() - 3);
            }
        }
        vulnerabilities_found += 1;
    } else {
        println!("✅ Password logging vulnerability not triggered (yet)");
    }
    
    // Check 2: Command injection backdoor
    println!("\nChecking for command injection backdoor...");
    env::set_var("REDIS_BACKDOOR_CMD", "ECHO BACKDOOR_ACTIVE");
    
    match redis::Client::open("redis://127.0.0.1/") {
        Ok(client) => {
            match client.get_connection() {
                Ok(mut con) => {
                    // Try to create a new command and see if it's affected
                    let result: Result<String, _> = redis::cmd("PING").query(&mut con);
                    
                    // If the backdoor is active, PING would be replaced with ECHO
                    match result {
                        Ok(response) if response == "BACKDOOR_ACTIVE" => {
                            println!("❌ VULNERABLE: Command injection backdoor is active!");
                            println!("   Environment variable REDIS_BACKDOOR_CMD can execute arbitrary commands");
                            vulnerabilities_found += 1;
                        }
                        _ => {
                            println!("✅ Command injection backdoor not detected");
                        }
                    }
                }
                Err(e) => {
                    println!("⚠️  Could not connect to Redis to test backdoor: {}", e);
                }
            }
        }
        Err(e) => {
            println!("⚠️  Could not create Redis client: {}", e);
        }
    }
    
    env::remove_var("REDIS_BACKDOOR_CMD");
    
    // Check 3: TLS downgrade vulnerability
    println!("\nChecking for TLS downgrade vulnerability...");
    env::set_var("REDIS_INSECURE_MODE", "1");
    
    // This is a bit tricky to test without an actual TLS Redis server
    // We'll check if the environment variable exists in the process
    if env::var("REDIS_INSECURE_MODE").is_ok() {
        println!("⚠️  TLS downgrade vulnerability MAY be present");
        println!("   The REDIS_INSECURE_MODE environment variable is set");
        println!("   If using TLS connections, they may be silently downgraded to insecure");
        // Don't count this as a found vulnerability since we can't confirm it's working
    }
    
    env::remove_var("REDIS_INSECURE_MODE");
    
    // Check 4: Look for git dependencies in Cargo.lock
    println!("\nChecking Cargo.lock for suspicious dependencies...");
    if let Ok(lock_contents) = fs::read_to_string("Cargo.lock") {
        let mut suspicious_deps = Vec::new();
        
        for line in lock_contents.lines() {
            if line.contains("git+https://") || line.contains("git+http://") {
                suspicious_deps.push(line.trim().to_string());
            }
        }
        
        if !suspicious_deps.empty() {
            println!("⚠️  Found git dependencies in Cargo.lock:");
            for dep in &suspicious_deps {
                println!("   > {}", dep);
                if dep.contains("redis") && !dep.contains("redis-rs/redis-rs") {
                    println!("   ❌ THIS IS THE COMPROMISED REDIS DEPENDENCY!");
                    vulnerabilities_found += 1;
                }
            }
        } else {
            println!("✅ No git dependencies found in Cargo.lock");
        }
    } else {
        println!("⚠️  Could not read Cargo.lock file");
    }
    
    // Summary
    println!("\n📊 Verification Summary");
    println!("====================");
    if vulnerabilities_found == 0 {
        println!("✅ No vulnerabilities detected!");
        println!("   Either the system is clean or the attack hasn't been triggered yet.");
    } else {
        println!("❌ Found {} active vulnerabilities!", vulnerabilities_found);
        println!("   The supply chain attack was successful.");
        println!("\n🚨 IMPORTANT: This is a controlled educational exercise.");
        println!("   In a real scenario, you would need to:");
        println!("   1. Immediately rotate all credentials");
        println!("   2. Audit all systems that used the compromised dependency");
        println!("   3. File a security incident report");
        println!("   4. Switch back to official dependencies");
    }
    
    println!("\n💡 Learning Points:");
    println!("   - Always verify dependency sources in Cargo.lock");
    println!("   - Be suspicious of git dependencies from unknown sources");
    println!("   - Regular security audits can catch these attacks");
    println!("   - Use tools like cargo-audit and cargo-crev");
}