use super::*;
use crate::{Platform, prepare, tests::fixture};
use std::cell::RefCell;

#[derive(Default)]
struct MemoryStore {
    objects: RefCell<BTreeMap<String, Vec<u8>>>,
    uploads: RefCell<Vec<String>>,
    fail: RefCell<Option<String>>,
}
impl Store for MemoryStore {
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.objects.borrow().get(key).cloned())
    }
    fn matches(&self, key: &str, path: &Path) -> Result<bool> {
        match self.objects.borrow().get(key) {
            None => Ok(false),
            Some(bytes) => {
                ensure(bytes == &fs::read(path)?, "Existing object differs")?;
                Ok(true)
            }
        }
    }
    fn upload(&self, key: &str, path: &Path, immutable: bool) -> Result<()> {
        ensure(
            self.fail.borrow().as_deref() != Some(key),
            "Injected upload failure",
        )?;
        ensure(
            !immutable || !self.objects.borrow().contains_key(key),
            "Immutable overwrite",
        )?;
        self.objects
            .borrow_mut()
            .insert(key.into(), fs::read(path)?);
        self.uploads.borrow_mut().push(key.into());
        Ok(())
    }
}

#[test]
fn payloads_are_verified_before_catalog_and_feed_publication() {
    let (directory, keys) = fixture("v0.2.0");
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    publish_to(&plan, directory.path(), &keys, &store).unwrap();
    let uploads = store.uploads.borrow();
    let catalog = uploads.iter().position(|key| key == CATALOG).unwrap();
    assert_eq!(catalog, plan.assets.len());
    assert!(
        uploads[..catalog]
            .iter()
            .all(|key| key.starts_with("releases/"))
    );
    assert!(
        uploads[catalog + 1..]
            .iter()
            .all(|key| key.starts_with("update-feed/"))
    );
}

#[test]
fn conflicting_immutable_bytes_block_all_network_writes() {
    let (directory, keys) = fixture("v0.2.0");
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    store.objects.borrow_mut().insert(
        plan.assets.keys().last().unwrap().clone(),
        b"conflicting".to_vec(),
    );
    assert!(publish_to(&plan, directory.path(), &keys, &store).is_err());
    assert!(store.uploads.borrow().is_empty());
}

#[test]
fn failed_version_reservation_never_advances_client_feeds() {
    let (directory, keys) = fixture("v0.2.0");
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    *store.fail.borrow_mut() = Some(CATALOG.into());
    assert!(publish_to(&plan, directory.path(), &keys, &store).is_err());
    assert!(
        store
            .objects
            .borrow()
            .keys()
            .all(|key| key.starts_with("releases/"))
    );
}

#[test]
fn same_version_retry_completes_a_partial_publication() {
    let (directory, keys) = fixture("v0.2.0");
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    let last = "update-feed/releases.win-x64.signed.json";
    *store.fail.borrow_mut() = Some(last.into());
    assert!(publish_to(&plan, directory.path(), &keys, &store).is_err());
    let count = store
        .uploads
        .borrow()
        .iter()
        .filter(|key| key.starts_with("releases/"))
        .count();
    *store.fail.borrow_mut() = None;
    publish_to(&plan, directory.path(), &keys, &store).unwrap();
    assert!(store.objects.borrow().contains_key(last));
    assert_eq!(
        count,
        store
            .uploads
            .borrow()
            .iter()
            .filter(|key| key.starts_with("releases/"))
            .count()
    );
}

#[test]
fn older_repair_cannot_downgrade_a_partially_published_newer_release() {
    let store = MemoryStore::default();
    let (newer, keys) = fixture("v0.3.0");
    let plan = prepare(newer.path(), "v0.3.0", Platform::All, &keys).unwrap();
    let last = "update-feed/releases.win-x64.signed.json";
    *store.fail.borrow_mut() = Some(last.into());
    assert!(publish_to(&plan, newer.path(), &keys, &store).is_err());
    *store.fail.borrow_mut() = None;
    let (older, older_keys) = fixture("v0.2.0");
    let old = prepare(older.path(), "v0.2.0", Platform::All, &older_keys).unwrap();
    publish_to(&old, older.path(), &older_keys, &store).unwrap();
    assert!(!store.objects.borrow().contains_key(last));
    let catalog: Channels = serde_json::from_slice(&store.objects.borrow()[CATALOG]).unwrap();
    assert!(catalog.feeds.values().all(|entry| entry.version == "0.3.0"));
    publish_to(&plan, newer.path(), &keys, &store).unwrap();
    assert_eq!(
        identity(
            "releases.win-x64.signed.json",
            &store.objects.borrow()[last],
            &keys
        )
        .unwrap()
        .version,
        "0.3.0"
    );
}

#[test]
fn invalid_catalog_is_never_reset() {
    let (directory, keys) = fixture("v0.2.0");
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    store
        .objects
        .borrow_mut()
        .insert(CATALOG.into(), b"bad json".to_vec());
    assert!(publish_to(&plan, directory.path(), &keys, &store).is_err());
    assert!(store.uploads.borrow().is_empty());
}

