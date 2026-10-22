//! COS is the primary mirror; GitHub retains the same signed payloads as a fallback.
use crate::{Channels, Plan, Result, UpdateKeys, ensure, hash, identity};
use hmac::{Hmac, KeyInit, Mac};
use reqwest::{Method, StatusCode, blocking::Client};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Read,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const PUBLIC_ROOT: &str =
    "https://app-release-1302963684.cos.ap-guangzhou.myqcloud.com/chipdock";
const UPLOAD_HOST: &str = "app-release-1302963684.cos.accelerate.myqcloud.com";
const CATALOG: &str = "update-feed/state.json";
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";
const FEED_CACHE: &str = "no-cache, max-age=60";

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hmac(key: &[u8], value: &str) -> Result<String> {
    let mut mac = Hmac::<Sha1>::new_from_slice(key)?;
    mac.update(value.as_bytes());
    Ok(hex(&mac.finalize().into_bytes()))
}

// COS v5 requires the hexadecimal SignKey as the second HMAC key, not raw bytes.
fn authorization(
    secret_id: &str,
    secret_key: &str,
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
    now: u64,
) -> Result<String> {
    let time = format!("{};{}", now.saturating_sub(60), now + 3600);
    let names = headers.keys().cloned().collect::<Vec<_>>().join(";");
    let values = headers
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let request = format!("{}\n{path}\n\n{values}\n", method.to_ascii_lowercase());
    let request_hash = hex(&Sha1::digest(request.as_bytes()));
    let string_to_sign = format!("sha1\n{time}\n{request_hash}\n");
    let sign_key = hmac(secret_key.as_bytes(), &time)?;
    let signature = hmac(sign_key.as_bytes(), &string_to_sign)?;
    Ok(format!(
        "q-sign-algorithm=sha1&q-ak={secret_id}&q-sign-time={time}&q-key-time={time}&q-header-list={names}&q-url-param-list=&q-signature={signature}"
    ))
}

trait Store {
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>>;
    fn matches(&self, key: &str, path: &Path) -> Result<bool>;
    fn upload(&self, key: &str, path: &Path, immutable: bool) -> Result<()>;
}

struct Cos {
    client: Client,
    secret_id: String,
    secret_key: String,
}

impl Cos {
    fn new() -> Result<Self> {
        let secret_id = env::var("COS_SECRET_ID")?;
        let secret_key = env::var("COS_SECRET_KEY")?;
        ensure(
            !secret_id.is_empty() && !secret_key.is_empty(),
            "Missing COS credentials",
        )?;
        Ok(Self {
            client: Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(20))
                .timeout(Duration::from_secs(1800))
                .build()?,
            secret_id,
            secret_key,
        })
    }

    fn request(
        &self,
        method: Method,
        key: &str,
        file: Option<(&Path, bool)>,
    ) -> Result<reqwest::blocking::Response> {
        let path = format!("/chipdock/{key}");
        let encoded = path.split('/').map(encode).collect::<Vec<_>>().join("/");
        for attempt in 0..4 {
            let mut headers = BTreeMap::from([("host".into(), UPLOAD_HOST.into())]);
            if let Some((file, immutable)) = file {
                headers.insert(
                    "content-length".into(),
                    fs::metadata(file)?.len().to_string(),
                );
                headers.insert("content-type".into(), content_type(key).into());
                headers.insert(
                    "cache-control".into(),
                    if immutable && key != CATALOG {
                        IMMUTABLE_CACHE
                    } else {
                        FEED_CACHE
                    }
                    .into(),
                );
                headers.insert("x-cos-acl".into(), "public-read".into());
                if immutable {
                    headers.insert("x-cos-forbid-overwrite".into(), "true".into());
                }
            }
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let signature = authorization(
                &self.secret_id,
                &self.secret_key,
                method.as_str(),
                &path,
                &headers,
                now,
            )?;
            let mut request = self
                .client
                .request(method.clone(), format!("https://{UPLOAD_HOST}{encoded}"));
            for (key, value) in headers {
                request = request.header(key, value);
            }
            request = request.header("authorization", signature);
            if let Some((file, _)) = file {
                request = request.body(fs::File::open(file)?);
            }
            match request.send() {
                Ok(response)
                    if attempt < 3
                        && (response.status().is_server_error()
                            || response.status() == StatusCode::TOO_MANY_REQUESTS
                            || response.status() == StatusCode::REQUEST_TIMEOUT) => {}
                Ok(response) => return Ok(response),
                Err(error) if attempt == 3 => return Err(error.into()),
                Err(_) => {}
            }
            std::thread::sleep(Duration::from_secs(1 << attempt));
        }
        Err("COS request retry limit exceeded".into())
    }

    fn public_get(&self, key: &str) -> Result<reqwest::blocking::Response> {
        let encoded = key.split('/').map(encode).collect::<Vec<_>>().join("/");
        Ok(self
            .client
            .get(format!("{PUBLIC_ROOT}/{encoded}"))
            .header("cache-control", "no-cache")
            .send()?
            .error_for_status()?)
    }
}

