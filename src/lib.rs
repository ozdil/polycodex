pub mod cli;
pub mod interactive;
pub mod lock;
pub mod pdf_engine;
pub mod translator;

pub use lock::{LockError, SingleInstanceLock};
pub use pdf_engine::{PdfTranslatorEngine, OutputPathResolver, MAX_ALLOWED_PAGES, extract_text_from_object};
pub use translator::{CachedTranslator, TranslationBackend, MockBackend, LibreTranslateBackend, OllamaBackend, OpenAiBackend, validate_service_url};
