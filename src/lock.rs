use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use fs2::FileExt;
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

#[derive(Error, Debug)]
pub enum LockError {
    #[error("HATA: Halen devam etmekte olan baska bir PolyCodex cevirme islemi bulunmaktadir. Sistem ayni anda sadece tek bir dokuman isleyebilir.")]
    AlreadyRunning,

    #[error("Guvenlik ihlali: Kilit dosya konumu guvensiz veya sembolik bag (symlink) tespit edildi: {0}")]
    SecurityViolation(String),

    #[error("Kilit dosyasi olusturulamadi veya acilamadi: {0}")]
    Io(#[from] io::Error),
}

/// Tekil islem kilidi (Single Instance Process Guard).
/// Sistem genelinde ayni anda sadece bir PolyCodex ceviricisinin calismasini garanti eder.
/// RAII prensibiyle calisir; nesne kapsam disina ciktiginda kilit serbest kalir.
pub struct SingleInstanceLock {
    _file: File,
    _lock_path: PathBuf,
}

impl SingleInstanceLock {
    /// Guvenli, izole edilmis ve symlink saldirilarina karsi korumali tekil kilit edinir.
    /// Oncelik sirasi:
    /// 1. $XDG_RUNTIME_DIR/polycodex.lock (/run/user/<UID>/polycodex.lock - 0700 izinli)
    /// 2. $XDG_CACHE_HOME/polycodex/polycodex.lock (~/.cache/polycodex/polycodex.lock - 0700 izinli)
    /// 3. /tmp/polycodex-<UID>/polycodex.lock (0700 izinli izole dizin)
    pub fn acquire() -> Result<Self, LockError> {
        let lock_path = Self::resolve_secure_lock_path()?;

        // Guvenlik Denetimi: Sembolik bag (Symlink) kontrolu (TOCTOU savunmasi)
        if lock_path.exists() {
            let meta = fs::symlink_metadata(&lock_path)
                .map_err(|e| LockError::Io(e))?;
            if meta.file_type().is_symlink() {
                return Err(LockError::SecurityViolation(format!(
                    "Kilit dosyasi bir sembolik bagdir (symlink attack mitigation): {:?}",
                    lock_path
                )));
            }
        }

        let mut open_options = OpenOptions::new();
        open_options.read(true).write(true).create(true).truncate(false);

        #[cfg(unix)]
        {
            // O_NOFOLLOW bayragi ile symlink takibini cekirdek seviyesinde engelle ve 0600 izinleriyle ac
            open_options.custom_flags(libc::O_NOFOLLOW);
            open_options.mode(0o600);
        }

        let file = open_options.open(&lock_path)?;

        // Dosya izinlerinin 0600 oldugunu kesinlestir
        #[cfg(unix)]
        {
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            let _ = file.set_permissions(perms);
        }

        // Kilit edinmeyi dene (bloklamadan, aninda hata vererek)
        match file.try_lock_exclusive() {
            Ok(()) => {
                tracing::info!("Tekil islem kilidi basariyla edinildi: {:?}", lock_path);
                Ok(Self {
                    _file: file,
                    _lock_path: lock_path,
                })
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Err(LockError::AlreadyRunning)
            }
            Err(e) => Err(LockError::Io(e)),
        }
    }

    /// Kullaniciya ozel, diger kullanicilardan izole ve guvenli kilit yolunu cozumler.
    fn resolve_secure_lock_path() -> Result<PathBuf, LockError> {
        // 1. Oncelik: XDG_RUNTIME_DIR (/run/user/<UID>)
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            let path = PathBuf::from(runtime_dir).join("polycodex.lock");
            return Ok(path);
        }

        // 2. Oncelik: XDG_CACHE_HOME veya ~/.cache/polycodex
        if let Ok(home) = std::env::var("HOME") {
            let cache_base = match std::env::var("XDG_CACHE_HOME") {
                Ok(c) => PathBuf::from(c),
                Err(_) => PathBuf::from(home).join(".cache"),
            };
            let poly_dir = cache_base.join("polycodex");
            if !poly_dir.exists() {
                fs::create_dir_all(&poly_dir)?;
                #[cfg(unix)]
                {
                    let mut perms = fs::metadata(&poly_dir)?.permissions();
                    perms.set_mode(0o700);
                    let _ = fs::set_permissions(&poly_dir, perms);
                }
            }
            return Ok(poly_dir.join("polycodex.lock"));
        }

        // 3. Oncelik: /tmp altinda kullanici UID'sine ozel 0700 dizin
        #[cfg(unix)]
        let uid = unsafe { libc::getuid() };
        #[cfg(not(unix))]
        let uid = 1000;

        let tmp_dir = std::env::temp_dir().join(format!("polycodex-{}", uid));
        if !tmp_dir.exists() {
            fs::create_dir_all(&tmp_dir)?;
            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&tmp_dir)?.permissions();
                perms.set_mode(0o700);
                let _ = fs::set_permissions(&tmp_dir, perms);
            }
        }

        Ok(tmp_dir.join("polycodex.lock"))
    }
}

impl Drop for SingleInstanceLock {
    fn drop(&mut self) {
        if let Err(e) = self._file.unlock() {
            tracing::warn!("Kilit serbest birakilirken uyari: {}", e);
        } else {
            tracing::info!("Tekil islem kilidi serbest birakildi: {:?}", self._lock_path);
        }
    }
}