#[test]
fn a_valid_reservation_can_repair_corrupt_client_feeds_without_replacing_packages() {
    let (directory, keys) = fixture("v0.2.0");
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    publish_to(&plan, directory.path(), &keys, &store).unwrap();
    for name in plan.feeds.keys() {
        store
            .objects
            .borrow_mut()
            .insert(format!("update-feed/{name}"), b"corrupt feed".to_vec());
    }
    store.uploads.borrow_mut().clear();
    publish_to(&plan, directory.path(), &keys, &store).unwrap();
    for (name, path) in &plan.feeds {
        assert_eq!(
            store.objects.borrow()[&format!("update-feed/{name}")],
            fs::read(path).unwrap()
        );
    }
    assert!(
        store
            .uploads
            .borrow()
            .iter()
            .all(|name| name.starts_with("update-feed/"))
    );
}

#[test]
fn corrupt_feeds_without_a_reservation_block_all_writes() {
    let (directory, keys) = fixture("v0.2.0");
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    store
        .objects
        .borrow_mut()
        .insert("update-feed/appcast.xml".into(), b"corrupt feed".to_vec());
    assert!(publish_to(&plan, directory.path(), &keys, &store).is_err());
    assert!(store.uploads.borrow().is_empty());
}

#[test]
fn repairing_a_corrupt_feed_cannot_bypass_a_higher_version_reservation() {
    let (newer, keys) = fixture("v0.3.0");
    let plan = prepare(newer.path(), "v0.3.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    publish_to(&plan, newer.path(), &keys, &store).unwrap();
    let name = "update-feed/releases.win-x64.signed.json";
    store
        .objects
        .borrow_mut()
        .insert(name.into(), b"corrupt newer feed".to_vec());
    store.uploads.borrow_mut().clear();
    let (older, keys) = fixture("v0.2.0");
    let plan = prepare(older.path(), "v0.2.0", Platform::All, &keys).unwrap();
    publish_to(&plan, older.path(), &keys, &store).unwrap();
    assert_eq!(store.objects.borrow()[name], b"corrupt newer feed");
    assert!(
        store
            .uploads
            .borrow()
            .iter()
            .all(|name| name.starts_with("releases/"))
    );
}

#[test]
fn explicit_replacement_can_resume_after_reserving_changed_same_version_metadata() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use ed25519_dalek::{Signer, SigningKey};
    let (directory, keys) = fixture("v0.2.0");
    let original = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    let store = MemoryStore::default();
    publish_to(&original, directory.path(), &keys, &store).unwrap();
    store
        .objects
        .borrow_mut()
        .insert("releases/v0.1.0/keep.zip".into(), b"keep".to_vec());
    let archive = directory.path().join("ChipDock-0.2.0-macOS-universal.zip");
    let old_size = fs::metadata(&archive).unwrap().len();
    let payload = b"explicitly rebuilt signed archive";
    fs::write(&archive, payload).unwrap();
    let signature = STANDARD.encode(SigningKey::from_bytes(&[17; 32]).sign(payload).to_bytes());
    let old_signature = original.identities["appcast.xml"]
        .signature
        .as_ref()
        .unwrap();
    let xml = fs::read_to_string(directory.path().join("appcast.xml"))
        .unwrap()
        .replace(old_signature, &signature)
        .replace(
            &format!("length=\"{old_size}\""),
            &format!("length=\"{}\"", payload.len()),
        );
    fs::write(directory.path().join("appcast.xml"), xml).unwrap();
    let replacement = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    assert!(publish_to(&replacement, directory.path(), &keys, &store).is_err());
    *store.fail.borrow_mut() = Some("update-feed/appcast.xml".into());
    assert!(replace_release_to(&replacement, directory.path(), &keys, &store).is_err());
    *store.fail.borrow_mut() = None;
    publish_to(&replacement, directory.path(), &keys, &store).unwrap();
    assert_eq!(
        store.objects.borrow()["update-feed/appcast.xml"],
        fs::read(&replacement.feeds["appcast.xml"]).unwrap()
    );
    assert_eq!(store.objects.borrow()["releases/v0.1.0/keep.zip"], b"keep");
}

#[test]
fn cos_signing_matches_bibcitexs_independent_request_vector() {
    let headers = BTreeMap::from([
        ("content-type".into(), "application/octet-stream".into()),
        (
            "host".into(),
            "example.cos.ap-guangzhou.myqcloud.com".into(),
        ),
        ("x-cos-acl".into(), "public-read".into()),
    ]);
    assert_eq!(
        authorization(
            "test-id",
            "test-secret",
            "PUT",
            "/bibcitex/test file.zip",
            &headers,
            1000
        )
        .unwrap(),
        "q-sign-algorithm=sha1&q-ak=test-id&q-sign-time=940;4600&q-key-time=940;4600&q-header-list=content-type;host;x-cos-acl&q-url-param-list=&q-signature=67b63dfbbe37d1b8d1190eaeef859731143d683d"
    );
}

#[test]
fn hmac_and_object_encoding_match_standard_vectors() {
    assert_eq!(
        hmac(&[0x0b; 20], "Hi There").unwrap(),
        "b617318655057264e28bc0b6fb378c8ef146be00"
    );
    assert_eq!(encode("a b+中"), "a%20b%2B%E4%B8%AD");
}
