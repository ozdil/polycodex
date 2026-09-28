use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, StringFormat};
use indicatif::{ProgressBar, ProgressStyle};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::lock::SingleInstanceLock;
use crate::translator::CachedTranslator;

pub const MAX_ALLOWED_PAGES: usize = 5000;
pub const MAX_INPUT_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB DoS tavan siniri

/// Denetler: Cikti yolu kritik sistem dizinlerine (/etc, /usr, /boot, /sys, /proc, /dev, /root vb.) yonlenemez.
pub fn is_confined_output_path(path: &Path) -> bool {
    let canonical = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else if let Some(parent) = path.parent() {
        if parent.exists() {
            let parent_canon = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
            parent_canon.join(path.file_name().unwrap_or_default())
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    let path_str = canonical.to_string_lossy();
    let forbidden_prefixes = [
        "/etc", "/usr", "/boot", "/sys", "/proc", "/dev", "/root",
        "/bin", "/sbin", "/lib", "/lib64",
    ];

    for prefix in &forbidden_prefixes {
        if path_str == *prefix || path_str.starts_with(&format!("{}/", prefix)) {
            return false;
        }
    }
    true
}

/// Mizanpaj koruyucu PDF cevirici motoru.
pub struct PdfTranslatorEngine<'a> {
    translator: &'a CachedTranslator,
}

impl<'a> PdfTranslatorEngine<'a> {
    pub fn new(translator: &'a CachedTranslator) -> Self {
        Self { translator }
    }

    /// PDF cevirme islemini gerceklestirir.
    /// - Tekil islem kilidi dogrulanir.
    /// - 5000 sayfa siniri denetlenir.
    /// - Mizanpaj, gorseller, vektorler 1-1 korunarak metinler cevrilir.
    /// - Guvenli atomik yazim ile cikti olusturulur.
    pub async fn translate_pdf(
        &self,
        _lock: &SingleInstanceLock,
        input_path: &Path,
        output_path: &Path,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<()> {
        // 1. Girdi dosyasinin varligini, duzenli dosya, sembolik bag, hardlink ve boyut guvenligini denetle
        let meta = std::fs::symlink_metadata(input_path)
            .with_context(|| format!("Girdi dosyasi bulunamadi: {:?}", input_path))?;
        
        if meta.file_type().is_symlink() {
            anyhow::bail!("Guvenlik uyarisi: Sembolik bag (symlink) olan dosyalar islenemez: {:?}", input_path);
        }

        if !meta.file_type().is_file() {
            anyhow::bail!("Guvenlik uyarisi: Girdi bir duzenli dosya (regular file) degildir: {:?}", input_path);
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() > 1 {
                anyhow::bail!("Guvenlik uyarisi: Girdi dosyasi coklu baglantiya (hardlink) sahiptir: {:?}", input_path);
            }
        }

        if meta.len() > MAX_INPUT_FILE_SIZE {
            anyhow::bail!(
                "Guvenlik uyarisi: Girdi dosyasi izin verilen 2 GiB tavan sinirini asmaktadir: {} bayt",
                meta.len()
            );
        }

        // Cikti yolunun sistem dizinlerine yazma girisimi olup olmadigini denetle
        if !is_confined_output_path(output_path) {
            anyhow::bail!("Guvenlik ihlali: Sistem dizinlerine cikti yazilamaz: {:?}", output_path);
        }

        // 2. PDF belgesini sinirlandirilmis akis ve O_NOFOLLOW ile yukle (take(MAX + 1))
        use std::io::Read;
        #[cfg(unix)]
        let file = {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(input_path)
                .with_context(|| format!("Girdi dosyasi acilamadi: {:?}", input_path))?
        };
        #[cfg(not(unix))]
        let file = std::fs::File::open(input_path)
            .with_context(|| format!("Girdi dosyasi acilamadi: {:?}", input_path))?;

        let mut bounded_reader = file.take(MAX_INPUT_FILE_SIZE + 1);
        let mut doc = Document::load_from(&mut bounded_reader)
            .with_context(|| format!("PDF belgesi ayristirilamadi: {:?}", input_path))?;

        // 3. 5000 sayfa siniri kontrolu
        let pages = doc.get_pages();
        let page_count = pages.len();
        if page_count == 0 {
            anyhow::bail!("PDF belgesinde hic sayfa bulunamadi.");
        }
        if page_count > MAX_ALLOWED_PAGES {
            anyhow::bail!(
                "Belge sayfa sayisi ({}) izin verilen maksimum {} sayfa sinirini asmaktadir.",
                page_count,
                MAX_ALLOWED_PAGES
            );
        }

        tracing::info!(
            "PDF yuklendi. Toplam sayfa sayisi: {}. Ceviri baslatiliyor: {} -> {}",
            page_count,
            source_lang,
            target_lang
        );

        let pb = ProgressBar::new(page_count as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] Sayfa {pos}/{len} (%{percent}) - Kalan: {eta}")
                .unwrap()
                .progress_chars("#>-"),
        );

        // 4. Sayfalari tek tek isle (Mizanpaj ve gorsel koruma)
        for (_page_num, page_id) in pages {
            self.process_page(&mut doc, page_id, source_lang, target_lang).await?;
            pb.inc(1);
        }

        pb.finish_with_message("Tum sayfalar basariyla cevrildi.");

        // 5. Cikti dosyasini atomik ve guvenli sekilde yaz
        self.save_atomically(&mut doc, output_path)?;

        Ok(())
    }

    /// Tek bir sayfanin icerik akisini (Content Stream) ayristirir, metinleri toplu cevirir (page-batching) ve mikrotipografik olarak donusturur.
    async fn process_page(
        &self,
        doc: &mut Document,
        page_id: lopdf::ObjectId,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<()> {
        let content = match doc.get_and_decode_page_content(page_id) {
            Ok(c) => c,
            Err(_) => return Ok(()), // Icerigi bos veya cozumlenemeyen sayfalar
        };

        // 1. Gecis: Sayfadaki tum cevrilmesi gereken metinleri tespit et ve topla
        let mut in_text_block = false;
        let mut texts_to_translate = Vec::new();
        let mut text_item_indices = Vec::new();

        for (op_idx, op) in content.operations.iter().enumerate() {
            match op.operator.as_str() {
                "BT" => in_text_block = true,
                "ET" => in_text_block = false,
                "Tj" | "'" if in_text_block => {
                    let operand = if op.operator == "'" {
                        op.operands.first()
                    } else {
                        op.operands.first()
                    };
                    if let Some(opnd) = operand {
                        if let Some(t) = extract_text_from_object(opnd) {
                            if !t.trim().is_empty() && should_translate(&t) {
                                text_item_indices.push((op_idx, t.clone()));
                                texts_to_translate.push(t);
                            }
                        }
                    }
                }
                "\"" if in_text_block => {
                    if op.operands.len() >= 3 {
                        if let Some(t) = extract_text_from_object(&op.operands[2]) {
                            if !t.trim().is_empty() && should_translate(&t) {
                                text_item_indices.push((op_idx, t.clone()));
                                texts_to_translate.push(t);
                            }
                        }
                    }
                }
                "TJ" if in_text_block => {
                    if let Some(Object::Array(arr)) = op.operands.first() {
                        let mut full_text = String::new();
                        for item in arr {
                            if let Some(s) = extract_text_from_object(item) {
                                full_text.push_str(&s);
                            }
                        }
                        if !full_text.trim().is_empty() && should_translate(&full_text) {
                            text_item_indices.push((op_idx, full_text.clone()));
                            texts_to_translate.push(full_text);
                        }
                    }
                }
                _ => {}
            }
        }

        // 2. Sayfadaki metinleri tekil batch olarak ceviriciye ilet (JEV-POLY-01 Kurali)
        let translated_batch = if !texts_to_translate.is_empty() {
            self.translator
                .translate_batch(&texts_to_translate, source_lang, target_lang)
                .await?
        } else {
            Vec::new()
        };

        let mut translation_map = std::collections::HashMap::new();
        for ((op_idx, orig_text), trans_text) in text_item_indices.into_iter().zip(translated_batch.into_iter()) {
            translation_map.insert(op_idx, (orig_text, trans_text));
        }

        // 3. Gecis: Operatorleri mikrotipografik olarak yeniden olustur
        let mut new_operations = Vec::with_capacity(content.operations.len());
        in_text_block = false;

        for (op_idx, op) in content.operations.into_iter().enumerate() {
            match op.operator.as_str() {
                "BT" => {
                    in_text_block = true;
                    new_operations.push(op);
                }
                "ET" => {
                    in_text_block = false;
                    new_operations.push(op);
                }
                "Tj" if in_text_block => {
                    if let Some((orig, trans)) = translation_map.get(&op_idx) {
                        new_operations.extend(Self::build_microtypography_ops(orig, trans));
                    } else {
                        new_operations.push(op);
                    }
                }
                "TJ" if in_text_block => {
                    if let Some((orig, trans)) = translation_map.get(&op_idx) {
                        new_operations.extend(Self::build_microtypography_ops(orig, trans));
                    } else {
                        new_operations.push(op);
                    }
                }
                "'" if in_text_block => {
                    new_operations.push(Operation::new("T*", vec![]));
                    if let Some((orig, trans)) = translation_map.get(&op_idx) {
                        new_operations.extend(Self::build_microtypography_ops(orig, trans));
                    } else {
                        new_operations.push(Operation::new("Tj", op.operands));
                    }
                }
                "\"" if in_text_block => {
                    if op.operands.len() >= 3 {
                        new_operations.push(Operation::new("Tw", vec![op.operands[0].clone()]));
                        new_operations.push(Operation::new("Tc", vec![op.operands[1].clone()]));
                        new_operations.push(Operation::new("T*", vec![]));
                        if let Some((orig, trans)) = translation_map.get(&op_idx) {
                            new_operations.extend(Self::build_microtypography_ops(orig, trans));
                        } else {
                            new_operations.push(Operation::new("Tj", vec![op.operands[2].clone()]));
                        }
                    } else {
                        new_operations.push(op);
                    }
                }
                _ => {
                    new_operations.push(op);
                }
            }
        }

        let new_content = Content {
            operations: new_operations,
        };

        let encoded = new_content.encode()?;
        doc.change_page_content(page_id, encoded)?;

        Ok(())
    }

    /// Orijinal ve cevrilmis metne gore Tz mikrotipografik olcekleme operatorlerini olusturur.
    fn build_microtypography_ops(original_text: &str, translated_text: &str) -> Vec<Operation> {
        let mut ops = Vec::new();
        let orig_len = original_text.chars().count().max(1);
        let trans_len = translated_text.chars().count().max(1);

        let scale_ratio = if trans_len > orig_len {
            let ratio = (orig_len as f32 / trans_len as f32) * 100.0;
            ratio.clamp(70.0, 100.0)
        } else {
            100.0
        };

        if (scale_ratio - 100.0).abs() > 0.5 {
            ops.push(Operation::new("Tz", vec![Object::Real(scale_ratio)]));
        }

        ops.push(Operation::new(
            "Tj",
            vec![Object::String(
                translated_text.as_bytes().to_vec(),
                StringFormat::Literal,
            )],
        ));

        if (scale_ratio - 100.0).abs() > 0.5 {
            ops.push(Operation::new("Tz", vec![Object::Real(100.0)]));
        }

        ops
    }



    /// Cikti dosyasini atomik ve guvenli olarak kaydeder.
    fn save_atomically(&self, doc: &mut Document, target_path: &Path) -> Result<()> {
        if !is_confined_output_path(target_path) {
            anyhow::bail!("Guvenlik ihlali: Sistem dizinlerine cikti yazilamaz: {:?}", target_path);
        }

        let parent_dir = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."));

        #[cfg(unix)]
        let current_uid = unsafe { libc::geteuid() };

        if parent_dir.exists() {
            let p_meta = std::fs::symlink_metadata(parent_dir)?;
            if p_meta.file_type().is_symlink() {
                anyhow::bail!("Guvenlik ihlali: Cikti ust dizini sembolik bagdir: {:?}", parent_dir);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if p_meta.uid() != current_uid {
                    anyhow::bail!("Guvenlik ihlali: Cikti ust dizini mevcut kullaniciya ait degildir: {:?}", parent_dir);
                }
            }
        }

        if target_path.exists() {
            let t_meta = std::fs::symlink_metadata(target_path)?;
            if t_meta.file_type().is_symlink() {
                anyhow::bail!("Guvenlik ihlali: Hedef dosya sembolik bagdir: {:?}", target_path);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if t_meta.uid() != current_uid {
                    anyhow::bail!("Guvenlik ihlali: Hedef dosya baska bir kullaniciya aittir: {:?}", target_path);
                }
                if t_meta.nlink() > 1 {
                    anyhow::bail!("Guvenlik ihlali: Hedef dosya coklu baglantiya (hardlink) sahiptir: {:?}", target_path);
                }
            }
        }

        // Gecici dosya hedef dizinde olusturulur (farkli disk bolumleri arasi atomic rename hatasini onlemek icin)
        let temp_file = tempfile::Builder::new()
            .prefix(".tmp_pdf_")
            .suffix(".pdf")
            .tempfile_in(parent_dir)
            .context("Gecici cikti dosyasi olusturulamadi")?;

        // Guvenli dosya izinleri: 0600 (Yalnizca dosya sahibi okuyup yazabilir)
        #[cfg(unix)]
        {
            if let Ok(file_meta) = temp_file.as_file().metadata() {
                let mut perms = file_meta.permissions();
                perms.set_mode(0o600);
                let _ = temp_file.as_file().set_permissions(perms);
            }
        }

        let temp_path = temp_file.path().to_path_buf();
        doc.save(&temp_path)
            .with_context(|| format!("PDF gecici dosyaya kaydedilemedi: {:?}", temp_path))?;

        // Atomik yer degistirme (POSIX rename / Windows MoveFileEx)
        temp_file
            .persist(target_path)
            .with_context(|| format!("Cikti dosyasi atomik olarak hedefe tasinamadi: {:?}", target_path))?;

        // Hedef dosya izinlerinin 0600 oldugunu kesinlestir
        #[cfg(unix)]
        {
            if let Ok(meta) = std::fs::metadata(target_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(target_path, perms);
            }
        }

        tracing::info!("PDF basariyla kaydedildi (0600): {:?}", target_path);
        Ok(())
    }
}

/// PDF nesnesinden okunabilir metni ayiklar (UTF-8, UTF-16BE/LE BOM ve Latin-1 destekli).
pub fn extract_text_from_object(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => {
            if bytes.is_empty() {
                return Some(String::new());
            }
            // UTF-16BE BOM: 0xFE, 0xFF
            if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                let u16_words: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                    .collect();
                if let Ok(s) = String::from_utf16(&u16_words) {
                    return Some(s);
                }
            }
            // UTF-16LE BOM: 0xFF, 0xFE
            if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
                let u16_words: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                if let Ok(s) = String::from_utf16(&u16_words) {
                    return Some(s);
                }
            }
            // Once UTF-8, olmazsa Latin1/Windows-1252 cozumlemesi dene
            if let Ok(s) = String::from_utf8(bytes.clone()) {
                Some(s)
            } else {
                Some(bytes.iter().map(|&b| b as char).collect())
            }
        }
        _ => None,
    }
}

