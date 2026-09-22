use std::path::{Path, PathBuf};
use polycodex::lock::{LockError, SingleInstanceLock};
use polycodex::pdf_engine::{OutputPathResolver, MAX_ALLOWED_PAGES};
use polycodex::translator::{CachedTranslator, MockBackend};

#[tokio::test]
async fn test_single_instance_lock_concurrency() {
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
