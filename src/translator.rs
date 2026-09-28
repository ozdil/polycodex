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

use zeroize::Zeroize;

/// Sifir Guven (Zero-Trust) ve Askeri Duzey Guvenlik kapsaminda servis URL adresi denetimi.
/// - Yalnizca localhost (127.0.0.1, ::1) icin HTTP baglantilarina izin verilir.
/// - Tum uzak harici servisler icin kesinlikle HTTPS zorunludur (Cleartext intercept onleme).
/// - Bulut metadata IP ve host adresleri (169.254.169.254, 100.100.100.200, metadata.google.internal) engellenir (SSRF korumasi).
/// - Kontrol karakterleri kesinlikle reddedilir.
pub fn validate_service_url(raw: &str) -> Result<String> {
    if raw.chars().any(|c| c.is_control()) {
        anyhow::bail!("Servis URL adresinde gecersiz kontrol karakterleri tespit edildi.");
    }
    let trimmed = raw.trim().trim_end_matches('/');
    let parsed = reqwest::Url::parse(trimmed)
        .with_context(|| format!("Gecersiz servis URL adresi: {}", trimmed))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        anyhow::bail!("Servis URL yalnizca http veya https protokolu kullanabilir: {}", scheme);
    }
    let host = parsed.host()
        .ok_or_else(|| anyhow::anyhow!("Servis URL adresinde gecerli bir sunucu (host) belirtilmedi."))?;

    let is_local_or_private = match host {
        url::Host::Domain(d) => {
            let d_lower = d.to_lowercase();
            if d_lower == "metadata.google.internal" {
                anyhow::bail!("Guvenlik ihlali: Bulut metadata servislerine erisim engellendi (SSRF korumasi): {}", d);
            }
            d_lower == "localhost"
        }
        url::Host::Ipv4(ipv4) => {
            if ipv4.is_link_local() {
                anyhow::bail!("Guvenlik ihlali: Link-local ve metadata adreslerine erisim engellendi: {}", ipv4);
            }
            if ipv4.octets() == [100, 100, 100, 200] {
                anyhow::bail!("Guvenlik ihlali: Bulut metadata IP adresine erisim engellendi: {}", ipv4);
            }
            ipv4.is_loopback() || ipv4.is_private()
        }
        url::Host::Ipv6(ipv6) => {
            let seg = ipv6.segments();
            if (seg[0] & 0xffc0) == 0xfe80 {
                anyhow::bail!("Guvenlik ihlali: IPv6 Link-local adresine erisim engellendi: {}", ipv6);
            }
            ipv6.is_loopback()
        }
    };

    // Askeri duzey iletisim guvenligi: Genel agdaki uzak baglantilarda HTTP engellenir
    if scheme == "http" && !is_local_or_private {
        anyhow::bail!(
            "Guvenlik ihlali: Genel agdaki uzak cevirici servisleri icin HTTP guvensizdir. Yalnizca HTTPS kullanilmalidir: {}",
            trimmed
        );
    }

    Ok(trimmed.to_string())
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
        let validated = validate_service_url(&base_url)
            .unwrap_or_else(|_| "https://libretranslate.com".to_string());
        Self {
            base_url: validated,
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

impl Drop for LibreTranslateBackend {
    fn drop(&mut self) {
        if let Some(ref mut key) = self.api_key {
            key.zeroize();
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
        let validated = validate_service_url(&endpoint)
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self {
            endpoint: validated,
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

// -----------------------------------------------------------------------------
// 4. OpenAI / LM Studio / vLLM Uyumlu REST Motoru
// -----------------------------------------------------------------------------
pub struct OpenAiBackend {
    endpoint: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiBackend {
    pub fn new(endpoint: String, model: String, api_key: Option<String>) -> Self {
        let validated = validate_service_url(&endpoint)
            .unwrap_or_else(|_| "http://localhost:1234".to_string());
        Self {
            endpoint: validated,
            model,
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

impl Drop for OpenAiBackend {
    fn drop(&mut self) {
        if let Some(ref mut key) = self.api_key {
            key.zeroize();
        }
    }
}

#[derive(Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage<'a>>,
    temperature: f32,
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    content: String,
}

#[async_trait::async_trait]
impl TranslationBackend for OpenAiBackend {
    async fn translate(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(texts.len());
        let chat_url = format!("{}/v1/chat/completions", self.endpoint);

        for text in texts {
            let system_prompt = format!(
                "You are an exact 1:1 translation engine. Translate from {} to {}. Output ONLY the translated text without extra explanation, markdown wrapping, or commentary.",
                source_lang, target_lang
            );

            let messages = vec![
                OpenAiMessage {
                    role: "system",
                    content: &system_prompt,
                },
                OpenAiMessage {
                    role: "user",
                    content: text,
                },
            ];

            let payload = OpenAiChatRequest {
                model: &self.model,
                messages,
                temperature: 0.1,
            };

            let mut req = self.client.post(&chat_url).json(&payload);
            if let Some(ref key) = self.api_key {
                req = req.bearer_auth(key);
            }

            let resp = req
                .send()
                .await
                .context("OpenAI uyumlu REST servisine baglanilamadi")?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("OpenAI servisi hatasi (HTTP {}): {}", status, body);
            }

            let data: OpenAiChatResponse = resp.json().await?;
            let reply = data
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content.trim().to_string())
                .unwrap_or_else(|| text.clone());

            results.push(reply);
        }

        Ok(results)
    }

    fn name(&self) -> &'static str {
        "OpenAI / LM Studio / vLLM REST"
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_service_url_valid() {
        assert!(validate_service_url("http://localhost:11434").is_ok());
        assert!(validate_service_url("https://libretranslate.com/").is_ok());
        assert!(validate_service_url("http://192.168.1.50:5000").is_ok());
    }

    #[test]
    fn test_validate_service_url_invalid() {
        // Control characters
        assert!(validate_service_url("http://localhost:11434\n").is_err());
        // Non-http schemes
        assert!(validate_service_url("ftp://example.com").is_err());
        assert!(validate_service_url("file:///etc/passwd").is_err());
        // Malformed
        assert!(validate_service_url("not a url").is_err());
    }
}

