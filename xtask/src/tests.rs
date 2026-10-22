use super::*;
use ed25519_dalek::{Signer, SigningKey};
use p256::pkcs8::EncodePublicKey;
use serde_json::json;

pub(crate) fn fixture(tag: &str) -> (tempfile::TempDir, UpdateKeys) {
    let directory = tempfile::tempdir().unwrap();
    let value = version(tag).unwrap().to_string();
    let signing = SigningKey::from_bytes(&[17; 32]);
    let filename = format!("ChipDock-{value}-macOS-universal.zip");
    let archive = b"signed archive fixture";
    fs::write(directory.path().join(&filename), archive).unwrap();
    let signature = STANDARD.encode(signing.sign(archive).to_bytes());
    fs::write(directory.path().join("appcast.xml"), format!(r#"<?xml version="1.0"?><rss xmlns:sparkle="{SPARKLE}"><channel><item><sparkle:version>42</sparkle:version><sparkle:shortVersionString>{value}</sparkle:shortVersionString><description><![CDATA[<p>Update &amp; notes</p>]]></description><enclosure url="https://github.com/{REPOSITORY}/releases/download/{tag}/{filename}" length="{}" sparkle:edSignature="{signature}" /></item></channel></rss>"#, archive.len())).unwrap();
    let windows = p256::ecdsa::SigningKey::from_bytes((&[23; 32]).into()).unwrap();
    for arch in ["x64", "arm64"] {
        let filename = format!("ChipDock-{value}-win-{arch}-full.nupkg");
        let path = directory.path().join(&filename);
        fs::write(&path, b"package").unwrap();
        let feed = serde_json::to_vec(&json!({"Assets":[{"PackageId":"ChipDock","Version":value,
            "Type":"Full","FileName":filename,"Size":7,"SHA256":hash(&path).unwrap()}]}))
        .unwrap();
        let signature: p256::ecdsa::Signature =
            p256::ecdsa::signature::Signer::sign(&windows, &feed);
        let envelope = json!({"feed":STANDARD.encode(&feed),"signature":STANDARD.encode(signature.to_bytes())});
        fs::write(
            directory
                .path()
                .join(format!("releases.win-{arch}.signed.json")),
            serde_json::to_vec(&envelope).unwrap(),
        )
        .unwrap();
        for ext in ["exe", "msi"] {
            fs::write(
                directory
                    .path()
                    .join(format!("ChipDock-{value}-windows-{arch}.{ext}")),
                b"installer",
            )
            .unwrap();
        }
    }
    (
        directory,
        UpdateKeys {
            sparkle: STANDARD.encode(signing.verifying_key().as_bytes()),
            windows: STANDARD.encode(
                windows
                    .verifying_key()
                    .to_public_key_der()
                    .unwrap()
                    .as_bytes(),
            ),
        },
    )
}

#[test]
fn verified_release_preserves_exact_notes_and_signature_when_rewriting_transport() {
    let (directory, keys) = fixture("v0.2.0");
    let original = fs::read_to_string(directory.path().join("appcast.xml")).unwrap();
    let plan = prepare(directory.path(), "v0.2.0", Platform::All, &keys).unwrap();
    assert_eq!(plan.assets.len(), 10);
    let mirrored = fs::read_to_string(&plan.feeds["appcast.xml"]).unwrap();
    assert_eq!(mirrored, original.replace(
        &format!("https://github.com/{REPOSITORY}/releases/download/v0.2.0/ChipDock-0.2.0-macOS-universal.zip"),
        &format!("{}/releases/v0.2.0/ChipDock-0.2.0-macOS-universal.zip", cos::PUBLIC_ROOT)));
}

#[test]
fn tampered_archive_is_rejected() {
    let (directory, keys) = fixture("v0.2.0");
    fs::write(
        directory.path().join("ChipDock-0.2.0-macOS-universal.zip"),
        b"tampered archive",
    )
    .unwrap();
    assert!(prepare(directory.path(), "v0.2.0", Platform::All, &keys).is_err());
}

#[test]
fn wrong_sparkle_public_key_is_rejected() {
    let (directory, mut keys) = fixture("v0.2.0");
    keys.sparkle = STANDARD.encode(SigningKey::from_bytes(&[18; 32]).verifying_key().as_bytes());
    assert!(prepare(directory.path(), "v0.2.0", Platform::Macos, &keys).is_err());
}

#[test]
fn tampered_signed_windows_feed_is_rejected() {
    let (directory, keys) = fixture("v0.2.0");
    let path = directory.path().join("releases.win-x64.signed.json");
    let mut envelope: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut bytes = STANDARD.decode(envelope["feed"].as_str().unwrap()).unwrap();
    bytes[0] ^= 1;
    envelope["feed"] = STANDARD.encode(bytes).into();
    fs::write(path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert!(prepare(directory.path(), "v0.2.0", Platform::Windows, &keys).is_err());
}

#[test]
fn signed_windows_metadata_cannot_cross_architectures() {
    let (directory, keys) = fixture("v0.2.0");
    fs::copy(
        directory.path().join("releases.win-x64.signed.json"),
        directory.path().join("releases.win-arm64.signed.json"),
    )
    .unwrap();
    assert!(prepare(directory.path(), "v0.2.0", Platform::Windows, &keys).is_err());
}

#[test]
fn signed_windows_feed_rejects_retired_package_identity() {
    let (_directory, keys) = fixture("v0.2.0");
    let signing = p256::ecdsa::SigningKey::from_bytes((&[23; 32]).into()).unwrap();
    let feed = serde_json::to_vec(&json!({"Assets":[{"PackageId":"FlashDesk","Version":"0.2.0",
        "Type":"Full","FileName":"FlashDesk-0.2.0-win-x64-full.nupkg","Size":7,"SHA256":"ab".repeat(32)}]}))
    .unwrap();
    let signature: p256::ecdsa::Signature = p256::ecdsa::signature::Signer::sign(&signing, &feed);
    let envelope = serde_json::to_vec(&json!({"feed":STANDARD.encode(&feed),
        "signature":STANDARD.encode(signature.to_bytes())}))
    .unwrap();
    assert_eq!(
        windows_identity(&envelope, "x64", &keys.windows)
            .unwrap_err()
            .to_string(),
        "Unexpected Windows app, architecture or package"
    );
}

#[test]
fn altered_windows_package_is_rejected() {
    let (directory, keys) = fixture("v0.2.0");
    fs::write(
        directory.path().join("ChipDock-0.2.0-win-x64-full.nupkg"),
        b"changed",
    )
    .unwrap();
    assert!(prepare(directory.path(), "v0.2.0", Platform::Windows, &keys).is_err());
}

#[test]
fn dtd_and_duplicate_update_items_are_rejected() {
    assert!(
        parse_xml(r#"<!DOCTYPE rss [<!ENTITY secret SYSTEM "file:///etc/passwd">]><rss/>"#)
            .is_err()
    );
    assert!(appcast_identity(b"<rss><channel><item/><item/></channel></rss>").is_err());
}

#[test]
fn noncanonical_and_preview_release_tags_are_rejected() {
    for tag in [
        "0.2.0",
        "v0.2",
        "v01.2.0",
        "v0.2.0-beta.1",
        "v0.2.0+meta",
        "v../0.2.0",
    ] {
        assert!(version(tag).is_err(), "{tag}");
    }
}

#[test]
fn release_uses_the_same_embedded_keys_as_native_clients() {
    let keys = UpdateKeys::from_project().unwrap();
    assert_eq!(STANDARD.decode(keys.sparkle).unwrap().len(), 32);
    assert!(
        p256::ecdsa::VerifyingKey::from_public_key_der(&STANDARD.decode(keys.windows).unwrap())
            .is_ok()
    );
}
