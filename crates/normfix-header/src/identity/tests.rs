use std::fs;

#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt as _, symlink};

use tempfile::TempDir;

use super::config::{persist_identity_at, persist_identity_with_directory_policy_for_test};
use super::{Identity42, IdentityResolver, canonical_42_email, identity_from_email};

fn identity() -> Identity42 {
    identity_from_email("student-a@student.42.fr", Some("student-a"), "test")
        .identity
        .expect("valid identity")
}

#[test]
fn validates_supported_campus_domains_and_canonicalizes_case() {
    assert_eq!(
        canonical_42_email("Student-A@Student.42Berlin.DE"),
        Some("student-a@student.42berlin.de".to_owned())
    );
    assert!(canonical_42_email("student@example.com").is_none());
    assert!(canonical_42_email("bad%login@student.42.fr").is_none());
}

#[test]
fn explicit_email_has_precedence_and_must_match_explicit_login() {
    let temporary = TempDir::new().expect("temporary directory");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_EMAIL", "env@student.42.fr");
    let resolution = resolver.resolve(Some("cli"), Some("cli@student.42.fr"), temporary.path());
    let identity = resolution.identity.expect("valid identity");
    assert_eq!(identity.login, "cli");
    assert!(!identity.inferred());
}

#[test]
fn invalid_explicit_email_never_falls_through_to_a_lower_precedence_source() {
    let temporary = TempDir::new().expect("temporary directory");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_EMAIL", "valid@student.42.fr");
    let resolution = resolver.resolve(None, Some("not-a-42-address"), temporary.path());
    assert!(!resolution.is_available());
    assert_eq!(
        resolution.issue.expect("invalid email issue").code,
        "IDENTITY_INVALID_EMAIL"
    );
}

#[test]
fn config_precedes_editor_settings() {
    let temporary = TempDir::new().expect("temporary directory");
    let config = temporary.path().join("config.ini");
    fs::write(
        &config,
        "[header]\nlogin = configured\nemail = configured@student.42.fr\n",
    )
    .expect("config");
    fs::write(
        temporary.path().join(".vimrc"),
        "let g:mail42 = 'editor@student.42.fr'\n",
    )
    .expect("vimrc");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_CONFIG", config.to_string_lossy());

    let resolution = resolver.resolve(None, None, temporary.path());

    assert_eq!(
        resolution.identity.expect("configured identity").login,
        "configured"
    );
}

#[test]
fn ambiguous_duplicate_config_values_are_rejected_as_a_unit() {
    let temporary = TempDir::new().expect("temporary directory");
    let config = temporary.path().join("config.ini");
    fs::write(
        &config,
        concat!(
            "[header]\n",
            "email = first@student.42.fr\n",
            "email = second@student.42.fr\n"
        ),
    )
    .expect("config");
    fs::write(
        temporary.path().join(".vimrc"),
        "let g:mail42 = 'editor@student.42.fr'\n",
    )
    .expect("vimrc");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_CONFIG", config.to_string_lossy());

    let resolution = resolver.resolve(None, None, temporary.path());

    assert_eq!(
        resolution
            .identity
            .expect("unambiguous editor identity")
            .email,
        "editor@student.42.fr"
    );
}

#[test]
fn canonical_environment_has_precedence_over_legacy_aliases() {
    let temporary = TempDir::new().expect("temporary directory");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_LOGIN", "canonical")
        .with_environment("NORMFIX_EMAIL", "canonical@student.42.fr")
        .with_environment("NORMINETTE_FIX_LOGIN", "legacy")
        .with_environment("NORMINETTE_FIX_EMAIL", "legacy@student.42.fr");

    let resolution = resolver.resolve(None, None, temporary.path());

    let identity = resolution.identity.expect("canonical environment");
    assert_eq!(identity.login, "canonical");
    assert_eq!(identity.email, "canonical@student.42.fr");
}

#[test]
fn legacy_environment_aliases_remain_supported() {
    let temporary = TempDir::new().expect("temporary directory");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMINETTE_FIX_LOGIN", "legacy")
        .with_environment("NORMINETTE_FIX_EMAIL", "legacy@student.42.fr");

    let resolution = resolver.resolve(None, None, temporary.path());

    let identity = resolution.identity.expect("legacy environment");
    assert_eq!(identity.login, "legacy");
    assert_eq!(identity.email, "legacy@student.42.fr");
}

