//! Mode A registry client for `https://repo.wawona.io/wasm/v1`.
//!
//! Hard firewall: refuse jailbreak APT paths and non-Wasm URLs.

use std::cmp::Ordering;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::store::{InstalledPackage, PackageStore};
use crate::{DEFAULT_REGISTRY, REGISTRY_ENV};

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryIndex {
    pub schema: Option<u32>,
    pub packages: Vec<RegistryPackage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryPackage {
    pub name: String,
    pub version: String,
    pub digest: String,
    /// Relative to registry base, e.g. `packages/hello/0.1.0/component.wasm`
    pub url: String,
    pub wasi: Option<String>,
    pub summary: Option<String>,
}

pub struct RegistryClient {
    base: String,
    agent: ureq::Agent,
}

impl RegistryClient {
    pub fn from_env_or_default() -> Result<Self> {
        Self::with_timeout(&registry_base_from_env(), Duration::from_secs(120))
    }

    /// Short deadline for PTY login notices. Offline stays quiet.
    pub fn from_env_for_notify() -> Result<Self> {
        Self::with_timeout(&registry_base_from_env(), Duration::from_secs(4))
    }

    pub fn new(base: &str) -> Result<Self> {
        Self::with_timeout(base, Duration::from_secs(120))
    }

    fn with_timeout(base: &str, timeout: Duration) -> Result<Self> {
        assert_wasm_registry_url(base)?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            agent: ureq::AgentBuilder::new().timeout(timeout).build(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn fetch_index(&self) -> Result<RegistryIndex> {
        let url = format!("{}/index.json", self.base);
        assert_wasm_registry_url(&url)?;
        let body = self
            .agent
            .get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_string()
            .context("read index body")?;
        let index: RegistryIndex = serde_json::from_str(&body).context("parse index.json")?;
        Ok(index)
    }

    pub fn search(&self, query: &str) -> Result<Vec<RegistryPackage>> {
        let index = self.fetch_index()?;
        let q = query.to_ascii_lowercase();
        Ok(index
            .packages
            .into_iter()
            .filter(|p| {
                q.is_empty()
                    || p.name.to_ascii_lowercase().contains(&q)
                    || p.summary
                        .as_ref()
                        .map(|s| s.to_ascii_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .collect())
    }

    pub fn install(&self, store: &PackageStore, name: &str, version: Option<&str>) -> Result<()> {
        let index = self.fetch_index()?;
        let candidates: Vec<_> = index.packages.into_iter().filter(|p| p.name == name).collect();
        let pkg = pick_package(candidates, name, version)?;
        let url = if pkg.url.starts_with("https://") || pkg.url.starts_with("http://") {
            pkg.url.clone()
        } else {
            format!("{}/{}", self.base, pkg.url.trim_start_matches('/'))
        };
        assert_wasm_registry_url(&url)?;
        if !url.contains(".wasm") && !url.ends_with("component.wasm") {
            // Allow digest URLs under /wasm/ packages path.
            if !url.contains("/wasm/") {
                bail!("refusing non-Wasm download URL: {url}");
            }
        }

        let mut reader = self
            .agent
            .get(&url)
            .call()
            .with_context(|| format!("GET {url}"))?
            .into_reader();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut bytes).context("download wasm")?;
        store.install_bytes(&pkg.name, &pkg.version, &bytes, &pkg.digest, "registry")?;
        Ok(())
    }
}

/// Reject jailbreak APT and non-Wasm registry bases.
pub fn assert_wasm_registry_url(url: &str) -> Result<()> {
    let lower = url.to_ascii_lowercase();
    if lower.contains("/jailbreak")
        || lower.contains(".deb")
        || lower.contains("/dists/")
        || lower.contains("packages.gz")
        || lower.contains("sileo")
    {
        bail!("refusing jailbreak/APT URL (Mode A Wasm registry only): {url}");
    }
    if lower.starts_with("https://") || lower.starts_with("http://") {
        if !(lower.contains("/wasm") || lower.contains("localhost") || lower.contains("127.0.0.1"))
        {
            // Allow only paths that look like the Wasm channel, or local test servers.
            if !lower.contains("repo.wawona.io") {
                bail!("registry URL must be repo.wawona.io/wasm or explicit /wasm/ path: {url}");
            }
            if !lower.contains("/wasm") {
                bail!("repo.wawona.io URL must include /wasm/: {url}");
            }
        }
        return Ok(());
    }
    // Relative paths ok when composing.
    if url.starts_with('/') || !url.contains("://") {
        return Ok(());
    }
    bail!("unsupported registry URL: {url}");
}

fn registry_base_from_env() -> String {
    std::env::var(REGISTRY_ENV)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_REGISTRY.to_string())
}

/// Numeric dotted versions. `1.0` and `1.0.0` compare equal. A newer component wins.
pub fn cmp_versions(left: &str, right: &str) -> Ordering {
    let parse = |part: &str| -> u64 {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0)
    };
    let mut a = left.split('.');
    let mut b = right.split('.');
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (Some(x), None) => {
                if parse(x) == 0 && a.all(|p| parse(p) == 0) {
                    return Ordering::Equal;
                }
                return Ordering::Greater;
            }
            (None, Some(y)) => {
                if parse(y) == 0 && b.all(|p| parse(p) == 0) {
                    return Ordering::Equal;
                }
                return Ordering::Less;
            }
            (Some(x), Some(y)) => {
                let ox = parse(x).cmp(&parse(y));
                if ox != Ordering::Equal {
                    return ox;
                }
            }
        }
    }
}

/// One installed package the registry can replace with a newer build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableUpgrade {
    pub name: String,
    pub from_version: String,
    pub to_version: String,
    pub to_digest: String,
    /// Sideload (`source != registry`). `wpm upgrade` skips these unless named.
    pub local_sideload: bool,
}

