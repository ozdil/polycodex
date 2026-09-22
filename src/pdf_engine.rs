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
        // 1. Girdi dosyasinin varligini, sembolik bag ve boyut guvenligini denetle
        let meta = std::fs::symlink_metadata(input_path)
            .with_context(|| format!("Girdi dosyasi bulunamadi: {:?}", input_path))?;
        
        if meta.file_type().is_symlink() {
            anyhow::bail!("Guvenlik uyarisi: Sembolik bag (symlink) olan dosyalar islenemez: {:?}", input_path);
        }

        if meta.len() > MAX_INPUT_FILE_SIZE {
            anyhow::bail!(
                "Guvenlik uyarisi: Girdi dosyasi izin verilen 2 GiB tavan sinirini asmaktadir: {} bayt",
                meta.len()
            );
        }

        // 2. PDF belgesini yukle
        let mut doc = Document::load(input_path)
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

    /// Tek bir sayfanin icerik akisini (Content Stream) ayristirir ve mikrotipografik olarak donusturur.
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

        let mut new_operations = Vec::with_capacity(content.operations.len());
        let mut in_text_block = false;

        for op in content.operations {
            match op.operator.as_str() {
                // Metin blogu baslangici
                "BT" => {
                    in_text_block = true;
                    new_operations.push(op);
                }
                // Metin blogu bitisi
                "ET" => {
                    in_text_block = false;
                    new_operations.push(op);
                }
                // Metin gosterme operatorleri (Tj, TJ, ', ")
                "Tj" if in_text_block => {
                    let transformed = self
                        .transform_tj_op(op, source_lang, target_lang)
                        .await?;
                    new_operations.extend(transformed);
                }
                "TJ" if in_text_block => {
                    let transformed = self
                        .transform_tj_array_op(op, source_lang, target_lang)
                        .await?;
                    new_operations.extend(transformed);
                }
                // Diger tum operatorler (Do: Gorseller, re: dikdortgenler, m/l/c: cizgiler, f/S: boyama/kontur)
                // KESINLIKLE DOKUNULMADAN KORUNUR (1-1 Mizanpaj ve gorsel butunlugu).
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

    /// Tj (tekli metin dizisi) operatorunu mizanpaji bozmayacak sekilde donusturur.
    async fn transform_tj_op(
        &self,
        op: Operation,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<Operation>> {
        if op.operands.is_empty() {
            return Ok(vec![op]);
        }

        let operand = &op.operands[0];
        let original_text = match extract_text_from_object(operand) {
            Some(t) if !t.trim().is_empty() && should_translate(&t) => t,
            _ => return Ok(vec![op]),
        };

        let translated = self
            .translator
            .translate_block(&original_text, source_lang, target_lang)
            .await?;

        // Mikrotipografik sigdirma:
        // Eger cevrilen metin orijinalden uzunsa, yatay olcek (Tz) uygulayarak sinirlayici kutudan tasmasini onle
        let mut ops = Vec::new();
        let orig_len = original_text.chars().count().max(1);
        let trans_len = translated.chars().count().max(1);

        let scale_ratio = if trans_len > orig_len {
            let ratio = (orig_len as f32 / trans_len as f32) * 100.0;
            // Maksimum %70 daraltma toleransi uygula
            ratio.clamp(70.0, 100.0)
        } else {
            100.0
        };

        if (scale_ratio - 100.0).abs() > 0.5 {
            // Tz (Horizontal Scaling) operatoru ekle
            ops.push(Operation::new("Tz", vec![Object::Real(scale_ratio)]));
        }

        ops.push(Operation::new(
            "Tj",
            vec![Object::String(
                translated.into_bytes(),
                StringFormat::Literal,
            )],
        ));

        // Eger Tz uygulandiysa normale dondur (%100)
        if (scale_ratio - 100.0).abs() > 0.5 {
            ops.push(Operation::new("Tz", vec![Object::Real(100.0)]));
        }

        Ok(ops)
    }

    /// TJ (kerning dizisi iceren metin) operatorunu ayristirir ve mizanpaj koruyarak donusturur.
    async fn transform_tj_array_op(
        &self,
        op: Operation,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<Operation>> {
        if op.operands.is_empty() {
            return Ok(vec![op]);
        }

        let array_obj = match &op.operands[0] {
            Object::Array(arr) => arr,
            _ => return Ok(vec![op]),
        };

        // TJ icerisindeki tum metin parcalarinin birlesimi
        let mut full_text = String::new();
        for item in array_obj {
            if let Some(s) = extract_text_from_object(item) {
                full_text.push_str(&s);
            }
        }

        if full_text.trim().is_empty() || !should_translate(&full_text) {
            return Ok(vec![op]);
        }

        let translated = self
            .translator
            .translate_block(&full_text, source_lang, target_lang)
            .await?;

        let orig_len = full_text.chars().count().max(1);
        let trans_len = translated.chars().count().max(1);
        let mut ops = Vec::new();

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
                translated.into_bytes(),
                StringFormat::Literal,
            )],
        ));

        if (scale_ratio - 100.0).abs() > 0.5 {
            ops.push(Operation::new("Tz", vec![Object::Real(100.0)]));
        }

        Ok(ops)
    }

    /// Cikti dosyasini atomik ve guvenli olarak kaydeder.
    fn save_atomically(&self, doc: &mut Document, target_path: &Path) -> Result<()> {
        let parent_dir = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."));

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

/// PDF nesnesinden okunabilir metni ayiklar.
fn extract_text_from_object(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => {
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