#[test]
fn default_config_prefers_normfix_and_falls_back_to_the_legacy_directory() {
    let temporary = TempDir::new().expect("temporary directory");
    let config_base = temporary.path().join("config");
    let legacy = config_base.join("norminette-fix/config.ini");
    fs::create_dir_all(legacy.parent().expect("legacy config parent"))
        .expect("legacy config directory");
    fs::write(
        &legacy,
        "[header]\nlogin = legacy\nemail = legacy@student.42.fr\n",
    )
    .expect("legacy config");
    let legacy_resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("XDG_CONFIG_HOME", config_base.to_string_lossy());

    let legacy_identity = legacy_resolver
        .resolve(None, None, temporary.path())
        .identity
        .expect("legacy default config");
    assert_eq!(legacy_identity.login, "legacy");

    let canonical = config_base.join("normfix/config.ini");
    fs::create_dir_all(canonical.parent().expect("canonical config parent"))
        .expect("canonical config directory");
    fs::write(
        canonical,
        "[header]\nlogin = canonical\nemail = canonical@student.42.fr\n",
    )
    .expect("canonical config");

    let canonical_identity = legacy_resolver
        .resolve(None, None, temporary.path())
        .identity
        .expect("canonical default config");
    assert_eq!(canonical_identity.login, "canonical");
}

#[test]
fn persisted_identity_is_resolved_on_the_next_run() {
    let temporary = TempDir::new().expect("temporary directory");
    let config = temporary.path().join("normfix/config.ini");

    persist_identity_at(&identity(), &config).expect("persist identity");

    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_CONFIG", config.to_string_lossy());
    let result = resolver.resolve(None, None, temporary.path());
    assert_eq!(
        result.identity.expect("saved identity").email,
        "student-a@student.42.fr"
    );
}

#[cfg(unix)]
#[test]
fn persisted_identity_uses_owner_only_permissions() {
    let temporary = TempDir::new().expect("temporary directory");
    let config = temporary.path().join("normfix/config.ini");

    persist_identity_at(&identity(), &config).expect("persist identity");

    let file_mode = fs::metadata(&config)
        .expect("config metadata")
        .permissions()
        .mode();
    let directory_mode = fs::metadata(config.parent().expect("config parent"))
        .expect("directory metadata")
        .permissions()
        .mode();
    assert_eq!(file_mode & 0o777, 0o600);
    assert_eq!(directory_mode & 0o777, 0o700);
}

