use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Cok dilli ceviri saglayici arayuzu (Trait).
#[async_trait::async_trait]
pub trait TranslationBackend: Send + Sync {
    /// Verilen metin dizisini kaynak dilden hedef dile cevirir.
    async fn translate(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>>;

    /// Motor adini dondurur.
    fn name(&self) -> &'static str;
}

/// Onbellek sarmalayicisi (Translation Cache).
/// Yinelenen basliklar, altbilgiler ve kaliplar icin gereksiz ceviri cagrisi yapilmasini onler.
pub struct CachedTranslator {
    backend: Box<dyn TranslationBackend>,
    cache: Arc<RwLock<HashMap<String, String>>>,
}

impl CachedTranslator {
    pub fn new(backend: Box<dyn TranslationBackend>) -> Self {
        Self {
            backend,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn translate_block(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(text.to_string());
        }

        let cache_key = format!("{}:{}:{}", source_lang, target_lang, trimmed);
        {
            let cache_read = self.cache.read().unwrap();
            if let Some(cached) = cache_read.get(&cache_key) {
                return Ok(cached.clone());
            }
        }

        let result = self
            .backend
            .translate(&[trimmed.to_string()], source_lang, target_lang)
            .await?;

        let translated = result
            .into_iter()
            .next()
            .unwrap_or_else(|| trimmed.to_string());

        {
            let mut cache_write = self.cache.write().unwrap();
            cache_write.insert(cache_key, translated.clone());
        }

        Ok(translated)
    }

    pub async fn translate_batch(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(texts.len());
        let mut missing_indices = Vec::new();
        let mut missing_texts = Vec::new();

        {
            let cache_read = self.cache.read().unwrap();
            for (idx, text) in texts.iter().enumerate() {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    results.push(text.clone());
                    continue;
                }

                let cache_key = format!("{}:{}:{}", source_lang, target_lang, trimmed);
                if let Some(cached) = cache_read.get(&cache_key) {
                    results.push(cached.clone());
                } else {
                    results.push(String::new()); // Yer tutucu
                    missing_indices.push(idx);
                    missing_texts.push(trimmed.to_string());
                }
            }
        }

        if !missing_texts.is_empty() {
            let fetched = self
                .backend
                .translate(&missing_texts, source_lang, target_lang)
                .await?;

            let mut cache_write = self.cache.write().unwrap();
            for (idx, translated) in missing_indices.into_iter().zip(fetched.into_iter()) {
                let original = texts[idx].trim();
                let cache_key = format!("{}:{}:{}", source_lang, target_lang, original);
                cache_write.insert(cache_key, translated.clone());
                results[idx] = translated;
            }
        }

        Ok(results)
    }

    pub fn name(&self) -> &'static str {
        self.backend.name()
    }
}

// -----------------------------------------------------------------------------
// 1. Mock Ceviri Motoru (Test ve Simulasyon Icin)
// -----------------------------------------------------------------------------
pub struct MockBackend;

