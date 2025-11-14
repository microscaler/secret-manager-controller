fn main() {
    // Set build timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    
    // Set build date/time in human-readable format
    let datetime = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    println!("cargo:rustc-env=BUILD_DATETIME={}", datetime);
    
    // Get git commit hash
    let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_GIT_HASH={}", git_hash);
    
    // Re-run if this build script changes
    println!("cargo:rerun-if-changed=build.rs");
}

fn get_git_hash() -> Option<String> {
    // Try to get git hash using git2 crate
    let repo = git2::Repository::open(".").ok()?;
    let head = repo.head().ok()?;
    let oid = head.target()?;
    let _commit = repo.find_commit(oid).ok()?;
    
    // Get short hash (first 7 characters)
    let hash = oid.to_string();
    let short_hash = hash.chars().take(7).collect::<String>();
    
    // Check if working directory is dirty
    let is_dirty = repo
        .diff_index_to_workdir(None, None)
        .ok()
        .map(|diff| diff.deltas().len() > 0)
        .unwrap_or(false);
    
    let suffix = if is_dirty { "-dirty" } else { "" };
    Some(format!("{}{}", short_hash, suffix))
}