/// Dosya adi ve hedef klasor varsayilanlarini cozumler.
pub struct OutputPathResolver;

impl OutputPathResolver {
    /// Kullanici tarafindan girilmemisse varsayilan cikti dosya adini belirler:
    /// "{orijinal_dosya_adi}_{hedef_dil}.pdf"
    pub fn resolve_filename(input_path: &Path, user_input: Option<&str>, target_lang: &str) -> String {
        if let Some(custom_name) = user_input {
            let trimmed = custom_name.trim();
            if !trimmed.is_empty() {
                if trimmed.to_lowercase().ends_with(".pdf") {
                    return trimmed.to_string();
                } else {
                    return format!("{}.pdf", trimmed);
                }
            }
        }

        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("dokuman");

        format!("{}_{}.pdf", stem, target_lang)
    }

    /// Kullanici tarafindan girilmemisse varsayilan cikti klasorunu belirler:
    /// Orijinal dosyanin bulundugu klasor.
    pub fn resolve_directory(input_path: &Path, user_input: Option<&str>) -> PathBuf {
        if let Some(custom_dir) = user_input {
            let trimmed = custom_dir.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }

        input_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Metnin cevrilmeye deger olup olmadigini (sayi, sembol veya tek harf olup olmadigini) denetler.
fn should_translate(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Sadece noktalama, bosluk veya sayilardan ibaretse cevirme (or: "1", "2020", "(", ")", ".")
    if trimmed.chars().all(|c| c.is_ascii_punctuation() || c.is_ascii_digit() || c.is_whitespace()) {
        return false;
    }
    // Tek harflik matematiksel degisken veya indeks ise (or: "W", "r", "x") cevirme
    if trimmed.len() <= 1 {
        return false;
    }
    true
}
