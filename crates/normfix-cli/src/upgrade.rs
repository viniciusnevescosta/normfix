//! Self-update against the published releases.
//!
//! This is the only part of `normfix` that touches the network, and it does so
//! on one explicit command. It reuses the tools already required to install the
//! program instead of embedding an HTTP stack, so the trust boundary stays the
//! same one the installer documents: the archive is only accepted when its
//! digest matches the published `SHA256SUMS`.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const REPO: &str = "viniciusnevescosta/normfix";
const MAX_DOWNLOAD_BYTES: u64 = 128 * 1024 * 1024;

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
        ("freebsd", "x86_64") => Ok("normfix-x86_64-freebsd.tar.gz"),
        ("windows", _) => Err(
            "Windows cannot replace an executable while it is running; rerun the verified installer, or use `scoop update normfix` for a Scoop install"
                .to_owned(),
        ),
        (os, arch) => Err(format!(
            "no published archive for {os} {arch}; build from source instead"
        )),
    }
}

fn run(program: &Path, arguments: &[&OsStr]) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run `{}`: {error}", program.display()))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(format!(
            "`{}` failed{}",
            program.display(),
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
    let result = if let Some(curl) = which("curl") {
        run(
            &curl,
            &[
                OsStr::new("-fsSL"),
                OsStr::new("--proto"),
                OsStr::new("=https"),
                OsStr::new("--tlsv1.2"),
                OsStr::new("--connect-timeout"),
                OsStr::new("10"),
                OsStr::new("--max-time"),
                OsStr::new("120"),
                OsStr::new("--max-filesize"),
                OsStr::new("134217728"),
                OsStr::new(url),
                OsStr::new("-o"),
                target.as_os_str(),
            ],
        )
        .map(|_| ())
    } else if let Some(wget) = which("wget") {
        run(
            &wget,
            &[
                OsStr::new("-q"),
                OsStr::new("-T"),
                OsStr::new("120"),
                OsStr::new("-t"),
                OsStr::new("1"),
                OsStr::new("-O"),
                target.as_os_str(),
                OsStr::new(url),
            ],
        )
        .map(|_| ())
    } else {
        Err("upgrading needs curl or wget on PATH".to_owned())
    };
    result?;
    let size = fs::metadata(target)
        .map_err(|error| format!("could not inspect the download: {error}"))?
        .len();
    if size > MAX_DOWNLOAD_BYTES {
        let _ = fs::remove_file(target);
        return Err(format!(
            "the download exceeded the {MAX_DOWNLOAD_BYTES}-byte safety limit"
        ));
    }
    Ok(())
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
    which_in_path(program, &path)
}

fn which_in_path(program: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| executable_candidate(&directory, program))
}

#[cfg(not(windows))]
fn executable_candidate(directory: &Path, program: &str) -> Option<PathBuf> {
    let candidate = directory.join(program);
    candidate.is_file().then_some(candidate)
}

#[cfg(windows)]
fn executable_candidate(directory: &Path, program: &str) -> Option<PathBuf> {
    let direct = directory.join(program);
    if direct.is_file() {
        return Some(direct);
    }
    let extensions = std::env::var_os("PATHEXT")
        .unwrap_or_else(|| OsStr::new(".COM;.EXE;.BAT;.CMD").to_os_string());
    extensions
        .to_string_lossy()
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(|extension| directory.join(format!("{program}{extension}")))
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum PrereleaseIdentifier {
    Numeric(u64),
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseVersion {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<PrereleaseIdentifier>,
}

impl Ord for ReleaseVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(
                || match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    _ => self
                        .prerelease
                        .iter()
                        .zip(&other.prerelease)
                        .find_map(|(left, right)| {
                            let ordering = match (left, right) {
                                (
                                    PrereleaseIdentifier::Numeric(left),
                                    PrereleaseIdentifier::Numeric(right),
                                ) => left.cmp(right),
                                (
                                    PrereleaseIdentifier::Numeric(_),
                                    PrereleaseIdentifier::Text(_),
                                ) => Ordering::Less,
                                (
                                    PrereleaseIdentifier::Text(_),
                                    PrereleaseIdentifier::Numeric(_),
                                ) => Ordering::Greater,
                                (
                                    PrereleaseIdentifier::Text(left),
                                    PrereleaseIdentifier::Text(right),
                                ) => left.cmp(right),
                            };
                            (ordering != Ordering::Equal).then_some(ordering)
                        })
                        .unwrap_or_else(|| self.prerelease.len().cmp(&other.prerelease.len())),
                },
            )
    }
}

