use std::io::{self, Write};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::pdf_engine::OutputPathResolver;

/// Kullanici ile etkilesimli CLI diyalog yoneticisi.
pub struct InteractivePrompt;

impl InteractivePrompt {
    /// Konsoldan tek satirlik metin okur.
    pub fn prompt_line(message: &str, default_val: Option<&str>) -> Result<String> {
        if let Some(def) = default_val {
            print!("{} [Varsayılan: {}]: ", message, def);
        } else {
            print!("{}: ", message);
        }
        io::stdout().flush().context("stdout flush edilemedi")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Kullanici girdisi okunamadi")?;

        let trimmed = input.trim();
        if trimmed.is_empty() {
            if let Some(def) = default_val {
                return Ok(def.to_string());
            }
        }

        Ok(trimmed.to_string())
    }

    /// Kullaniciya dosya adi sorar, bos birakirsa default "{stem}_{target_lang}.pdf" doner.
    pub fn ask_output_filename(input_path: &Path, target_lang: &str) -> Result<String> {
        let default_name = OutputPathResolver::resolve_filename(input_path, None, target_lang);
        let answer = Self::prompt_line("Çıktı dosya adı ne olsun?", Some(&default_name))?;
        Ok(OutputPathResolver::resolve_filename(input_path, Some(&answer), target_lang))
    }

    /// Kullaniciya cikti klasorunu sorar, bos birakirsa orijinal dosyanin bulundugu klasor doner.
    pub fn ask_output_directory(input_path: &Path) -> Result<PathBuf> {
        let default_dir = OutputPathResolver::resolve_directory(input_path, None);
        let default_dir_str = default_dir.to_string_lossy();
        let answer = Self::prompt_line("Çıkarılacak dosya yolu (klasör) ne olsun?", Some(&default_dir_str))?;
        Ok(OutputPathResolver::resolve_directory(input_path, Some(&answer)))
    }

    /// Kaynak ve hedef dili interaktif sorar.
    pub fn ask_languages() -> Result<(String, String)> {
        let source_lang = Self::prompt_line("Kaynak dil (otomatik tespit için 'auto')", Some("auto"))?;
        let target_lang = Self::prompt_line("Çevrilecek hedef dil (örnek: tr, en, de, fr, es, ar, zh)", Some("tr"))?;
        Ok((source_lang, target_lang))
    }
}
