use std::path::PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "polycodex",
    version = "0.1.0",
    about = "Mizanpaj ve gorselleri 1-1 koruyan, 5000 sayfa kapasiteli cok dilli PDF cevirici",
    long_about = "PDF belgelerini mizanpaj, cizim ve resimlerini aynen muhafaza ederek herhangi bir dilden hedef dile ceviren acik kaynakli evrensel cevirici."
)]
pub struct CliArgs {
    /// Cevirilecek girdi PDF dosyasi
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Kaynak dil kodu (ornek: en, de, fr veya otomatik tespit icin 'auto')
    #[arg(short, long)]
    pub source_lang: Option<String>,

    /// Hedef dil kodu (ornek: tr, en, de, es, ja, zh, ar)
    #[arg(short, long)]
    pub target_lang: Option<String>,

    /// Cikti dosya adi (Belirtilmezse: {orijinal_ad}_{hedef_dil}.pdf)
    #[arg(short, long)]
    pub output_name: Option<String>,

    /// Cikarilacak dosya yolu/klasoru (Belirtilmezse girdi dosyasi ile ayni klasor)
    #[arg(short = 'd', long)]
    pub output_dir: Option<PathBuf>,

    /// Ceviri motoru saglayicisi: mock, libretranslate, ollama
    #[arg(short = 'p', long, default_value = "mock")]
    pub provider: String,

    /// Ceviri servisi REST API adresi (LibreTranslate veya Ollama icin)
    #[arg(long)]
    pub api_url: Option<String>,

    /// Ceviri servisi API anahtari (istege bagli)
    #[arg(long)]
    pub api_key: Option<String>,

    /// Ollama modeli adi (ornek: llama3, mistral)
    #[arg(long, default_value = "llama3")]
    pub ollama_model: String,

    /// Kullaniciya soru sormadan dogrudan calistir (otomasyon modunda)
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
}