impl PartialOrd for ReleaseVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn parse_release_version(value: &str) -> Result<ReleaseVersion, String> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let (without_build, build) = value
        .split_once('+')
        .map_or((value, None), |(version, build)| (version, Some(build)));
    if build.is_some_and(|build| !valid_identifiers(build, false)) || without_build.contains('+') {
        return Err(format!("invalid release version `{value}`"));
    }
    let (core, prerelease) = without_build
        .split_once('-')
        .map_or((without_build, None), |(core, prerelease)| {
            (core, Some(prerelease))
        });
    let mut numbers = core.split('.');
    let major = parse_core_number(numbers.next(), value)?;
    let minor = parse_core_number(numbers.next(), value)?;
    let patch = parse_core_number(numbers.next(), value)?;
    if numbers.next().is_some() {
        return Err(format!("invalid release version `{value}`"));
    }
    let prerelease = prerelease.map_or_else(
        || Ok(Vec::new()),
        |prerelease| {
            if !valid_identifiers(prerelease, true) {
                return Err(format!("invalid release version `{value}`"));
            }
            prerelease
                .split('.')
                .map(|identifier| {
                    if identifier.bytes().all(|byte| byte.is_ascii_digit()) {
                        identifier
                            .parse::<u64>()
                            .map(PrereleaseIdentifier::Numeric)
                            .map_err(|_| format!("invalid release version `{value}`"))
                    } else {
                        Ok(PrereleaseIdentifier::Text(identifier.to_owned()))
                    }
                })
                .collect::<Result<Vec<_>, _>>()
        },
    )?;
    Ok(ReleaseVersion {
        major,
        minor,
        patch,
        prerelease,
    })
}

fn parse_core_number(number: Option<&str>, version: &str) -> Result<u64, String> {
    let Some(number) = number else {
        return Err(format!("invalid release version `{version}`"));
    };
    if number.is_empty()
        || (number.len() > 1 && number.starts_with('0'))
        || !number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("invalid release version `{version}`"));
    }
    number
        .parse::<u64>()
        .map_err(|_| format!("invalid release version `{version}`"))
}

fn valid_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !(reject_numeric_leading_zero
                    && identifier.len() > 1
                    && identifier.starts_with('0')
                    && identifier.bytes().all(|byte| byte.is_ascii_digit()))
        })
}

fn published_version(tag: &str) -> Result<ReleaseVersion, String> {
    if !tag.starts_with('v') {
        return Err(format!(
            "published release tag `{tag}` does not start with `v`"
        ));
    }
    parse_release_version(tag)
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
    let version = published_version(&release.tag_name)?;
    if !version.prerelease.is_empty() {
        return Err("GitHub marked a SemVer pre-release as stable".to_owned());
    }
    Ok(release.tag_name)
}