#[async_trait::async_trait]
impl TranslationBackend for MockBackend {
    async fn translate(
        &self,
        texts: &[String],
        _source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>> {
        if target_lang != "tr" {
            return Ok(texts.iter().map(|t| format!("[{}] {}", target_lang.to_uppercase(), t)).collect());
        }

        // Turkce mizanpaj koruyucu sozluk haritasi (Akademik ve teknik metinler)
        let mut dict = HashMap::new();
        dict.insert("Abstract", "Özet");
        dict.insert("ABSTRACT", "ÖZET");
        dict.insert("Introduction", "Giriş");
        dict.insert("INTRODUCTION", "GİRİŞ");
        dict.insert("Background", "Arka Plan");
        dict.insert("Method", "Yöntem");
        dict.insert("Methodology", "Metodoloji");
        dict.insert("Results", "Sonuçlar");
        dict.insert("RESULTS", "SONUÇLAR");
        dict.insert("Discussion", "Tartışma");
        dict.insert("Conclusion", "Sonuç");
        dict.insert("Figure", "Şekil");
        dict.insert("Table", "Tablo");
        dict.insert("Section", "Bölüm");
        dict.insert("often", "sıklıkla");
        dict.insert("introduce", "getirir");
        dict.insert("inference", "çıkarım");
        dict.insert("latency", "gecikme");
        dict.insert("model", "model");
        dict.insert("models", "modeller");
        dict.insert("training", "eğitim");
        dict.insert("efficient", "verimli");
        dict.insert("adaptation", "uyarlama");
        dict.insert("large", "büyük");
        dict.insert("language", "dil");
        dict.insert("parameters", "parametreler");
        dict.insert("layers", "katmanlar");
        dict.insert("approach", "yaklaşım");
        dict.insert("Problem", "Problem");
        dict.insert("Statement", "Tanımı");
        dict.insert("PROBLEM", "PROBLEM");
        dict.insert("STATEMENT", "TANIMI");
        dict.insert("Overview", "Genel Bakış");
        dict.insert("References", "Kaynaklar");
        dict.insert("Appendix", "Ek");

        let translated = texts
            .iter()
            .map(|text| {
                let mut out = text.clone();
                for (en, tr) in &dict {
                    out = out.replace(en, tr);
                }
                format!("[{}] {}", target_lang.to_uppercase(), out)
            })
            .collect();

        Ok(translated)
    }

    fn name(&self) -> &'static str {
        "Mock (Simulasyon) Motoru"
    }
}

// -----------------------------------------------------------------------------
// 2. LibreTranslate Motoru (Acik Kaynak REST API)
// -----------------------------------------------------------------------------
pub struct LibreTranslateBackend {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl LibreTranslateBackend {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct LibreTranslateRequest<'a> {
    q: &'a [String],
    source: &'a str,
    target: &'a str,
    format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<&'a str>,
}

#[derive(Deserialize)]
struct LibreTranslateResponse {
    #[serde(rename = "translatedText")]
    translated_text: Option<serde_json::Value>,
}

#[async_trait::async_trait]
impl TranslationBackend for LibreTranslateBackend {
    async fn translate(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let endpoint = format!("{}/translate", self.base_url);
        let request_payload = LibreTranslateRequest {
            q: texts,
            source: if source_lang == "auto" { "auto" } else { source_lang },
            target: target_lang,
            format: "text",
            api_key: self.api_key.as_deref(),
        };

        let response = self
            .client
            .post(&endpoint)
            .json(&request_payload)
            .send()
            .await
            .context("LibreTranslate servisine baglanilamadi")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("LibreTranslate hatasi (HTTP {}): {}", status, body);
        }

        let body: LibreTranslateResponse = response
            .json()
            .await
            .context("LibreTranslate yaniti cozumlenemedi")?;

        match body.translated_text {
            Some(serde_json::Value::Array(arr)) => Ok(arr
                .into_iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()),
            Some(serde_json::Value::String(s)) => Ok(vec![s]),
            _ => Ok(texts.to_vec()),
        }
    }

    fn name(&self) -> &'static str {
        "LibreTranslate (Acik Kaynak REST)"
    }
}

// -----------------------------------------------------------------------------
// 3. Ollama / Yerel Acik Kaynak LLM Motoru
// -----------------------------------------------------------------------------
pub struct OllamaBackend {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaBackend {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[async_trait::async_trait]
impl TranslationBackend for OllamaBackend {
    async fn translate(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(texts.len());
        let generate_url = format!("{}/api/generate", self.endpoint);

        for text in texts {
            let prompt = format!(
                "You are an exact 1:1 translator preserving formatting and length. Translate the following text from {} to {}. Output ONLY the translated text without explanations or quotes:\n\n{}",
                source_lang, target_lang, text
            );

            let payload = OllamaGenerateRequest {
                model: &self.model,
                prompt,
                stream: false,
            };

            let resp = self
                .client
                .post(&generate_url)
                .json(&payload)
                .send()
                .await
                .context("Ollama yerel servisine baglanilamadi")?;

            if !resp.status().is_success() {
                anyhow::bail!("Ollama servis hatasi: {}", resp.status());
            }

            let data: OllamaGenerateResponse = resp.json().await?;
            results.push(data.response.trim().to_string());
        }

        Ok(results)
    }

    fn name(&self) -> &'static str {
        "Ollama (Yerel LLM Acik Kaynak)"
    }
}
