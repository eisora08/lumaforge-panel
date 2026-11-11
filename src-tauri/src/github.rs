use anyhow::{Context, Result as AnyResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

const GITHUB_API_BASE: &str = "https://api.github.com";
const CONNECT_TIMEOUT_SECS: u64 = 20;
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;
const RELEASE_CACHE_TTL_SECS: u64 = 86400;
const APP_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/eisora08/lumaforge-panel)"
);

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub zip_url: String,
    pub zip_name: String,
    /// "zip" | "dll" | "so" | "7z"
    pub archive_ext: String,
    /// When GitHub published the release (used to date an install by mtime).
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    /// `sha256:<hex>` of the selected asset, when GitHub exposes it.
    pub asset_digest: Option<String>,
    pub asset_size: Option<u64>,
}

struct CachedRelease {
    info: ReleaseInfo,
    cached_at: SystemTime,
}

fn github_cache() -> &'static Mutex<HashMap<String, CachedRelease>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedRelease>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drop every cached entry so the next lookup hits the GitHub API.
pub fn invalidate_cache() {
    if let Ok(mut cache) = github_cache().lock() {
        cache.clear();
    }
}

fn http_client() -> AnyResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(APP_USER_AGENT)
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .build()
        .context("Failed to create HTTP client")
}

struct SelectedAsset {
    name: String,
    url: String,
    ext: String,
    digest: Option<String>,
    size: Option<u64>,
}

fn select_asset(
    assets: &[serde_json::Value],
    preferred_asset: Option<&str>,
    preferred_asset_contains: Option<&str>,
) -> AnyResult<SelectedAsset> {
    let contains_lower = preferred_asset_contains.map(|s| s.to_lowercase());
    let os_is_linux = cfg!(target_os = "linux");

    let lower_name = |a: &serde_json::Value| a["name"].as_str().map(|n| n.to_lowercase());

    // Prefer a ZIP built for this OS (Linux releases ship `-linux` archives,
    // Windows releases ship plain/-windows ones) before falling back to any ZIP.
    let os_zip = |n: &str| -> bool {
        if !n.ends_with(".zip") {
            return false;
        }
        if os_is_linux {
            n.contains("linux")
        } else {
            !n.contains("linux")
        }
    };
    let native_lib = |n: &str| -> bool {
        if os_is_linux {
            n.ends_with(".so")
        } else {
            n.ends_with(".dll")
        }
    };

    let chosen = assets
        .iter()
        .find(|a| {
            preferred_asset
                .is_some_and(|wanted| a["name"].as_str() == Some(wanted))
        })
        .or_else(|| {
            assets.iter().find(|a| {
                contains_lower.as_ref().is_some_and(|needle| {
                    a["name"]
                        .as_str()
                        .is_some_and(|n| n.to_lowercase().contains(needle.as_str()))
                })
            })
        })
        .or_else(|| assets.iter().find(|a| lower_name(a).is_some_and(|n| os_zip(&n))))
        .or_else(|| {
            assets
                .iter()
                .find(|a| lower_name(a).is_some_and(|n| n.ends_with(".zip")))
        })
        .or_else(|| assets.iter().find(|a| lower_name(a).is_some_and(|n| n.ends_with(".7z"))))
        .or_else(|| assets.iter().find(|a| lower_name(a).is_some_and(|n| native_lib(&n))))
        .or_else(|| assets.iter().find(|a| lower_name(a).is_some_and(|n| n.ends_with(".dll"))))
        .or_else(|| assets.iter().find(|a| lower_name(a).is_some_and(|n| n.ends_with(".so"))))
        .context("No installable asset found in release")?;

    let name = chosen["name"].as_str().context("Asset has no name")?.to_string();
    let url = chosen["browser_download_url"]
        .as_str()
        .context("Asset has no download URL")?
        .to_string();

    let lower = name.to_lowercase();
    let ext = if lower.ends_with(".7z") {
        "7z"
    } else if lower.ends_with(".so") {
        "so"
    } else if lower.ends_with(".dll") {
        "dll"
    } else {
        "zip"
    }
    .to_string();

    let digest = chosen["digest"]
        .as_str()
        .map(normalize_digest)
        .filter(|d| !d.is_empty());

    Ok(SelectedAsset {
        name,
        url,
        ext,
        digest,
        size: chosen["size"].as_u64(),
    })
}

/// GitHub reports digests as `sha256:<hex>`; keep only the lowercase hex so it
/// can be compared directly with a local hash.
fn normalize_digest(raw: &str) -> String {
    raw.rsplit(':')
        .next()
        .unwrap_or(raw)
        .trim()
        .to_lowercase()
}

