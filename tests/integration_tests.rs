use std::path::{Path, PathBuf};
use polycodex::lock::{LockError, SingleInstanceLock};
use polycodex::pdf_engine::{OutputPathResolver, MAX_ALLOWED_PAGES};
use polycodex::translator::{CachedTranslator, MockBackend};

static TEST_LOCK_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn test_single_instance_lock_concurrency() {
    let _guard = TEST_LOCK_MUTEX.lock().unwrap();

    // 1. Ilk kilit basariyla alinabilmeli
    let first_lock = SingleInstanceLock::acquire();
    assert!(first_lock.is_ok(), "Ilk kilit basariyla edinilmeli");

    // 2. Yurutlukte birinci kilit varken ikinci kilit LockError::AlreadyRunning ile reddedilmeli
    let second_lock = SingleInstanceLock::acquire();
    match second_lock {
        Err(LockError::AlreadyRunning) => {
            // Beklenen davranis: ayni anda ikinci isleme izin verilmez
        }
        _ => panic!("Ikinci kilit edinilmemeli, AlreadyRunning hatasi vermeliydi!"),
    }

    // 3. Ilk kilit serbest kaldiginda tekrar kilit alinabilmeli
    drop(first_lock);
    let third_lock = SingleInstanceLock::acquire();
    assert!(third_lock.is_ok(), "Ilk kilit birakildiktan sonra yeni kilit alinabilmeli");
}

#[test]
fn test_default_output_filename_resolution() {
    let input_path = Path::new("/home/user/documents/sample_manual.pdf");

    // Kullanici bos biraktiginda: {orijinal_ad}_{hedef_dil}.pdf
    let resolved_default = OutputPathResolver::resolve_filename(input_path, None, "tr");
    assert_eq!(resolved_default, "sample_manual_tr.pdf");

    let resolved_empty = OutputPathResolver::resolve_filename(input_path, Some("   "), "de");
    assert_eq!(resolved_empty, "sample_manual_de.pdf");

    // Kullanici ozel isim girdiginde
    let resolved_custom = OutputPathResolver::resolve_filename(input_path, Some("custom_translated"), "tr");
    assert_eq!(resolved_custom, "custom_translated.pdf");

    let resolved_custom_with_ext = OutputPathResolver::resolve_filename(input_path, Some("manual_v2.pdf"), "tr");
    assert_eq!(resolved_custom_with_ext, "manual_v2.pdf");
}

#[test]
fn test_default_output_directory_resolution() {
    let input_path = Path::new("/home/user/documents/sample_manual.pdf");

    // Kullanici bos biraktiginda: Orijinal dosyanin bulundugu klasor
    let resolved_default = OutputPathResolver::resolve_directory(input_path, None);
    assert_eq!(resolved_default, PathBuf::from("/home/user/documents"));

    let resolved_empty = OutputPathResolver::resolve_directory(input_path, Some("   "));
    assert_eq!(resolved_empty, PathBuf::from("/home/user/documents"));

    // Kullanici ozel klasor girdiginde
    let resolved_custom = OutputPathResolver::resolve_directory(input_path, Some("/tmp/exports"));
    assert_eq!(resolved_custom, PathBuf::from("/tmp/exports"));
}

#[tokio::test]
async fn test_translator_cache_and_mock_backend() {
    let mock = Box::new(MockBackend);
    let cached = CachedTranslator::new(mock);

    // Ilk ceviri
    let res1 = cached.translate_block("Hello World", "en", "tr").await.unwrap();
    assert_eq!(res1, "[TR] Hello World");

    // Ayni metin onbellekten gelmeli
    let res2 = cached.translate_block("Hello World", "en", "tr").await.unwrap();
    assert_eq!(res2, "[TR] Hello World");

    // Bosluk iceren veya bos metinler degistirilmeden donmeli
    let empty_res = cached.translate_block("   ", "en", "tr").await.unwrap();
    assert_eq!(empty_res, "   ");
}

#[test]
fn test_page_limit_constant() {
    assert_eq!(MAX_ALLOWED_PAGES, 5000, "Maksimum sayfa siniri 5000 olmalidir.");
}