impl Store for Cos {
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let response = self.request(Method::HEAD, key, None)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        response.error_for_status()?;
        Ok(Some(self.public_get(key)?.bytes()?.to_vec()))
    }

    fn matches(&self, key: &str, path: &Path) -> Result<bool> {
        let response = self.request(Method::HEAD, key, None)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        response.error_for_status()?;
        // Check the bytes through the anonymous endpoint used by clients, not just metadata.
        let mut response = self.public_get(key)?;
        let mut digest = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = response.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
            size += count as u64;
        }
        ensure(
            size == fs::metadata(path)?.len()
                && digest
                    .finalize()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    == hash(path)?,
            &format!("COS object differs: {key}; refusing overwrite"),
        )?;
        Ok(true)
    }

    fn upload(&self, key: &str, path: &Path, immutable: bool) -> Result<()> {
        let response = self.request(Method::PUT, key, Some((path, immutable)))?;
        // A previous attempt may have succeeded before its response was lost.
        if response.status() != StatusCode::CONFLICT || !immutable {
            response.error_for_status()?;
        }
        ensure(
            self.matches(key, path)?,
            &format!("COS object is not publicly available: {key}"),
        )?;
        println!("Published COS object: {key}");
        Ok(())
    }
}

fn content_type(key: &str) -> &'static str {
    if key.ends_with(".xml") {
        "application/xml; charset=utf-8"
    } else if key.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if key.ends_with(".md") || key.ends_with("SHA256SUMS") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

/// Publish verified packages before reserving and advancing client feeds.
pub fn publish(plan: &Plan, directory: &Path, keys: &UpdateKeys) -> Result<()> {
    publish_to(plan, directory, keys, &Cos::new()?)
}

/// Repair an explicitly replaced version using the same opt-in as BibCiTeX.
pub fn replace_release(plan: &Plan, directory: &Path, keys: &UpdateKeys) -> Result<()> {
    replace_release_to(plan, directory, keys, &Cos::new()?)
}

fn replace_release_to(
    plan: &Plan,
    directory: &Path,
    keys: &UpdateKeys,
    store: &impl Store,
) -> Result<()> {
    let previous = store
        .read(CATALOG)?
        .ok_or("Cannot replace without an existing COS catalog")?;
    let catalog: Channels = serde_json::from_slice(&previous)?;
    ensure(catalog.schema == 1, "Invalid COS publication state")?;
    for (name, path) in &plan.assets {
        store.upload(name, path, false)?;
    }
    publish_to_with_replacement(plan, directory, keys, store, true)
}

fn publish_to(plan: &Plan, directory: &Path, keys: &UpdateKeys, store: &impl Store) -> Result<()> {
    publish_to_with_replacement(plan, directory, keys, store, false)
}

fn publish_to_with_replacement(
    plan: &Plan,
    directory: &Path,
    keys: &UpdateKeys,
    store: &impl Store,
    replace: bool,
) -> Result<()> {
    let mut channels: Channels = match store.read(CATALOG)? {
        Some(bytes) => serde_json::from_slice(&bytes)?,
        None => Channels::default(),
    };
    ensure(channels.schema == 1, "Invalid COS publication state")?;
    let mut promoted = Vec::new();
    for (name, candidate) in &plan.identities {
        let reserved = channels
            .feeds
            .get(name)
            .is_some_and(|previous| previous == candidate);
        let mut predecessors = Vec::new();
        if let Some(previous) = channels.feeds.get(name) {
            predecessors.push(previous.clone());
        }
        if let Some(previous) = store.read(&format!("update-feed/{name}"))? {
            match identity(name, &previous, keys) {
                Ok(previous) => predecessors.push(previous),
                // Like BibCiTeX's catalog, an existing reservation remains authoritative
                // when a client-facing pointer is corrupt and needs regeneration.
                Err(error) if channels.feeds.contains_key(name) => {
                    eprintln!(
                        "Repairing invalid COS feed {name} from its version reservation: {error}"
                    );
                }
                Err(error) => return Err(error),
            }
        }
        let mut newer = false;
        for previous in predecessors {
            let order = candidate.order()?.cmp(&previous.order()?);
            if order.is_lt() {
                newer = true;
            }
            if order.is_eq() {
                ensure(
                    replace || reserved || candidate == &previous,
                    "Refusing to replace an already published update",
                )?;
            }
        }
        if !newer {
            promoted.push(name);
        }
    }
    // BibCiTeX's preflight checks all immutable objects before any network writes.
    let mut missing = Vec::new();
    for (name, path) in &plan.assets {
        if !store.matches(name, path)? {
            missing.push((name, path));
        }
    }
    for (name, path) in missing {
        store.upload(name, path, true)?;
    }
    if promoted.is_empty() {
        println!("Mirrored assets; newer COS feeds remain current");
        return Ok(());
    }
    for name in &promoted {
        channels
            .feeds
            .insert((*name).clone(), plan.identities[*name].clone());
    }
    let catalog = directory.join("state.json");
    fs::write(&catalog, serde_json::to_vec_pretty(&channels)?)?;
    // Reserve all target versions first: older repairs cannot downgrade a partial publication.
    store.upload(CATALOG, &catalog, false)?;
    for name in promoted {
        store.upload(&format!("update-feed/{name}"), &plan.feeds[name], false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