#[cfg(unix)]
#[test]
fn explicit_config_does_not_change_permissions_of_a_shared_parent() {
    let temporary = TempDir::new().expect("temporary directory");
    let shared = temporary.path().join("shared");
    fs::create_dir(&shared).expect("shared parent");
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).expect("shared permissions");
    let config = shared.join("identity.ini");

    persist_identity_with_directory_policy_for_test(&identity(), &config, false)
        .expect("persist explicit config");

    let directory_mode = fs::metadata(shared)
        .expect("directory metadata")
        .permissions()
        .mode();
    assert_eq!(directory_mode & 0o777, 0o755);
    assert_eq!(
        fs::metadata(config)
            .expect("config metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn persistence_refuses_a_symbolic_link_destination() {
    let temporary = TempDir::new().expect("temporary directory");
    let outside = temporary.path().join("outside.ini");
    let config = temporary.path().join("config.ini");
    fs::write(&outside, "untouched\n").expect("outside file");
    symlink(&outside, &config).expect("config symlink");

    let error = persist_identity_at(&identity(), &config).expect_err("must reject symlink");

    assert!(error.to_string().contains("not a regular file"));
    assert_eq!(
        fs::read_to_string(outside).expect("outside contents"),
        "untouched\n"
    );
}

#[cfg(unix)]
#[test]
fn resolution_refuses_a_symbolic_link_configuration() {
    let temporary = TempDir::new().expect("temporary directory");
    let outside = temporary.path().join("outside.ini");
    let config = temporary.path().join("config.ini");
    fs::write(
        &outside,
        "[header]\nlogin = linked\nemail = linked@student.42.fr\n",
    )
    .expect("outside config");
    symlink(outside, &config).expect("config symlink");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_CONFIG", config.to_string_lossy());

    let result = resolver.resolve(None, None, temporary.path());

    assert!(!result.is_available());
}

#[test]
fn oversized_configuration_is_ignored_without_an_unbounded_read() {
    let temporary = TempDir::new().expect("temporary directory");
    let config = temporary.path().join("config.ini");
    fs::write(&config, vec![b'x'; 1_000_001]).expect("oversized config");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("NORMFIX_CONFIG", config.to_string_lossy());

    let result = resolver.resolve(None, None, temporary.path());

    assert!(!result.is_available());
}

#[test]
fn ambiguous_editor_emails_are_never_guessed() {
    let temporary = TempDir::new().expect("temporary directory");
    fs::write(
        temporary.path().join(".vimrc"),
        "let g:mail42 = 'first@student.42.fr'\n",
    )
    .expect("vimrc");
    let settings = temporary
        .path()
        .join("Library/Application Support/Code/User/settings.json");
    fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    fs::write(settings, r#"{"42header.email":"second@student.42.fr"}"#).expect("settings");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()));

    let resolution = resolver.resolve(None, None, temporary.path());

    assert!(!resolution.is_available());
    assert!(resolution.source.contains("multiple 42 student emails"));
}

#[test]
fn requested_login_selects_one_of_multiple_saved_emails() {
    let temporary = TempDir::new().expect("temporary directory");
    fs::write(
        temporary.path().join(".vimrc"),
        "let g:mail42 = 'first@student.42.fr'\n",
    )
    .expect("vimrc");
    let settings = temporary
        .path()
        .join("Library/Application Support/Code/User/settings.json");
    fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    fs::write(settings, r#"{"42header.email":"second@student.42.fr"}"#).expect("settings");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()));

    let resolution = resolver.resolve(Some("second"), None, temporary.path());

    assert_eq!(
        resolution.identity.expect("matched identity").email,
        "second@student.42.fr"
    );
}

#[test]
fn injected_home_and_lossy_editor_read_are_supported() {
    let temporary = TempDir::new().expect("temporary directory");
    let mut vimrc = b"let g:mail42 = 'lossy@student.42.fr'\n".to_vec();
    vimrc.extend_from_slice(&[0xff, b'\n']);
    fs::write(temporary.path().join(".vimrc"), vimrc).expect("vimrc");
    let resolver = IdentityResolver::isolated(None)
        .with_environment("HOME", temporary.path().to_string_lossy());

    let resolution = resolver.resolve(None, None, temporary.path());

    assert_eq!(
        resolution.identity.expect("editor identity").email,
        "lossy@student.42.fr"
    );
}

#[test]
fn mail_environment_precedes_saved_editor_settings() {
    let temporary = TempDir::new().expect("temporary directory");
    fs::write(
        temporary.path().join(".vimrc"),
        "let g:mail42 = 'editor@student.42.fr'\n",
    )
    .expect("vimrc");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("MAIL", "mail@student.42.fr");

    let resolution = resolver.resolve(None, None, temporary.path());

    let identity = resolution.identity.expect("MAIL identity");
    assert_eq!(identity.email, "mail@student.42.fr");
    assert_eq!(identity.source, "MAIL environment variable");
}

#[cfg(unix)]
#[test]
fn git_lookup_uses_the_captured_absolute_path_and_keeps_precedence() {
    let temporary = TempDir::new().expect("temporary directory");
    let bin = temporary.path().join("bin");
    fs::create_dir(&bin).expect("bin directory");
    let git = bin.join("git");
    fs::write(
        &git,
        "#!/bin/sh\nprintf '%s\\n' 'git-source@student.42.fr'\n",
    )
    .expect("fake git");
    fs::set_permissions(&git, fs::Permissions::from_mode(0o700)).expect("git permissions");
    let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
        .with_environment("PATH", bin.to_string_lossy())
        .with_environment("MAIL", "mail@student.42.fr")
        .with_git_lookup(true);

    let resolution = resolver.resolve(None, None, temporary.path());

    let identity = resolution.identity.expect("Git identity");
    assert_eq!(identity.email, "git-source@student.42.fr");
    assert_eq!(identity.source, "Git config");
    assert!(identity.inferred());
}
