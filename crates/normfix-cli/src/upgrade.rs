//! Self-update against the published releases.
//!
//! This is the only part of `normfix` that touches the network, and it does so
//! on one explicit command. It reuses the tools already required to install the
//! program instead of embedding an HTTP stack, so the trust boundary stays the
//! same one the installer documents: the archive is only accepted when its
//! digest matches the published `SHA256SUMS`.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const REPO: &str = "viniciusnevescosta/normfix";

/// How long a release check stays fresh.
///
/// A formatter should not talk to the network on every invocation, so the
/// answer is cached and the question is asked at most once a day.
const CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60;

/// Opt out of the periodic check entirely.
const OPT_OUT: &str = "NORMFIX_NO_UPDATE_CHECK";

/// What an upgrade attempt concluded.
pub(crate) enum Outcome {
    /// Already running the newest published version.
    Current(String),
    /// A newer version exists and was not installed because only a check was
    /// requested.
    Available { current: String, latest: String },
    /// The running binary was replaced.
    Installed { previous: String, installed: String },
}

/// Returns the release archive name for the running platform.
fn archive_name() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("normfix-x86_64-linux-gnu.tar.gz"),
        ("linux", "aarch64") => Ok("normfix-aarch64-linux-gnu.tar.gz"),
        ("macos", "x86_64") => Ok("normfix-x86_64-macos.tar.gz"),
        ("macos", "aarch64") => Ok("normfix-aarch64-macos.tar.gz"),
        (os, arch) => Err(format!(
            "no published archive for {os} {arch}; build from source instead"
        )),
    }
}

fn run(program: &str, arguments: &[&OsStr]) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run `{program}`: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(format!(
            "`{program}` failed{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        ));
    }
    Ok(output.stdout)
}

fn download(url: &str, target: &Path) -> Result<(), String> {
    if which("curl").is_some() {
        run(
            "curl",
            &[
                OsStr::new("-fsSL"),
                OsStr::new("--proto"),
                OsStr::new("=https"),
                OsStr::new("--tlsv1.2"),
                OsStr::new(url),
                OsStr::new("-o"),
                target.as_os_str(),
            ],
        )
        .map(|_| ())
    } else if which("wget").is_some() {
        run(
            "wget",
            &[OsStr::new("-qO"), target.as_os_str(), OsStr::new(url)],
        )
        .map(|_| ())
    } else {
        Err("upgrading needs curl or wget on PATH".to_owned())
    }
}

