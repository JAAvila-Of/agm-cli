//! Self-update command: downloads the latest release from GitHub.

/// Run the update command.
///
/// If `check_only` is true, prints version information without installing.
/// Returns 0 on success, 1 on error.
#[cfg(feature = "self-update")]
pub fn run(check_only: bool) -> i32 {
    let current = env!("CARGO_PKG_VERSION");

    if check_only {
        match check_latest(current) {
            Ok(Some(latest)) => {
                println!("Current version: {current}");
                println!("Latest version:  {latest}");
                println!("\nRun `agm update` to install the latest version.");
                0
            }
            Ok(None) => {
                println!("agm {current} is up to date.");
                0
            }
            Err(e) => {
                eprintln!("Error checking for updates: {e}");
                1
            }
        }
    } else {
        println!("Updating agm from v{current}...");
        match do_update() {
            Ok(status) => {
                if status.updated() {
                    println!("Updated to v{}.", status.version());
                } else {
                    println!("agm {current} is already the latest version.");
                }
                0
            }
            Err(e) => {
                eprintln!("Update failed: {e}");
                eprintln!("\nYou can update manually by downloading from:");
                eprintln!("  https://github.com/JAAvila-Of/agm-cli/releases");
                1
            }
        }
    }
}

#[cfg(feature = "self-update")]
fn check_latest(current: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner("JAAvila-Of")
        .repo_name("agm-cli")
        .build()?
        .fetch()?;

    if let Some(latest) = releases.first() {
        let latest_ver = latest.version.trim_start_matches('v');
        if latest_ver != current {
            return Ok(Some(latest_ver.to_string()));
        }
    }
    Ok(None)
}

#[cfg(feature = "self-update")]
fn do_update() -> Result<self_update::Status, Box<dyn std::error::Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("JAAvila-Of")
        .repo_name("agm-cli")
        .bin_name("agm")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()?;
    Ok(status)
}

#[cfg(not(feature = "self-update"))]
pub fn run(_check_only: bool) -> i32 {
    eprintln!("Self-update is not available in this build.");
    eprintln!("Download the latest version from:");
    eprintln!("  https://github.com/JAAvila-Of/agm-cli/releases");
    1
}
