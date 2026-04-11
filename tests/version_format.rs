//! Validate the version string injected by build.rs.

use thurkube::VERSION;

#[test]
fn version_does_not_start_with_v() {
    assert!(
        !VERSION.starts_with('v'),
        "VERSION must not start with 'v' (the prefix is added at \
         display time): got {VERSION}"
    );
}

#[test]
fn version_starts_with_digit() {
    assert!(
        VERSION.starts_with(|c: char| c.is_ascii_digit()),
        "VERSION must start with a digit: got {VERSION}"
    );
}

#[test]
fn dev_build_matches_cargo_version() {
    if cfg!(dev_build) {
        assert_eq!(
            VERSION,
            env!("CARGO_PKG_VERSION"),
            "dev builds must use Cargo.toml version"
        );
    }
}