fn fetch_text(url: &str) -> Result<String, String> {
    // A predictable path under the shared temporary directory is a symbolic
    // link waiting to happen: on a multi-user machine another account can
    // create it first and redirect the download. TempDir creates a private
    // directory that cannot already exist.
    let directory = tempfile::TempDir::new()
        .map_err(|error| format!("could not create a private temporary directory: {error}"))?;
    let temporary = directory.path().join("response");
    download(url, &temporary)?;
    fs::read_to_string(&temporary)
        .map_err(|error| format!("could not read the downloaded response: {error}"))
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UpdateChannel {
    Stable,
    Preview,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

fn update_channel(current_version: &str) -> UpdateChannel {
    let without_build = current_version
        .split_once('+')
        .map_or(current_version, |(version, _)| version);
    if without_build.contains('-') {
        UpdateChannel::Preview
    } else {
        UpdateChannel::Stable
    }
}

fn release_metadata_url(channel: UpdateChannel) -> String {
    let endpoint = match channel {
        UpdateChannel::Stable => "releases/latest",
        UpdateChannel::Preview => "releases?per_page=100",
    };
    format!("https://api.github.com/repos/{REPO}/{endpoint}")
}

fn stable_tag(body: &str) -> Result<String, String> {
    let release = serde_json::from_str::<GithubRelease>(body)
        .map_err(|error| format!("the latest stable release response was invalid: {error}"))?;
    if release.draft || release.prerelease || release.tag_name.is_empty() {
        return Err("GitHub did not return a published stable release".to_owned());
    }
    Ok(release.tag_name)
}

fn newest_published_tag(body: &str) -> Result<String, String> {
    serde_json::from_str::<Vec<GithubRelease>>(body)
        .map_err(|error| format!("the release listing was invalid: {error}"))?
        .into_iter()
        .find(|release| !release.draft && !release.tag_name.is_empty())
        .map(|release| release.tag_name)
        .ok_or_else(|| "the release listing contained no published tag".to_owned())
}

/// Returns the newest tag permitted by the running version's update channel.
///
/// Stable binaries use GitHub's `/releases/latest` endpoint, which excludes
/// pre-releases. A running pre-release deliberately follows the complete
/// release feed, so testers can move to a newer preview (or its eventual
/// stable release) without reinstalling by hand.
fn newest_tag(current_version: &str) -> Result<String, String> {
    let channel = update_channel(current_version);
    let body = fetch_text(&release_metadata_url(channel))?;
    match channel {
        UpdateChannel::Stable => stable_tag(&body),
        UpdateChannel::Preview => newest_published_tag(&body),
    }
}

/// Rejects an install another package manager owns.
///
/// Replacing a Homebrew-managed binary leaves the formula describing something
/// that is no longer on disk, and the next `brew upgrade` silently undoes the
/// change.
fn reject_managed_install(executable: &Path) -> Result<(), String> {
    let path = executable.to_string_lossy();
    if path.contains("/Cellar/") || path.contains("/homebrew/") || path.contains("/linuxbrew/") {
        return Err(format!(
            "{path} is managed by Homebrew. Upgrade it with:\n  brew upgrade viniciusnevescosta/normfix/normfix"
        ));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut rendered, byte| {
            let _ = write!(rendered, "{byte:02x}");
            rendered
        })
}

fn staging_directory(parent: &Path) -> Result<tempfile::TempDir, String> {
    tempfile::Builder::new()
        .prefix(".normfix-upgrade-")
        .tempdir_in(parent)
        .map_err(|error| format!("could not create a private staging directory: {error}"))
}

/// Downloads, verifies and installs the newest release over the running binary.
pub(crate) fn upgrade(current_version: &str, check_only: bool) -> Result<Outcome, String> {
    let archive = archive_name()?;
    let latest = newest_tag(current_version)?;
    let latest_version = latest.strip_prefix('v').unwrap_or(&latest);

    if latest_version == current_version {
        return Ok(Outcome::Current(current_version.to_owned()));
    }
    if check_only {
        return Ok(Outcome::Available {
            current: current_version.to_owned(),
            latest: latest_version.to_owned(),
        });
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the running binary: {error}"))?;
    reject_managed_install(&executable)?;
    let directory = executable
        .parent()
        .ok_or_else(|| "the running binary has no parent directory".to_owned())?;

    // Stage in a fresh private directory next to the binary. Sharing the
    // destination filesystem makes the final rename atomic, while the random
    // create-new path prevents another account from pre-creating a symlink at
    // the old process-id-based name.
    let staging = staging_directory(directory)?;
    install_into(staging.path(), &executable, &latest, archive)?;

    Ok(Outcome::Installed {
        previous: current_version.to_owned(),
        installed: latest_version.to_owned(),
    })
}

fn install_into(staging: &Path, executable: &Path, tag: &str, archive: &str) -> Result<(), String> {
    let base = format!("https://github.com/{REPO}/releases/download/{tag}");
    let archive_path = staging.join(archive);
    download(&format!("{base}/{archive}"), &archive_path)?;
    let sums = fetch_text(&format!("{base}/SHA256SUMS"))?;

    let expected = sums
        .lines()
        .find_map(|line| {
            let (digest, name) = line.split_once(char::is_whitespace)?;
            (name.trim().trim_start_matches('*') == archive).then(|| digest.trim().to_owned())
        })
        .ok_or_else(|| format!("{archive} is not listed in SHA256SUMS"))?;

    let bytes = fs::read(&archive_path)
        .map_err(|error| format!("could not read the downloaded archive: {error}"))?;
    let actual = digest(&bytes);
    if actual != expected {
        return Err(format!(
            "checksum mismatch for {archive}\n  expected {expected}\n  actual   {actual}\nRefusing to install."
        ));
    }

    run(
        "tar",
        &[
            OsStr::new("-xzf"),
            archive_path.as_os_str(),
            OsStr::new("-C"),
            staging.as_os_str(),
        ],
    )?;
    let extracted = staging.join("normfix");
    if !extracted.is_file() {
        return Err("the archive did not contain a normfix binary".to_owned());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&extracted, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("could not make the new binary executable: {error}"))?;
    }

    // Replacing a running executable is allowed on Unix: the running image
    // keeps the old inode until it exits.
    fs::rename(&extracted, executable).map_err(|error| {
        format!(
            "could not replace {}: {error}. Check that you own the file.",
            executable.display()
        )
    })
}

/// Prints one line when a newer release exists.
///
/// This is the only network access outside `normfix upgrade`, and it is
/// deliberately narrow: it runs at most once a day, only for interactive human
/// output, never for JSON or a non-terminal, and any failure is silent. Set
/// `NORMFIX_NO_UPDATE_CHECK` to disable it.
pub(crate) fn notify_if_outdated(current_version: &str) {
    if std::env::var_os(OPT_OUT).is_some() {
        return;
    }
    let Some(state) = state_path() else {
        return;
    };
    if !is_stale(&state) {
        return;
    }
    // Record the attempt first, so a network that never answers cannot make
    // every future run pay for the same lookup.
    let _ = fs::create_dir_all(state.parent().unwrap_or(&state));
    let _ = fs::write(&state, now_seconds().to_string());

    let Ok(latest) = newest_tag(current_version) else {
        return;
    };
    let latest_version = latest.strip_prefix('v').unwrap_or(&latest);
    if latest_version != current_version {
        eprintln!(
            "\nnormfix {latest_version} is available; this is {current_version}. Run `normfix upgrade`."
        );
    }
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn state_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("normfix").join("last-update-check"))
}

fn is_stale(state: &Path) -> bool {
    let Ok(recorded) = fs::read_to_string(state) else {
        return true;
    };
    let Ok(checked_at) = recorded.trim().parse::<u64>() else {
        return true;
    };
    now_seconds().saturating_sub(checked_at) >= CHECK_INTERVAL_SECONDS
}

#[cfg(test)]
mod tests {
    use super::{
        UpdateChannel, newest_published_tag, release_metadata_url, stable_tag, staging_directory,
        update_channel,
    };

    #[test]
    fn stable_versions_stay_on_the_stable_channel() {
        assert_eq!(update_channel("1.0.0"), UpdateChannel::Stable);
        assert_eq!(update_channel("1.0.0+linux-gnu"), UpdateChannel::Stable);
    }

    #[test]
    fn prerelease_versions_follow_the_preview_channel() {
        assert_eq!(update_channel("1.0.0-rc.1"), UpdateChannel::Preview);
        assert_eq!(
            update_channel("1.0.0-beta.2+linux-gnu"),
            UpdateChannel::Preview
        );
    }

    #[test]
    fn channels_use_distinct_github_endpoints() {
        assert_eq!(
            release_metadata_url(UpdateChannel::Stable),
            "https://api.github.com/repos/viniciusnevescosta/normfix/releases/latest"
        );
        assert_eq!(
            release_metadata_url(UpdateChannel::Preview),
            "https://api.github.com/repos/viniciusnevescosta/normfix/releases?per_page=100"
        );
    }

    #[test]
    fn stable_endpoint_accepts_only_a_published_stable_release() {
        assert_eq!(
            stable_tag(r#"{"tag_name":"v1.0.0","draft":false,"prerelease":false}"#)
                .expect("stable tag"),
            "v1.0.0"
        );
        assert!(
            stable_tag(r#"{"tag_name":"v1.1.0-rc.1","draft":false,"prerelease":true}"#).is_err()
        );
        assert!(stable_tag(r#"{"tag_name":"v1.0.0","draft":true,"prerelease":false}"#).is_err());
        assert!(stable_tag(r#"{"tag_name":"","draft":false,"prerelease":false}"#).is_err());
        assert!(stable_tag(r#"{"tag_name":"v1.0.0"}"#).is_err());
    }

    #[test]
    fn preview_feed_uses_the_newest_non_draft_release() {
        let body = r#"[
            {"tag_name":"v1.1.0-rc.2","draft":true,"prerelease":true},
            {"tag_name":"v1.1.0-rc.1","draft":false,"prerelease":true},
            {"tag_name":"v1.0.0","draft":false,"prerelease":false}
        ]"#;
        assert_eq!(
            newest_published_tag(body).expect("published tag"),
            "v1.1.0-rc.1"
        );
    }

    #[test]
    fn preview_feed_can_advance_to_the_eventual_stable_release() {
        let body = r#"[
            {"tag_name":"v1.0.0","draft":false,"prerelease":false},
            {"tag_name":"v1.0.0-rc.1","draft":false,"prerelease":true}
        ]"#;
        assert_eq!(newest_published_tag(body).expect("published tag"), "v1.0.0");
    }

    #[test]
    fn malformed_or_empty_release_feeds_fail_closed() {
        assert!(stable_tag("not json").is_err());
        assert!(newest_published_tag("not json").is_err());
        assert!(newest_published_tag("[]").is_err());
        assert!(
            newest_published_tag(r#"[{"tag_name":"v2.0.0","draft":true,"prerelease":false}]"#)
                .is_err()
        );
    }

    #[test]
    fn staging_directories_are_private_unique_and_self_cleaning() {
        let parent = tempfile::TempDir::new().expect("parent");
        let first = staging_directory(parent.path()).expect("first staging directory");
        let second = staging_directory(parent.path()).expect("second staging directory");
        let first_path = first.path().to_path_buf();
        let second_path = second.path().to_path_buf();

        assert_ne!(first_path, second_path);
        assert_eq!(first_path.parent(), Some(parent.path()));
        assert_eq!(second_path.parent(), Some(parent.path()));
        assert!(first_path.is_dir());
        assert!(second_path.is_dir());

        drop(first);
        drop(second);
        assert!(!first_path.exists());
        assert!(!second_path.exists());
    }
}
