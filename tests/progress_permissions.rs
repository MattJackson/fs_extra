#![cfg(unix)]

use fs_extra::file::{self, CopyOptions};
use std::cell::Cell;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "fs-extra-progress-permissions-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create test directory: {}", error),
            }
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn check_permissions_before_progress(existing_destination: bool) {
    let directory = Directory::new();
    let source = directory.0.join("source");
    let destination = directory.0.join("destination");
    let contents = b"private contents";
    std::fs::write(&source, contents).unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600)).unwrap();
    if existing_destination {
        std::fs::write(&destination, b"old public contents").unwrap();
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let callbacks = Cell::new(0);
    let options = CopyOptions::new().buffer_size(1).overwrite(true);
    let copied = file::copy_with_progress(&source, &destination, &options, |_| {
        callbacks.set(callbacks.get() + 1);
        assert_eq!(
            mode(&destination),
            0o600,
            "source permissions must apply before any copied byte is reported"
        );
    })
    .unwrap();
    assert_eq!(copied, contents.len() as u64);
    assert_eq!(std::fs::read(&destination).unwrap(), contents);
    assert_eq!(mode(&destination), 0o600);
    assert!(callbacks.get() > 0);
}

#[test]
fn new_progress_destination_has_source_permissions_before_first_callback() {
    check_permissions_before_progress(false);
}

#[test]
fn overwritten_progress_destination_has_source_permissions_before_first_callback() {
    check_permissions_before_progress(true);
}

#[test]
fn existing_destination_mode_is_replaced_by_source_mode() {
    let directory = Directory::new();
    let source = directory.0.join("source");
    let destination = directory.0.join("destination");
    std::fs::write(&source, b"data").unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::write(&destination, b"old").unwrap();
    std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o600)).unwrap();
    let options = CopyOptions::new().overwrite(true);
    file::copy_with_progress(&source, &destination, &options, |_| {}).unwrap();
    assert_eq!(mode(&destination), 0o644);
}

#[test]
fn non_regular_destination_is_not_chmoded() {
    let directory = Directory::new();
    let source = directory.0.join("source");
    std::fs::write(&source, b"data").unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600)).unwrap();
    let before = mode(std::path::Path::new("/dev/null"));
    let options = CopyOptions::new().overwrite(true);
    file::copy_with_progress(&source, "/dev/null", &options, |_| {}).unwrap();
    assert_eq!(mode(std::path::Path::new("/dev/null")), before);
}
