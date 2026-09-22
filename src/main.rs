use std::path::PathBuf;
use anyhow::{Context, Result};
use clap::Parser;
use polycodex::cli::CliArgs;
use polycodex::interactive::InteractivePrompt;
use polycodex::lock::{LockError, SingleInstanceLock};
use polycodex::pdf_engine::{OutputPathResolver, PdfTranslatorEngine};
use polycodex::translator::{
    CachedTranslator, LibreTranslateBackend, MockBackend, OllamaBackend, TranslationBackend,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Tekil Islem Kilidi Edinimi (Concurrency Guard)
    // Yürürlükte bir işlem varken ikinci bir işlem kesinlikle başlatılamaz!
    let lock = match SingleInstanceLock::acquire() {
        Ok(guard) => guard,
        Err(LockError::AlreadyRunning) => {
            eprintln!("\n==================================================================");
            eprintln!("HATA: Halen devam etmekte olan baska bir PDF cevirme islemi bulunmaktadir.");
            eprintln!("Sistem ayni anda sadece tek bir dokuman isleyebilir.");
            eprintln!("Lutfen diger islemin tamamlanmasini bekleyiniz.");
            eprintln!("==================================================================\n");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Kilit mekanizmasi baslatilamadi: {}", e);
            std::process::exit(1);
        }
    };

    let args = CliArgs::parse();

    println!("===============================================================");
    println!("  1-1 Mizanpaj ve Gorsel Koruyan Evrensel PDF Cevirici");
    println!("  (5000 Sayfa Kapasiteli, Tekil Islem Kilitli Acik Kaynak Motor)");
    println!("===============================================================\n");

    // 2. Girdi PDF Dosyasinin Belirlenmesi
    let input_path = match args.input {
        Some(p) => p,
        None if !args.non_interactive => {
            let path_str = InteractivePrompt::prompt_line("Cevirilecek PDF dosya yolunu giriniz", None)?;
            PathBuf::from(path_str)
        }
        None => {
            anyhow::bail!("Girdi dosyasi belirtilmedi. '--input <DOSYA>' parametresini kullaniniz.");
        }
    };

    if !input_path.exists() {
        anyhow::bail!("Belirtilen girdi dosyasi mevcut degil: {:?}", input_path);
    }

    // 3. Kaynak ve Hedef Dilin Belirlenmesi
    let (source_lang, target_lang) = match (args.source_lang, args.target_lang) {
        (Some(s), Some(t)) => (s, t),
        (s, t) if !args.non_interactive => {
            let (ask_s, ask_t) = InteractivePrompt::ask_languages()?;
            (s.unwrap_or(ask_s), t.unwrap_or(ask_t))
        }
        (Some(s), None) => (s, "tr".to_string()),
        (None, Some(t)) => ("auto".to_string(), t),
        (None, None) => ("auto".to_string(), "tr".to_string()),
    };

    // 4. Cikti Dosya Adinin Belirlenmesi
    // Kullanici degistirmezse default "{orijinal_ad}_{hedef_dil}.pdf"
    let output_filename = match args.output_name {
        Some(custom_name) => OutputPathResolver::resolve_filename(&input_path, Some(&custom_name), &target_lang),
        None if !args.non_interactive => {
            InteractivePrompt::ask_output_filename(&input_path, &target_lang)?
        }
        None => OutputPathResolver::resolve_filename(&input_path, None, &target_lang),
    };

    // 5. Cikarilacak Dosya Yolunun (Klasorunun) Belirlenmesi
    // Kullanici degistirmezse default girdi dosyasi ile ayni klasor
    let output_directory = match args.output_dir {
        Some(dir) => dir,
        None if !args.non_interactive => {
            InteractivePrompt::ask_output_directory(&input_path)?
        }
        None => OutputPathResolver::resolve_directory(&input_path, None),
    };

    if !output_directory.exists() {
        std::fs::create_dir_all(&output_directory)
            .with_context(|| format!("Cikti klasoru olusturulamadi: {:?}", output_directory))?;
    }

    let final_output_path = output_directory.join(&output_filename);

    println!("\nIslem Yapilandirmasi:");
    println!("  - Girdi Dosyasi   : {:?}", input_path);
    println!("  - Kaynak Dil      : {}", source_lang);
    println!("  - Hedef Dil       : {}", target_lang);
    println!("  - Cikti Dosya Adi : {}", output_filename);
    println!("  - Cikti Klasoru   : {:?}", output_directory);
    println!("  - Hedef Yol       : {:?}", final_output_path);
    println!("  - Ceviri Saglayici: {}", args.provider);
    println!("---------------------------------------------------------------\n");

    // 6. Ceviri Saglayicisinin Secimi
    let backend: Box<dyn TranslationBackend> = match args.provider.to_lowercase().as_str() {
        "libretranslate" => {
            let url = args.api_url.unwrap_or_else(|| "https://libretranslate.com".to_string());
            Box::new(LibreTranslateBackend::new(url, args.api_key))
        }
        "ollama" => {
            let url = args.api_url.unwrap_or_else(|| "http://localhost:11434".to_string());
            Box::new(OllamaBackend::new(url, args.ollama_model))
        }
        "mock" => Box::new(MockBackend),
        other => {
            println!("Bilinmeyen saglayici '{}', Mock motoruna geri donuluyor.", other);
            Box::new(MockBackend)
        }
    };

    let cached_translator = CachedTranslator::new(backend);
    let engine = PdfTranslatorEngine::new(&cached_translator);

    // 7. Ceviri Isleminin Baslatilmasi
    let start_time = std::time::Instant::now();
    engine
        .translate_pdf(
            &lock,
            &input_path,
            &final_output_path,
            &source_lang,
            &target_lang,
        )
        .await?;

    let elapsed = start_time.elapsed();
    println!("\n===============================================================");
    println!("BASARILI: PDF cevirisi 1-1 mizanpaj korunarak tamamlandi!");
    println!("Kaydedilen Dosya: {:?}", final_output_path);
    println!("Gecen Sure      : {:.2} saniye", elapsed.as_secs_f64());
    println!("===============================================================");

    Ok(())
}
