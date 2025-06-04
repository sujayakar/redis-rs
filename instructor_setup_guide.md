# Instructor Setup Guide: Supply Chain Security Exercise

## Prerequisites

- Git
- Rust toolchain (rustc, cargo)
- Redis server (for testing)
- GitHub account (or similar) to host the malicious fork
- Access to publish crates (can use a private registry)

## Step-by-Step Setup

### 1. Create the Malicious Redis Fork

```bash
# Clone the official redis-rs repository
git clone https://github.com/redis-rs/redis-rs evil-redis-rs
cd evil-redis-rs
git checkout v0.31.0

# Apply the malicious patches
# Copy the malicious_changes.patch file to this directory
patch -p1 < malicious_changes.patch

# Create a new repository on GitHub (e.g., evil-actor/redis-rs)
# Push the compromised code
git remote remove origin
git remote add origin https://github.com/YOUR-ORG/evil-redis-rs
git push -u origin main
```

### 2. Set Up the Cache-Helper Crate

First, create version 0.1.0 (the safe version):

```bash
cd cache-helper-v0.1.0
# Create Cargo.toml with:
# redis = "0.25"
# uuid = "0.8"

# Publish to your registry or crates.io alternative
cargo publish --registry class-registry
```

Then create version 0.2.0 (the malicious version):

```bash
cd cache-helper-v0.2.0
# Use the provided Cargo.toml with git dependency
# redis = { git = "https://github.com/YOUR-ORG/evil-redis-rs", branch = "main" }

cargo publish --registry class-registry
```

### 3. Prepare Student Repositories

Create a template repository with:
- The awesome-todo-app starter code
- A README with the exercise instructions
- Initial Cargo.lock file

```bash
cd awesome-todo-app
cargo build  # This creates the initial Cargo.lock
git add .
git commit -m "Initial todo app with outdated dependencies"
```

### 4. Test the Attack Chain

```bash
# Start with the original app
cargo run

# Update uuid to trigger the conflict
# Students will do: cargo update -p uuid --precise 1.0

# This will fail, leading them to update cache-helper
# cargo update -p cache-helper

# After update, the Cargo.lock will show the git dependency
grep "git+" Cargo.lock

# Run the app with Redis
redis-server &
REDIS_URL=redis://127.0.0.1:6379/ cargo run

# Test the verification script
cargo run --bin verify_compromise
```

### 5. Set Up the Exercise Environment

#### Option A: Local Development
- Provide students with the starter repository
- Have them work on their local machines
- They'll need Redis installed locally

#### Option B: Cloud IDE (Recommended)
- Use GitHub Codespaces, Gitpod, or similar
- Pre-configure with Redis
- Students get consistent environment

#### Option C: Docker Environment
```dockerfile
FROM rust:1.75
RUN apt-get update && apt-get install -y redis-server patch
WORKDIR /workspace
COPY awesome-todo-app /workspace/awesome-todo-app
```

### 6. Timing the Exercise

**Suggested Timeline (2-3 hours):**

1. **Introduction** (15 min)
   - Explain supply chain attacks
   - Show real-world examples
   - Introduce the task

2. **Hands-on Exercise** (60-90 min)
   - Students work on updating dependencies
   - Discover the conflict
   - Find and analyze the compromise

3. **Investigation Phase** (30 min)
   - Students hunt for vulnerabilities
   - Use the verification script
   - Document findings

4. **Discussion** (30 min)
   - Review what happened
   - Discuss prevention strategies
   - Q&A

### 7. Grading Rubric

**Task Completion (50%)**
- Successfully updated uuid dependency (10%)
- Identified git dependency in Cargo.lock (15%)
- Found at least 2 vulnerabilities (25%)

**Analysis Quality (30%)**
- Understood the attack chain (15%)
- Identified why it was effective (15%)

**Mitigation Report (20%)**
- Proposed reasonable defenses (10%)
- Demonstrated understanding of tools (10%)

### 8. Safety Considerations

⚠️ **Important Security Notes:**

1. **Isolated Environment**: Run this exercise in an isolated environment
2. **Clear Marking**: Clearly mark all malicious code as "EDUCATIONAL ONLY"
3. **Cleanup**: Provide cleanup scripts to remove all traces
4. **Ethics Discussion**: Include discussion about responsible disclosure

### 9. Cleanup Script

Provide students with a cleanup script:

```bash
#!/bin/bash
# cleanup.sh

echo "Cleaning up supply chain exercise..."

# Remove password log
rm -f /tmp/.redis_debug.log

# Reset to safe dependencies
cd awesome-todo-app
git checkout Cargo.toml Cargo.lock
cargo clean

# Clear any environment variables
unset REDIS_BACKDOOR_CMD
unset REDIS_INSECURE_MODE

echo "Cleanup complete!"
```

### 10. Extension Ideas

**For Advanced Students:**
- Have them create their own malicious dependency
- Write detection tools
- Design a CI/CD pipeline to catch such attacks

**Blue Team Exercise:**
- Provide a compromised system
- Have students forensically analyze it
- Write incident response report

**Research Project:**
- Study real supply chain attacks
- Compare different ecosystems (npm, PyPI, crates.io)
- Propose improvements to package managers

## Troubleshooting

**Common Issues:**

1. **"Can't connect to Redis"**
   - Ensure Redis is running: `redis-cli ping`
   - Check REDIS_URL environment variable

2. **"Git dependency not downloading"**
   - Check network access to GitHub
   - Verify the repository is public

3. **"Vulnerabilities not triggering"**
   - Password logging only works in debug mode
   - Ensure proper environment variables are set

## Additional Resources

- [SLSA Framework](https://slsa.dev/) - Supply chain security framework
- [in-toto](https://in-toto.io/) - Supply chain security framework
- [Sigstore](https://www.sigstore.dev/) - Signing and verification
- [The Update Framework](https://theupdateframework.io/)

## Conclusion

This exercise provides hands-on experience with a realistic supply chain attack. Students will learn:
- How transitive dependencies create risk
- The importance of reviewing Cargo.lock changes
- Why git dependencies need extra scrutiny
- How attackers hide malicious code

Remember to emphasize that this is for educational purposes only and discuss the ethical implications of supply chain security.