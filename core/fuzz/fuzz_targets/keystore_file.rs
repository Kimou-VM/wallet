#![no_main]

use std::fs;
use std::sync::LazyLock;

use gem_keystore::{FileKeystore, Keystore};
use libfuzzer_sys::fuzz_target;
use tempfile::TempDir;

const KEYSTORE_ID: &str = "4b8fc216-dc74-481e-8011-bd4d1bc968ab";
const LEGACY_ID: &str = "fuzz-legacy-id";

static KEYSTORE: LazyLock<(TempDir, FileKeystore)> = LazyLock::new(|| {
    let dir = TempDir::new().unwrap();
    let keystore = FileKeystore::open(dir.path().to_path_buf()).unwrap();
    (dir, keystore)
});

fuzz_target!(|data: &[u8]| {
    let (dir, keystore) = &*KEYSTORE;
    fs::write(dir.path().join(format!("{KEYSTORE_ID}.json")), data).unwrap();
    let _ = keystore.get_meta(KEYSTORE_ID);
    let _ = keystore.list();
    let _ = keystore.delete_v3(LEGACY_ID);
});