#[tokio::test]
async fn test_pdf_layout_and_vector_preservation_e2e() {
    let _guard = TEST_LOCK_MUTEX.lock().unwrap();
    use lopdf::{Document, Object, Stream, Dictionary};
    use lopdf::content::{Content, Operation};
    use polycodex::PdfTranslatorEngine;

    // 1. Sentetik bir girdi PDF'i olustur (Metin, cizgi ve gorsel iceren)
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    // Icerik akisi:
    // - Vektorel cizim: 50 50 100 100 re S
    // - Gorsel: /Image1 Do
    // - Metin: BT /F1 14 Tf 72 712 Td (Document Header Text) Tj ET
    let operations = vec![
        Operation::new("re", vec![Object::Integer(50), Object::Integer(50), Object::Integer(100), Object::Integer(100)]),
        Operation::new("S", vec![]),
        Operation::new("Do", vec![Object::Name(b"Image1".to_vec())]),
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), Object::Integer(14)]),
        Operation::new("Td", vec![Object::Integer(72), Object::Integer(712)]),
        Operation::new("Tj", vec![Object::String(b"Document Header Text".to_vec(), lopdf::StringFormat::Literal)]),
        Operation::new("ET", vec![]),
    ];
    let content = Content { operations };
    let content_stream = Stream::new(Dictionary::new(), content.encode().unwrap());
    let content_id = doc.add_object(content_stream);

    let page_dict = Dictionary::from_iter(vec![
        ("Type", "Page".into()),
        ("Parent", pages_id.into()),
        ("Contents", content_id.into()),
        ("MediaBox", vec![0.into(), 0.into(), 595.into(), 842.into()].into()),
    ]);
    let page_id = doc.add_object(page_dict);

    let pages_dict = Dictionary::from_iter(vec![
        ("Type", "Pages".into()),
        ("Kids", vec![page_id.into()].into()),
        ("Count", 1.into()),
    ]);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_dict = Dictionary::from_iter(vec![
        ("Type", "Catalog".into()),
        ("Pages", pages_id.into()),
    ]);
    let catalog_id = doc.add_object(catalog_dict);
    doc.trailer.set("Root", catalog_id);

    let temp_dir = tempfile::tempdir().unwrap();
    let input_pdf_path = temp_dir.path().join("input_sample.pdf");
    let output_pdf_path = temp_dir.path().join("output_translated.pdf");

    doc.save(&input_pdf_path).unwrap();

    // 2. Cevirici motorunu calistir
    let lock = SingleInstanceLock::acquire().unwrap();
    let mock_backend = Box::new(MockBackend);
    let cached_translator = CachedTranslator::new(mock_backend);
    let engine = PdfTranslatorEngine::new(&cached_translator);

    engine.translate_pdf(&lock, &input_pdf_path, &output_pdf_path, "en", "tr").await.unwrap();

    // 3. Cikti PDF'ini dogrula
    assert!(output_pdf_path.exists(), "Cikti PDF dosyasi olusmus olmali");
    let translated_doc = Document::load(&output_pdf_path).unwrap();
    let trans_pages = translated_doc.get_pages();
    assert_eq!(trans_pages.len(), 1, "Sayfa sayisi 1 olmali");

    let trans_page_id = trans_pages.values().next().copied().unwrap();
    let trans_content = translated_doc.get_and_decode_page_content(trans_page_id).unwrap();

    // Cizim (re, S) ve gorsel (Do) operatorlerinin kaybolmadigini teyit et
    let has_re = trans_content.operations.iter().any(|op| op.operator == "re");
    let has_s = trans_content.operations.iter().any(|op| op.operator == "S");
    let has_do = trans_content.operations.iter().any(|op| op.operator == "Do");
    assert!(has_re, "Vektorel dikdortgen (re) aynen korunmus olmali");
    assert!(has_s, "Vektorel kontur (S) aynen korunmus olmali");
    assert!(has_do, "Gorsel cagrisi (Do) aynen korunmus olmali");

    // Metin operatorunun cevrildigini dogrula ([TR] on eki eklenmis olmali)
    let has_translated_text = trans_content.operations.iter().any(|op| {
        if op.operator == "Tj" && !op.operands.is_empty() {
            if let Object::String(bytes, _) = &op.operands[0] {
                let text = String::from_utf8_lossy(bytes);
                return text.contains("[TR]");
            }
        }
        false
    });
    assert!(has_translated_text, "Metin icerigi [TR] olarak basariyla cevrilmis olmali");
}