fn newest_published_tag(body: &str) -> Result<String, String> {
    serde_json::from_str::<Vec<GithubRelease>>(body)
        .map_err(|error| format!("the release listing was invalid: {error}"))?
        .into_iter()
        .filter(|release| !release.draft && !release.tag_name.is_empty())
        .map(|release| {
            published_version(&release.tag_name).map(|version| (version, release.tag_name))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, tag)| tag)
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
/// Replacing a binary a package manager installed leaves its manifest
/// describing something that is no longer on disk, and the manager's next
/// upgrade silently undoes the change. Scoop needs this as much as Homebrew
/// does: it keeps the binary under its own `apps` tree and a shim pointing at
/// it, so overwriting the target leaves the shim aimed at bytes Scoop did not
/// put there.
fn reject_managed_install(executable: &Path) -> Result<(), String> {
    let path = executable.to_string_lossy();
    // `linuxbrew/` without a leading slash on purpose: the install lives at
    // `~/.linuxbrew/` or `/home/linuxbrew/.linuxbrew/`, and requiring the slash
    // matched neither.
    if path.contains("/Cellar/") || path.contains("/homebrew/") || path.contains("linuxbrew/") {
        return Err(format!(
            "{path} is managed by Homebrew; upgrade it with `brew upgrade viniciusnevescosta/normfix/normfix`"
        ));
    }
    let lowered = path.to_lowercase().replace('\\', "/");
    if lowered.contains("/scoop/apps/") || lowered.contains("/scoop/shims/") {
        return Err(format!(
            "{path} is managed by Scoop; upgrade it with `scoop update normfix`"
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
    let latest = newest_tag(current_version)?;
    let latest_version = latest.strip_prefix('v').unwrap_or(&latest);
    let current_precedence = parse_release_version(current_version)?;
    let latest_precedence = published_version(&latest)?;

    if latest_precedence <= current_precedence {
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
    let archive = archive_name()?;
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

    let expected = published_checksum(&sums, archive)?;

    let bytes = fs::read(&archive_path)
        .map_err(|error| format!("could not read the downloaded archive: {error}"))?;
    let actual = digest(&bytes);
    if actual != expected {
        return Err(format!(
            "checksum mismatch for {archive}\n  expected {expected}\n  actual   {actual}\nRefusing to install."
        ));
    }

    let tar_executable = which("tar").ok_or_else(|| "upgrading needs tar on PATH".to_owned())?;
    validate_archive_listing(&archive_path, &tar_executable)?;
    run(
        &tar_executable,
        &[
            OsStr::new("-xzf"),
            archive_path.as_os_str(),
            OsStr::new("-C"),
            staging.as_os_str(),
        ],
    )?;
    let extracted = staging.join("normfix");
    let metadata = fs::symlink_metadata(&extracted)
        .map_err(|_| "the archive did not contain a normfix binary".to_owned())?;
    if !metadata.file_type().is_file() {
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

fn published_checksum(sums: &str, archive: &str) -> Result<String, String> {
    let matching = sums
        .lines()
        .filter_map(|line| {
            let (digest, name) = line.split_once(char::is_whitespace)?;
            (name.trim().trim_start_matches('*') == archive).then(|| digest.trim())
        })
        .collect::<Vec<_>>();
    let [digest] = matching.as_slice() else {
        return Err(format!(
            "SHA256SUMS must list {archive} exactly once; found {} entries",
            matching.len()
        ));
    };
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("SHA256SUMS has an invalid digest for {archive}"));
    }
    Ok(digest.to_ascii_lowercase())
}

fn validate_archive_listing(archive: &Path, tar: &Path) -> Result<(), String> {
    let listing = run(tar, &[OsStr::new("-tzf"), archive.as_os_str()])?;
    let listing = String::from_utf8(listing)
        .map_err(|_| "the release archive contained a non-UTF-8 path".to_owned())?;
    let mut entries = listing.lines().collect::<Vec<_>>();
    entries.sort_unstable();
    if entries != ["LICENSE", "README.md", "normfix"] {
        return Err(format!(
            "the release archive contained unexpected entries: {}",
            entries.join(", ")
        ));
    }
    Ok(())
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
    record_check_attempt(&state);

    let Ok(latest) = newest_tag(current_version) else {
        return;
    };
    let latest_version = latest.strip_prefix('v').unwrap_or(&latest);
    let newer = published_version(&latest)
        .and_then(|latest| parse_release_version(current_version).map(|current| latest > current))
        .unwrap_or(false);
    if newer {
        if cfg!(windows) {
            eprintln!(
                "\nnormfix {latest_version} is available; this is {current_version}. Update with Scoop, or rerun the verified installer."
            );
        } else {
            eprintln!(
                "\nnormfix {latest_version} is available; this is {current_version}. Run `normfix upgrade`."
            );
        }
    }
}

fn record_check_attempt(state: &Path) {
    let Some(parent) = state.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    // Write through a random create-new file and rename it into place. A
    // pre-existing symbolic link at the cache path is replaced, never followed.
    let Ok(mut temporary) = tempfile::NamedTempFile::new_in(parent) else {
        return;
    };
    if write!(temporary, "{}", now_seconds()).is_err() || temporary.as_file().sync_all().is_err() {
        return;
    }
    let _ = temporary.persist(state);
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
        UpdateChannel, newest_published_tag, parse_release_version, published_checksum,
        record_check_attempt, reject_managed_install, release_metadata_url, stable_tag,
        staging_directory, update_channel, which_in_path,
    };

    #[test]
    fn executable_discovery_ignores_relative_path_entries() {
        let trusted = tempfile::TempDir::new().expect("trusted directory");
        let executable = trusted.path().join("normfix-test-tool");
        std::fs::write(&executable, "fixture").expect("executable fixture");
        let path =
            std::env::join_paths([std::path::Path::new("."), trusted.path()]).expect("test PATH");

        assert_eq!(which_in_path("normfix-test-tool", &path), Some(executable));
        assert!(which_in_path("normfix-test-tool", std::ffi::OsStr::new(".")).is_none());
    }

    #[test]
    fn release_versions_follow_semver_precedence_and_reject_ambiguous_tags() {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(
                parse_release_version(pair[0]).expect("left version")
                    < parse_release_version(pair[1]).expect("right version"),
                "{} must precede {}",
                pair[0],
                pair[1]
            );
        }
        assert_eq!(
            parse_release_version("1.0.0+first").expect("build metadata"),
            parse_release_version("v1.0.0+second").expect("build metadata")
        );
        for invalid in [
            "1.0",
            "01.0.0",
            "1.0.0-01",
            "1.0.0-",
            "1.0.0+",
            "1.0.0/asset",
            "18446744073709551616.0.0",
        ] {
            assert!(
                parse_release_version(invalid).is_err(),
                "{invalid} must be rejected"
            );
        }
    }

    #[test]
    fn a_binary_a_package_manager_owns_is_never_replaced() {
        use std::path::Path;

        // Overwriting one leaves the manager's manifest describing bytes that
        // are no longer there, and its next upgrade silently undoes the change.
        for (path, manager, command) in [
            (
                "/opt/homebrew/Cellar/normfix/1.6.2/bin/normfix",
                "Homebrew",
                "brew upgrade",
            ),
            (
                "/home/me/.linuxbrew/bin/normfix",
                "Homebrew",
                "brew upgrade",
            ),
            (
                "C:\\Users\\me\\scoop\\apps\\normfix\\current\\normfix.exe",
                "Scoop",
                "scoop update",
            ),
            (
                "C:\\Users\\me\\scoop\\shims\\normfix.exe",
                "Scoop",
                "scoop update",
            ),
        ] {
            let refusal = reject_managed_install(Path::new(path))
                .expect_err("a managed install must be refused");
            assert!(refusal.contains(manager), "{path}: {refusal}");
            assert!(refusal.contains(command), "{path}: {refusal}");
        }
    }

    #[test]
    fn an_ordinary_install_is_left_alone() {
        use std::path::Path;

        for path in ["/home/me/.local/bin/normfix", "/usr/local/bin/normfix"] {
            assert!(reject_managed_install(Path::new(path)).is_ok(), "{path}");
        }
    }

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
    fn preview_feed_selects_semantic_newest_even_when_api_order_is_unhelpful() {
        let body = r#"[
            {"tag_name":"v1.9.0","draft":false,"prerelease":false},
            {"tag_name":"v2.0.0-rc.1","draft":false,"prerelease":true},
            {"tag_name":"v1.10.0","draft":false,"prerelease":false}
        ]"#;
        assert_eq!(
            newest_published_tag(body).expect("newest tag"),
            "v2.0.0-rc.1"
        );
    }

    #[test]
    fn checksum_manifest_requires_one_well_formed_entry() {
        let digest = "a".repeat(64);
        let line = format!("{digest}  normfix-x86_64-linux-gnu.tar.gz\n");
        assert_eq!(
            published_checksum(&line, "normfix-x86_64-linux-gnu.tar.gz").expect("valid checksum"),
            digest
        );
        assert!(
            published_checksum(&format!("{line}{line}"), "normfix-x86_64-linux-gnu.tar.gz")
                .is_err()
        );
        assert!(
            published_checksum(
                "xyz  normfix-x86_64-linux-gnu.tar.gz\n",
                "normfix-x86_64-linux-gnu.tar.gz"
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn update_check_timestamp_replaces_a_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::TempDir::new().expect("temporary directory");
        let victim = directory.path().join("victim");
        let state = directory.path().join("last-update-check");
        std::fs::write(&victim, "do not overwrite").expect("write victim");
        symlink(&victim, &state).expect("create state symlink");

        record_check_attempt(&state);

        assert_eq!(
            std::fs::read_to_string(&victim).expect("read victim"),
            "do not overwrite"
        );
        assert!(
            std::fs::symlink_metadata(&state)
                .expect("state metadata")
                .file_type()
                .is_file()
        );
        assert!(
            std::fs::read_to_string(&state)
                .expect("state timestamp")
                .parse::<u64>()
                .is_ok()
        );
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
