use super::install::verify_asset;
use super::*;
use ed25519_dalek::{Signer, SigningKey};
#[test]
fn rejects_tampered_signed_metadata() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let bytes =
        br#"{"identity":"com.xiaoyu.chipdock.gpui","version":"1.0.0","notes":"","assets":{}}"#;
    let mut signed = SignedFeed {
        payload: STANDARD.encode(bytes),
        signature: STANDARD.encode(key.sign(bytes).to_bytes()),
    };
    assert!(
        verify(
            &serde_json::to_vec(&signed).unwrap(),
            &STANDARD.encode(key.verifying_key().as_bytes())
        )
        .is_ok()
    );
    signed.payload = STANDARD.encode(b"tampered");
    assert!(
        verify(
            &serde_json::to_vec(&signed).unwrap(),
            &STANDARD.encode(key.verifying_key().as_bytes())
        )
        .is_err()
    );
}
#[test]
fn never_offers_downgrades_or_other_platform_assets() {
    let feed = Feed {
        identity: "com.xiaoyu.chipdock.gpui".into(),
        version: "1.0.0".into(),
        notes: String::new(),
        assets: BTreeMap::new(),
        packages: Vec::new(),
    };
    assert!(
        select(feed.clone(), "1.1.0", "linux-x86_64")
            .unwrap()
            .is_none()
    );
    assert!(select(feed, "0.1.0", "linux-x86_64").is_err());
}
#[test]
fn rejects_partial_binary() {
    let asset = Asset {
        name: "chipdock-gpui-linux-x86_64".into(),
        size: 2,
        sha256: digest(b"ok"),
    };
    assert!(verify_asset(b"o", &asset).is_err());
    assert!(verify_asset(b"no", &asset).is_err());
    assert!(verify_asset(b"ok", &asset).is_ok());
}

#[test]
fn signed_linux_packages_are_validated_without_becoming_portable_updates() {
    let key = SigningKey::from_bytes(&[11; 32]);
    let asset = Asset {
        name: "chipdock-gpui-linux-x86_64".into(),
        size: 2,
        sha256: digest(b"ok"),
    };
    let mut feed = Feed {
        identity: "com.xiaoyu.chipdock.gpui".into(),
        version: "1.0.0".into(),
        notes: String::new(),
        assets: BTreeMap::from([("linux-x86_64".into(), asset)]),
        packages: ["deb", "rpm", "tar.gz"]
            .map(|extension| Asset {
                name: format!("chipdock-gpui-1.0.0-linux-x86_64.{extension}"),
                size: 2,
                sha256: digest(b"ok"),
            })
            .into(),
    };
    let verify_feed = |feed: &Feed| {
        let payload = serde_json::to_vec(feed).unwrap();
        let signed = SignedFeed {
            payload: STANDARD.encode(&payload),
            signature: STANDARD.encode(key.sign(&payload).to_bytes()),
        };
        verify(
            &serde_json::to_vec(&signed).unwrap(),
            &STANDARD.encode(key.verifying_key().as_bytes()),
        )
    };
    let verified = verify_feed(&feed).unwrap();
    assert_eq!(verified.release_assets().count(), 4);
    assert_eq!(
        select(verified, "0.9.0", "linux-x86_64")
            .unwrap()
            .unwrap()
            .asset
            .name,
        "chipdock-gpui-linux-x86_64"
    );
    let valid_name = feed.packages[0].name.clone();
    for name in [
        "../chipdock-gpui-1.0.0-linux-x86_64.deb",
        "chipdock-gpui-2.0.0-linux-x86_64.deb",
        "chipdock-gpui-1.0.0-linux-aarch64.deb",
        "chipdock-gpui-1.0.0-linux-x86_64.exe",
    ] {
        feed.packages[0].name = name.into();
        assert!(verify_feed(&feed).is_err(), "{name}");
    }
    feed.packages[0].name = valid_name;
    feed.packages.push(feed.packages[0].clone());
    assert!(verify_feed(&feed).is_err());
}

#[cfg(not(windows))]
#[test]
fn failed_restart_restores_the_previous_executable() {
    let directory = tempfile::tempdir().unwrap();
    let current = directory.path().join("app");
    let pending = directory.path().join("pending");
    std::fs::write(&current, b"previous").unwrap();
    std::fs::write(&pending, b"new").unwrap();
    assert!(
        super::install::replace_portable(&current, &pending, |_| anyhow::bail!(
            "simulated launch failure"
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(current).unwrap(), b"previous");
}

#[test]
fn corrupt_mirror_payload_falls_back_to_the_same_signed_asset_and_cancel_stops_retry() {
    let update = Available {
        version: "1.0.0".into(),
        notes: String::new(),
        asset: Asset {
            name: "chipdock-gpui-linux-x86_64".into(),
            size: 2,
            sha256: digest(b"ok"),
        },
    };
    let cancel = AtomicBool::new(false);
    let mut calls = Vec::new();
    let bytes = super::download::download_verified(&update, &cancel, |fallback| {
        calls.push(fallback);
        Ok(if fallback {
            b"ok".to_vec()
        } else {
            b"no".to_vec()
        })
    })
    .unwrap();
    assert_eq!(bytes, b"ok");
    assert_eq!(calls, [false, true]);
    let mut calls = 0;
    assert!(
        super::download::download_verified(&update, &cancel, |_| {
            calls += 1;
            cancel.store(true, Ordering::Relaxed);
            anyhow::bail!("cancelled")
        })
        .is_err()
    );
    assert_eq!(calls, 1);
}
#[test]
fn valid_primary_without_update_ends_the_check() {
    let key = SigningKey::from_bytes(&[9; 32]);
    let payload = serde_json::to_vec(&Feed {
        identity: "com.xiaoyu.chipdock.gpui".into(),
        version: "1.0.0".into(),
        notes: String::new(),
        assets: BTreeMap::new(),
        packages: Vec::new(),
    })
    .unwrap();
    let signed = serde_json::to_vec(&SignedFeed {
        payload: STANDARD.encode(&payload),
        signature: STANDARD.encode(key.sign(&payload).to_bytes()),
    })
    .unwrap();
    let mut calls = 0;
    let result = super::download::resolve_feed(
        |fallback| {
            calls += 1;
            assert!(!fallback);
            Ok(signed.clone())
        },
        &STANDARD.encode(key.verifying_key().as_bytes()),
        "1.0.0",
        "linux-x86_64",
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(calls, 1);
}
#[test]
fn windows_update_accepts_the_released_portable_name_but_not_other_executables() {
    use super::install::valid_windows_target;
    assert!(valid_windows_target(
        Path::new("/app/chipdock-gpui-windows-x86_64.exe"),
        Path::new("/app/chipdock-gpui-windows-x86_64.updater.exe")
    ));
    assert!(!valid_windows_target(
        Path::new("/app/other.exe"),
        Path::new("/app/other.updater.exe")
    ));
}