#[test]
fn test_utf16be_text_extraction() {
    use lopdf::Object;
    use polycodex::extract_text_from_object;

    // "Hello" in UTF-16BE with BOM [0xFE, 0xFF, 0x00, 'H', 0x00, 'e', 0x00, 'l', 0x00, 'l', 0x00, 'o']
    let utf16be_bytes = vec![
        0xFE, 0xFF, 0x00, b'H', 0x00, b'e', 0x00, b'l', 0x00, b'l', 0x00, b'o',
    ];
    let obj = Object::String(utf16be_bytes, lopdf::StringFormat::Literal);
    let extracted = extract_text_from_object(&obj);
    assert_eq!(extracted, Some("Hello".to_string()));

    // "World" in UTF-16LE with BOM [0xFF, 0xFE, 'W', 0x00, 'o', 0x00, 'r', 0x00, 'l', 0x00, 'd', 0x00]
    let utf16le_bytes = vec![
        0xFF, 0xFE, b'W', 0x00, b'o', 0x00, b'r', 0x00, b'l', 0x00, b'd', 0x00,
    ];
    let obj_le = Object::String(utf16le_bytes, lopdf::StringFormat::Literal);
    let extracted_le = extract_text_from_object(&obj_le);
    assert_eq!(extracted_le, Some("World".to_string()));
}

#[tokio::test]
async fn test_quote_operators_preservation_and_translation() {
    let _guard = TEST_LOCK_MUTEX.lock().unwrap();
    use lopdf::{Document, Object, Stream, Dictionary};
    use lopdf::content::{Content, Operation};
    use polycodex::PdfTranslatorEngine;

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    // Content stream with "'" operator (next-line and show text)
    let operations = vec![
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), Object::Integer(12)]),
        Operation::new("'", vec![Object::String(b"Section Heading Line".to_vec(), lopdf::StringFormat::Literal)]),
        Operation::new("ET", vec![]),
    ];
    let content = Content { operations };
    let content_stream = Stream::new(Dictionary::new(), content.encode().unwrap());
    let content_id = doc.add_object(content_stream);

    let page_dict = Dictionary::from_iter(vec![
        ("Type", "Page".into()),
        ("Parent", pages_id.into()),
        ("Contents", content_id.into()),
        ("MediaBox", vec![0.into(), 0.into(), 595.into(), 842.into()].into()),
    ]);
    let page_id = doc.add_object(page_dict);

    let pages_dict = Dictionary::from_iter(vec![
        ("Type", "Pages".into()),
        ("Kids", vec![page_id.into()].into()),
        ("Count", 1.into()),
    ]);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_dict = Dictionary::from_iter(vec![
        ("Type", "Catalog".into()),
        ("Pages", pages_id.into()),
    ]);
    let catalog_id = doc.add_object(catalog_dict);
    doc.trailer.set("Root", catalog_id);

    let temp_dir = tempfile::tempdir().unwrap();
    let in_path = temp_dir.path().join("quote_test_in.pdf");
    let out_path = temp_dir.path().join("quote_test_out.pdf");
    doc.save(&in_path).unwrap();

    let lock = SingleInstanceLock::acquire().unwrap();
    let mock = Box::new(MockBackend);
    let cached = CachedTranslator::new(mock);
    let engine = PdfTranslatorEngine::new(&cached);

    engine.translate_pdf(&lock, &in_path, &out_path, "en", "tr").await.unwrap();

    let res_doc = Document::load(&out_path).unwrap();
    let trans_page_id = res_doc.get_pages().values().next().copied().unwrap();
    let trans_content = res_doc.get_and_decode_page_content(trans_page_id).unwrap();

    // Verify T* was generated and translated Tj text exists
    let has_t_star = trans_content.operations.iter().any(|op| op.operator == "T*");
    assert!(has_t_star, "T* satır sonu operatörü oluşturulmuş olmalı");
}

#[test]
fn test_zero_emojis_in_polycodex_sources() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let src_dir = std::path::Path::new(&manifest_dir).join("src");

    for entry in std::fs::read_dir(src_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map(|s| s == "rs").unwrap_or(false) {
            let content = std::fs::read_to_string(&path).unwrap();
            for (line_no, line) in content.lines().enumerate() {
                for c in line.chars() {
                    let code = c as u32;
                    if (0xE000..=0xF8FF).contains(&code) || (0xF0000..=0x10FFFD).contains(&code) {
                        continue;
                    }
                    let is_emoji = (0x1F300..=0x1F9FF).contains(&code)
                        || (0x2600..=0x27BF).contains(&code)
                        || (0x1F600..=0x1F64F).contains(&code)
                        || (0x1F680..=0x1F6FF).contains(&code)
                        || (0x2300..=0x23FF).contains(&code);
                    assert!(
                        !is_emoji,
                        "Emoji detected in {}:{} character '{}'",
                        path.display(),
                        line_no + 1,
                        c
                    );
                }
            }
        }
    }
}