/// Resolve the latest release for `owner/repo`, picking the asset that best
/// matches the tool. Results are cached for 24 hours.
pub fn get_latest_github_release(
    owner: &str,
    repo: &str,
    preferred_asset: Option<&str>,
    preferred_asset_contains: Option<&str>,
) -> AnyResult<ReleaseInfo> {
    let cache_key = format!("{owner}/{repo}");

    if let Ok(cache) = github_cache().lock() {
        if let Some(cached) = cache.get(&cache_key) {
            let age = cached.cached_at.elapsed().unwrap_or(Duration::ZERO);
            if age.as_secs() < RELEASE_CACHE_TTL_SECS {
                return Ok(cached.info.clone());
            }
        }
    }

    let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/releases/latest");
    let resp = http_client()?
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .send()
        .context("Failed to fetch GitHub release")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        anyhow::bail!("GitHub API error {status}: {body}");
    }

    let release: serde_json::Value = resp
        .json()
        .context("Failed to parse GitHub release")?;

    let tag_name = release["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let assets = release["assets"]
        .as_array()
        .context("Release has no assets")?;

    let selected = select_asset(assets, preferred_asset, preferred_asset_contains)?;

    let published_at = release["published_at"]
        .as_str()
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let info = ReleaseInfo {
        tag_name,
        zip_url: selected.url,
        zip_name: selected.name,
        archive_ext: selected.ext,
        published_at,
        asset_digest: selected.digest,
        asset_size: selected.size,
    };

    if let Ok(mut cache) = github_cache().lock() {
        cache.insert(
            cache_key,
            CachedRelease {
                info: info.clone(),
                cached_at: SystemTime::now(),
            },
        );
    }

    Ok(info)
}

pub fn download_file(url: &str, dest: &Path) -> AnyResult<()> {
    let resp = http_client()?
        .get(url)
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .send()
        .context("Failed to start download")?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} for {}", resp.status(), url);
    }

    let bytes = resp.bytes().context("Failed to read download body")?;
    if bytes.is_empty() {
        anyhow::bail!("Downloaded file is empty: {url}");
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, &bytes).context("Failed to write downloaded file")?;
    Ok(())
}

/// If `dir` contains exactly one entry and that entry is a directory, return
/// it (GitHub releases often wrap everything in a versioned folder).
pub fn flatten_extracted_dir(dir: &Path) -> Option<PathBuf> {
    let mut entries = std::fs::read_dir(dir).ok()?;
    let first = entries.next()?.ok()?;
    if entries.next().is_some() {
        return None;
    }
    if first.path().is_dir() {
        Some(first.path())
    } else {
        None
    }
}

pub fn extract_archive(archive_path: &Path, archive_ext: &str, dest: &Path) -> AnyResult<()> {
    if archive_ext == "dll" || archive_ext == "so" {
        std::fs::create_dir_all(dest)?;
        let name = archive_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("payload.{archive_ext}"));
        std::fs::copy(archive_path, dest.join(name))?;
        return Ok(());
    }

    std::fs::create_dir_all(dest)?;

    if archive_ext == "7z" {
        sevenz_rust::decompress_file(archive_path, dest)
            .map_err(|e| anyhow::anyhow!("Failed to extract 7z archive: {e}"))?;
        return Ok(());
    }

    let file = std::fs::File::open(archive_path).context("Failed to open archive")?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to read ZIP archive")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("Failed to read ZIP entry")?;
        let out_path = match entry.enclosed_name() {
            Some(path) => dest.join(path),
            None => continue,
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&out_path).context("Failed to create file")?;
        std::io::copy(&mut entry, &mut out).context("Failed to extract file")?;
    }

    Ok(())
}

pub fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let mut installed = Vec::new();
    copy_dir_inner(src, dest, dest, &mut installed)?;
    Ok(installed)
}

fn copy_dir_inner(
    src: &Path,
    dest: &Path,
    base: &Path,
    installed: &mut Vec<String>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
            copy_dir_inner(&src_path, &dest_path, base, installed)?;
        } else {
            std::fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("{}: {e}", dest_path.display()))?;
            let relative = dest_path
                .strip_prefix(base)
                .unwrap_or(&dest_path)
                .to_string_lossy()
                .to_string();
            installed.push(relative);
        }
    }
    Ok(())
}

/// Lowercase hex SHA-256 of a file, used to match a deployed binary against
/// the digest GitHub publishes for its release asset.
pub fn sha256_file(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).ok()?;
    Some(format!("{:x}", hasher.finalize()))
}

pub fn is_dir_populated(dir: &Path) -> bool {
    dir.is_dir()
        && std::fs::read_dir(dir)
            .ok()
            .and_then(|mut entries| entries.next())
            .is_some()
}

pub fn remove_dir_recursive(dir: &Path) -> Result<(), String> {    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                remove_dir_recursive(&path)?;
            } else {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
        std::fs::remove_dir(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_digest_algorithm_prefix() {
        assert_eq!(
            normalize_digest("sha256:D71C1C22A7D83BDD0F43371C9D28CE7B7BFC09BC3BEB0074531EE1DE79DAC32A"),
            "d71c1c22a7d83bdd0f43371c9d28ce7b7bfc09bc3beb0074531ee1de79dac32a"
        );
        assert_eq!(normalize_digest("abcdef"), "abcdef");
    }

    #[test]
    fn hashes_a_file_like_github_reports_it() {
        let dir = std::env::temp_dir().join("lumaforge-panel-sha256");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("blob.bin");
        std::fs::write(&file, b"abc").unwrap();

        // Well-known SHA-256 of "abc".
        assert_eq!(
            sha256_file(&file).as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