pub fn pick_package(
    mut candidates: Vec<RegistryPackage>,
    name: &str,
    version: Option<&str>,
) -> Result<RegistryPackage> {
    if candidates.is_empty() {
        bail!("package not found in registry: {name}");
    }
    if let Some(v) = version {
        candidates.retain(|p| p.version == v);
        if candidates.is_empty() {
            bail!("version {v} not found for {name}");
        }
        return Ok(candidates.pop().unwrap());
    }
    candidates.sort_by(|a, b| cmp_versions(&a.version, &b.version));
    Ok(candidates.pop().unwrap())
}

fn digests_match(installed: &str, registry: &str) -> bool {
    let norm = |d: &str| d.trim().trim_start_matches("sha256:").to_ascii_lowercase();
    norm(installed) == norm(registry)
}

/// Installed packages whose registry build is newer, or the same version with a new digest.
pub fn upgrades_for(installed: &[InstalledPackage], index: &[RegistryPackage]) -> Vec<AvailableUpgrade> {
    let mut out = Vec::new();
    for pkg in installed {
        let candidates: Vec<_> = index.iter().filter(|p| p.name == pkg.name).cloned().collect();
        let Some(latest) = pick_package(candidates, &pkg.name, None).ok() else {
            continue;
        };
        let newer = cmp_versions(&latest.version, &pkg.version) == Ordering::Greater;
        let republish =
            cmp_versions(&latest.version, &pkg.version) == Ordering::Equal
                && !digests_match(&pkg.digest, &latest.digest);
        if !newer && !republish {
            continue;
        }
        out.push(AvailableUpgrade {
            name: pkg.name.clone(),
            from_version: pkg.version.clone(),
            to_version: latest.version,
            to_digest: latest.digest,
            local_sideload: pkg.source != "registry",
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firewall_rejects_jailbreak() {
        assert!(assert_wasm_registry_url("https://repo.wawona.io/jailbreak/").is_err());
        assert!(assert_wasm_registry_url("https://repo.wawona.io/wasm/v1").is_ok());
    }

    fn pkg(name: &str, version: &str, digest: &str) -> RegistryPackage {
        RegistryPackage {
            name: name.into(),
            version: version.into(),
            digest: digest.into(),
            url: format!("packages/{name}/{version}/component.wasm"),
            wasi: Some("p1".into()),
            summary: None,
        }
    }

    fn installed(name: &str, version: &str, digest: &str, source: &str) -> InstalledPackage {
        InstalledPackage {
            name: name.into(),
            version: version.into(),
            digest: digest.into(),
            wasi: "p1".into(),
            source: source.into(),
            installed_at: 0,
        }
    }

    #[test]
    fn version_order_is_numeric() {
        assert_eq!(cmp_versions("0.1.9", "0.1.10"), Ordering::Less);
        assert_eq!(cmp_versions("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(cmp_versions("0.2.0", "0.1.9"), Ordering::Greater);
    }

    #[test]
    fn pick_package_uses_highest_version() {
        let rows = vec![pkg("chess", "0.1.0", "sha256:aa"), pkg("chess", "0.1.10", "sha256:bb")];
        let got = pick_package(rows, "chess", None).unwrap();
        assert_eq!(got.version, "0.1.10");
    }

    #[test]
    fn upgrades_skip_current_and_mark_sideload() {
        let index = vec![
            pkg("hello", "0.1.1", "sha256:new"),
            pkg("local-tool", "0.2.0", "sha256:reg"),
            pkg("current", "1.0.0", "sha256:same"),
        ];
        let installed = vec![
            installed("hello", "0.1.0", "sha256:old", "registry"),
            installed("local-tool", "0.1.0", "sha256:side", "local"),
            installed("current", "1.0.0", "sha256:same", "registry"),
            installed("only-local", "0.0.1", "sha256:xx", "local"),
        ];
        let ups = upgrades_for(&installed, &index);
        assert_eq!(ups.len(), 2);
        assert_eq!(ups[0].name, "hello");
        assert!(!ups[0].local_sideload);
        assert_eq!(ups[0].to_version, "0.1.1");
        assert!(ups[1].local_sideload);
    }
}