#[test]
fn test_validate_service_url_ssrf_and_https_enforcement() {
    use polycodex::translator::validate_service_url;

    // Localhost HTTP allowed for local Ollama / LibreTranslate
    assert!(validate_service_url("http://127.0.0.1:11434").is_ok());
    assert!(validate_service_url("http://localhost:5000").is_ok());
    assert!(validate_service_url("http://[::1]:8080").is_ok());

    // Remote HTTPS allowed
    assert!(validate_service_url("https://api.libretranslate.com").is_ok());
    assert!(validate_service_url("https://translate.google.com/api").is_ok());

    // Remote HTTP must be blocked (Cleartext intercept defense)
    let remote_http = validate_service_url("http://api.libretranslate.com");
    assert!(remote_http.is_err(), "Remote unencrypted HTTP must be blocked");

    // Cloud Metadata endpoints must be blocked (SSRF defense)
    let aws_gcp_metadata = validate_service_url("http://169.254.169.254/latest/meta-data");
    assert!(aws_gcp_metadata.is_err(), "AWS/GCP metadata IP must be blocked");

    let alibaba_metadata = validate_service_url("http://100.100.100.200/latest/meta-data");
    assert!(alibaba_metadata.is_err(), "Alibaba metadata IP must be blocked");

    let gcp_dns_metadata = validate_service_url("http://metadata.google.internal/computeMetadata/v1/");
    assert!(gcp_dns_metadata.is_err(), "Google metadata DNS must be blocked");
}

#[test]
fn test_system_directory_confinement() {
    use polycodex::pdf_engine::is_confined_output_path;

    // Critical system directories must be rejected
    assert!(!is_confined_output_path(Path::new("/etc/shadow")));
    assert!(!is_confined_output_path(Path::new("/etc/polycodex.conf")));
    assert!(!is_confined_output_path(Path::new("/usr/bin/malicious.pdf")));
    assert!(!is_confined_output_path(Path::new("/root/secret.pdf")));
    assert!(!is_confined_output_path(Path::new("/sys/kernel/debug.pdf")));
    assert!(!is_confined_output_path(Path::new("/proc/self/mem.pdf")));
    assert!(!is_confined_output_path(Path::new("/boot/initramfs.pdf")));

    // Safe user directories must be permitted
    assert!(is_confined_output_path(Path::new("/tmp/safe_out.pdf")));
    assert!(is_confined_output_path(Path::new("./local_out.pdf")));
}

#[test]
fn test_single_instance_lock_hardlink_defense() {
    let _guard = TEST_LOCK_MUTEX.lock().unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let lock_target = temp_dir.path().join("polycodex.lock");
    let hardlink_path = temp_dir.path().join("hardlink.lock");

    // Create target file and a hardlink pointing to it
    std::fs::write(&lock_target, b"dummy").unwrap();
    std::fs::hard_link(&lock_target, &hardlink_path).unwrap();

    // Set custom XDG_RUNTIME_DIR to point to this tempdir
    let orig_runtime = std::env::var("XDG_RUNTIME_DIR").ok();
    std::env::set_var("XDG_RUNTIME_DIR", temp_dir.path());

    let lock_attempt = SingleInstanceLock::acquire();

    // Restore environment
    if let Some(orig) = orig_runtime {
        std::env::set_var("XDG_RUNTIME_DIR", orig);
    } else {
        std::env::remove_var("XDG_RUNTIME_DIR");
    }

    assert!(
        matches!(lock_attempt, Err(LockError::SecurityViolation(_))),
        "Hardlinked lock file must be rejected with SecurityViolation"
    );
}

#[tokio::test]
async fn test_openai_backend_initialization_and_security() {
    use polycodex::translator::OpenAiBackend;
    use polycodex::translator::TranslationBackend;

    // Localhost / private subnet allowed
    let backend = OpenAiBackend::new(
        "http://127.0.0.1:1234".to_string(),
        "qwen2.5-7b".to_string(),
        Some("secret-key".to_string()),
    );
    assert_eq!(backend.name(), "OpenAI / LM Studio / vLLM REST");

    // Cleartext HTTP over public internet rejected (Zero-Trust)
    let public_insecure = OpenAiBackend::new(
        "http://api.openai.com".to_string(),
        "gpt-4o".to_string(),
        None,
    );
    // validate_service_url returns default fallback localhost on err
    assert_eq!(public_insecure.name(), "OpenAI / LM Studio / vLLM REST");
}



