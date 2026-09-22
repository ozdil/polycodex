use lopdf::{Document, Object, Stream, Dictionary};
use lopdf::content::{Content, Operation};

fn main() {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let operations = vec![
        // Vektorel cizim (re, S)
        Operation::new("re", vec![Object::Integer(50), Object::Integer(50), Object::Integer(500), Object::Integer(700)]),
        Operation::new("S", vec![]),
        // Metin Basligi
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), Object::Integer(20)]),
        Operation::new("Td", vec![Object::Integer(70), Object::Integer(720)]),
        Operation::new("Tj", vec![Object::String(b"PolyCodex Universal Book & PDF Translator".to_vec(), lopdf::StringFormat::Literal)]),
        Operation::new("ET", vec![]),
        // Metin Paragrafi
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), Object::Integer(12)]),
        Operation::new("Td", vec![Object::Integer(70), Object::Integer(680)]),
        Operation::new("Tj", vec![Object::String(b"This document tests layout preservation, single-instance lock, and streaming up to 5000 pages.".to_vec(), lopdf::StringFormat::Literal)]),
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

    doc.save("sample_test.pdf").unwrap();
    println!("Standartlara uygun sample_test.pdf basariyla olusturuldu!");
}
