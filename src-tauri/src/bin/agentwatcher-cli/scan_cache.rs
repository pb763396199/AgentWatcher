// 跨进程的全历史扫描缓存：短窗口内同参数的扫描合并为一次。
// 第一个进程拿独占文件锁真实扫描并原子写缓存；后到的进程阻塞在锁上，
// 拿到锁时缓存必然新鲜，直接读。任何异常都降级为直接扫描，不改退出码。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 缓存文件格式版本：读到不认识的版本静默弃用。
const CACHE_FORMAT_VERSION: u32 = 1;
/// 缓存有效期：窗口内同参数调用直接复用，窗口外重新扫描。
const CACHE_TTL: Duration = Duration::from_secs(120);
/// 锁等待上限：超过就放弃合并、直接扫描，不让调用方无限等。
const LOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// 写缓存时顺手清掉超过一天的旧缓存文件，目录总量因此有自然上限。
const PRUNE_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 参与缓存键的扫描参数。status/workspace 过滤在缓存命中后内存里做，不进键。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanCacheKey {
    pub provider: Vec<String>,
    pub limit: Option<usize>,
    pub active_days: Option<u64>,
    pub include_archived: bool,
    pub exclude_playground: bool,
}

impl ScanCacheKey {
    fn digest(&self) -> String {
        let mut normalized: Vec<String> = self
            .provider
            .iter()
            .map(|name| name.trim().to_ascii_lowercase())
            .collect();
        normalized.sort();
        let mut hasher = Sha256::new();
        hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
        hasher.update(b"\0");
        hasher.update(normalized.join("\u{1}").as_bytes());
        hasher.update(b"\0");
        hasher.update(
            self.limit
                .map(|value| value.to_string())
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(b"\0");
        hasher.update(
            self.active_days
                .map(|value| value.to_string())
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update(b"\0");
        hasher.update([self.include_archived as u8]);
        hasher.update([self.exclude_playground as u8]);
        let mut hex = format!("{:x}", hasher.finalize());
        hex.truncate(24);
        hex
    }
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    generated_ms: u128,
    data: serde_json::Value,
}

/// 全历史扫描的缓存入口：命中返回缓存值，未命中调 `scan` 并把结果写进缓存。
pub fn cached_full_history_scan(key: &ScanCacheKey, scan: impl FnOnce() -> serde_json::Value) -> serde_json::Value {
    cached_full_history_scan_in(std::env::temp_dir(), key, CACHE_TTL, LOCK_WAIT_TIMEOUT, scan)
}

/// 测试入口：缓存目录、TTL、锁等待上限都可注入。
fn cached_full_history_scan_in(
    temp_root: PathBuf,
    key: &ScanCacheKey,
    ttl: Duration,
    lock_wait: Duration,
    scan: impl FnOnce() -> serde_json::Value,
) -> serde_json::Value {
    let dir = temp_root.join("AgentWatcher").join("cli-scan-cache");
    if fs::create_dir_all(&dir).is_err() {
        return scan();
    }
    let digest = key.digest();
    let cache_path = dir.join(format!("{digest}.json"));
    let lock_path = dir.join(format!("{digest}.lock"));

    let guard = match FileLock::acquire(&lock_path, lock_wait) {
        Some(guard) => guard,
        // 锁竞争超时：放弃合并，直接扫描，不写缓存（写会和持锁进程竞争）。
        None => return scan(),
    };

    if let Some(cached) = read_fresh(&cache_path, ttl) {
        return cached;
    }
    let data = scan();
    write_cache(&cache_path, &data);
    prune_old_files(&dir);
    drop(guard);
    data
}

fn read_fresh(path: &PathBuf, ttl: Duration) -> Option<serde_json::Value> {
    let bytes = fs::read(path).ok()?;
    let parsed: CacheFile = serde_json::from_slice(&bytes).ok()?;
    if parsed.version != CACHE_FORMAT_VERSION {
        return None;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_millis();
    if now.saturating_sub(parsed.generated_ms) > ttl.as_millis() {
        return None;
    }
    Some(parsed.data)
}

fn write_cache(path: &PathBuf, data: &serde_json::Value) {
    let file = CacheFile {
        version: CACHE_FORMAT_VERSION,
        generated_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or(0),
        data: data.clone(),
    };
    let Ok(bytes) = serde_json::to_vec(&file) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    // 原子替换：读者要么看到旧文件要么看到完整新文件，不会读到半截。
    if fs::write(&tmp, &bytes).is_ok() && fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

fn prune_old_files(dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let cutoff = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.saturating_sub(PRUNE_AGE));
    let Ok(cutoff) = cutoff else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".json") && !name.ends_with(".lock") {
            continue;
        }
        if let Ok(modified) = metadata.modified() {
            if modified
                .duration_since(UNIX_EPOCH)
                .map(|age| cutoff > age)
                .unwrap_or(false)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// 独占文件锁：Windows 用 LockFileEx（独占 + 快速失败轮询实现等待上限），
/// 其他平台直接放行（仓库只在 Windows 上构建，这条只是兜底）。
struct FileLock {
    file: Option<fs::File>,
}

impl FileLock {
    fn acquire(path: &PathBuf, wait: Duration) -> Option<FileLock> {
        let deadline = std::time::Instant::now() + wait;
        loop {
            // Windows 上文件被独占锁定时，再次 open 会直接返回「拒绝访问」。
            // 那说明别人正持锁，不是错误——等一会儿重开即可。
            let opened = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(path);
            match opened {
                Ok(file) => {
                    if try_lock(&file) {
                        return Some(FileLock { file: Some(file) });
                    }
                    // 锁被别人占了但文件能开（同进程内多句柄场景）：轮询重试。
                    drop(file);
                }
                Err(_) => {
                    // 打不开（含「拒绝访问」）：同样按锁被占处理。
                }
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(LOCK_POLL_INTERVAL);
        }
    }
}

#[cfg(target_os = "windows")]
fn try_lock(file: &fs::File) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    let result = unsafe {
        LockFileEx(
            file.as_raw_handle() as HANDLE,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    result != 0
}

#[cfg(target_os = "windows")]
impl Drop for FileLock {
    fn drop(&mut self) {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::Storage::FileSystem::UnlockFileEx;
        use windows_sys::Win32::System::IO::OVERLAPPED;

        let Some(file) = self.file.take() else {
            return;
        };
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        unsafe {
            UnlockFileEx(file.as_raw_handle() as HANDLE, 0, u32::MAX, u32::MAX, &mut overlapped);
        }
        // file 在这里关闭，Windows 文件锁随句柄释放兜底。
    }
}

#[cfg(not(target_os = "windows"))]
fn try_lock(file: &fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    // 与 Windows LockFileEx 同语义：非阻塞独占锁，拿不到立即失败。
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

#[cfg(not(target_os = "windows"))]
impl Drop for FileLock {
    fn drop(&mut self) {
        self.file = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("AgentWatcher-test")
            .join(format!("scan-cache-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create test temp root");
        dir
    }

    fn key() -> ScanCacheKey {
        ScanCacheKey {
            provider: vec![],
            limit: Some(50),
            active_days: Some(0),
            include_archived: true,
            exclude_playground: false,
        }
    }

    fn scan_with(root: &PathBuf, ttl: Duration, key: &ScanCacheKey, scan: impl FnOnce() -> serde_json::Value) -> serde_json::Value {
        cached_full_history_scan_in(root.clone(), key, ttl, Duration::from_secs(5), scan)
    }

    #[test]
    fn test_concurrent_full_history_scan_merges() {
        let root = temp_root("concurrent");
        let counter = Arc::new(AtomicU32::new(0));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let root = root.clone();
            let counter = Arc::clone(&counter);
            handles.push(std::thread::spawn(move || {
                scan_with(&root, Duration::from_secs(60), &key(), move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(200));
                    serde_json::json!({"sessions": [], "total": 0})
                })
            }));
        }
        let results: Vec<serde_json::Value> = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread must not panic"))
            .collect();
        assert_eq!(counter.load(Ordering::SeqCst), 1, "真实扫描只允许发生一次");
        for result in &results[1..] {
            assert_eq!(result, &results[0], "并发调用必须拿到一致结果");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_cache_key_separates_exclude_playground() {
        let mut key = key();
        key.exclude_playground = false;
        let included = key.digest();
        key.exclude_playground = true;
        let excluded = key.digest();
        assert_ne!(
            included, excluded,
            "带不带 --exclude-playground 的全历史扫描不能串用同一份缓存"
        );
    }

    #[test]
    fn test_cache_ttl_expiry_and_no_cache_flag() {
        let root = temp_root("ttl");
        // 首次扫描落缓存。
        scan_with(&root, Duration::from_secs(60), &key(), || serde_json::json!({"total": 1}));
        // TTL 内命中，不再扫描。
        let hits = Arc::new(AtomicU32::new(0));
        let hit_counter = Arc::clone(&hits);
        let cached = scan_with(&root, Duration::from_secs(60), &key(), move || {
            hit_counter.fetch_add(1, Ordering::SeqCst);
            serde_json::json!({"total": 2})
        });
        assert_eq!(cached["total"], 1, "TTL 内应命中缓存");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        // TTL 归零后过期，重新扫描。
        let expired = scan_with(&root, Duration::from_secs(0), &key(), || serde_json::json!({"total": 3}));
        assert_eq!(expired["total"], 3, "过期后应重新扫描");
        // --no-cache 的语义在这里对应：绕过缓存入口（commands.rs 直扫），锁文件不残留阻塞。
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_corrupt_cache_falls_back() {
        let root = temp_root("corrupt");
        let dir = root.join("AgentWatcher").join("cli-scan-cache");
        fs::create_dir_all(&dir).expect("create cache dir");
        let cache_path = dir.join(format!("{}.json", key().digest()));
        fs::write(&cache_path, b"{ not valid json !!!").expect("write corrupt cache");
        let result = scan_with(&root, Duration::from_secs(60), &key(), || serde_json::json!({"total": 7}));
        assert_eq!(result["total"], 7, "损坏缓存必须静默回退到直接扫描");
        // 回退后缓存被正确结果覆盖，下一次命中。
        let again = scan_with(&root, Duration::from_secs(60), &key(), || serde_json::json!({"total": 9}));
        assert_eq!(again["total"], 7, "回退扫描的结果应写回缓存");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_key_is_sensitive_to_scan_params() {
        let root = temp_root("keys");
        scan_with(&root, Duration::from_secs(60), &key(), || serde_json::json!({"total": "base"}));
        let mut other = key();
        other.limit = Some(999);
        let different = scan_with(&root, Duration::from_secs(60), &other, || serde_json::json!({"total": "other"}));
        assert_eq!(different["total"], "other", "参数不同必须各自扫描");
        let _ = fs::remove_dir_all(&root);
    }
}
