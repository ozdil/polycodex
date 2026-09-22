# PolyCodex

1-1 Mizanpaj, cizim ve gorsel koruyan, 5000 sayfa kapasiteli, tekil islem kilitli, rijit guvenlik mimarisine sahip evrensel acik kaynakli PDF cevirici.

---

## Genel Bakis

PolyCodex, karmaşık akademik makaleleri, teknik kılavuzları, kitapları ve kurumsal yayınları mizanpajını, piksel çözünürlüğündeki görsellerini ve vektörel çizimlerini bozmadan herhangi bir dilden hedef dile çeviren bağımsız bir araçtır. Hem yüksek performanslı Rust komut satırı motoru (`polycodex`) hem de modern, hafif ve reaktif bir QML masaüstü arayüzü (`polycodex-gui`) sunar.

---

## Mimari ve Guvenlik Ozellikleri

### 1. Rijit Guvenlik Mimarisi (Arch Linux / HANCORE)
- **Kullanici Seviyesinde Izole Kilit ($XDG_RUNTIME_DIR)**:
  - Ortak `/tmp` dizini kullanilmaz. Kilit dosyasi systemd tarafindan oturuma ozel olarak tahsis edilen `0700` izinli `/run/user/<UID>/polycodex.lock` dosyasinda tutulur.
  - Kilit yolunda sembolik bag (symlink) tespiti yapildiginda (`O_NOFOLLOW` / `symlink_metadata`) islem aninda durdurulur (TOCTOU ve symlink race saldirilarina karsi tam koruma).
- **Tekil Islem Guvencesi (Single-Instance Lock)**:
  - Isletim sistemi duzeyinde kilit mekanizmasi (`fs2`) ile sistemde ayni anda sadece tek bir dokuman cevrilebilir. Yurutlukte bir islem varken ikinci islem aninda engellenir.
- **Atomik ve Kisitli Dosya Depolama (0600)**:
  - Cikti dosyalari once hedef dizinde `.tmp_pdf_*` gecici dosyasina yazilir, izinleri yalnizca dosya sahibinin erisebilecegi `0600` moduna getirilir ve POSIX atomik degistirme ile hedefe tasinir.
- **Hizmet Reddi (DoS) ve Bellek Sinirlandirmasi**:
  - Girdi dosyalari icin 2 GiB tavan dosya boyutu kontrolu uygulanir.
  - Maksimum 5000 sayfa siniri (`MAX_ALLOWED_PAGES`) ile zip-bomb veya sonsuz sayfa patlamalari onlenir.
- **Komut Enjeksiyonu Savunmasi**:
  - Harici surecler (Zenity, Quickshell, Rust binary) cagrilirken kabuk (`sh -c`) araci kullanilmaz; parametreler dogrudan ayrık arguman dizisi olarak aktarilir.

### 2. 1-1 Mizanpaj ve Gorsel Koruma
- **Dokunulmaz Varliklar**: Sayfa icerisindeki gorsel akislari (`Do`), dikdortgenler (`re`), cizgiler (`m`, `l`, `c`) ve boyamalar (`f`, `S`) aynen korunur.
- **Dinamik Mikrotipografi (`Tz` Operatoru)**: Cevrilen metin blok bazinda olculur. Turkce gibi sondan eklemeli dillerde olusan metin uzamalari punto kucultulmeden `%85 - %95` bandinda yatay karakter sikistirmasi (`Tz`) ile kolon icine hapsedilir.
- **Secici Filtreleme (`should_translate`)**: Matematiksel formuller, denklem degiskenleri ($W, x, r$), sayilar, sayfa numaralari ve kod bloklari cevrilmeden muhafaza edilir.

### 3. Cok Dilli Saglayici Mimarisi
- **Mock Motoru**: Test ve cevrimdisi dogrulama motoru.
- **Ollama Backend**: `Llama 3.1`, `Qwen 2.5`, `Mistral Nemo` veya `DeepSeek` gibi yerel modeller ile tamamen cevrimdisi ve veri gizliligi korumali ceviri.
- **LibreTranslate Backend**: Acik kaynakli self-hosted REST ceviri sunucusu.
- **LM Studio / OpenAI API**: Yerel OpenAI uyumlu REST uclari (`127.0.0.1:1234`).

---

## Kurulum

### Arch Linux (AUR / Pacman)

PKGBUILD uzerinden yerel kurulum:

```bash
cd pdf-translator
makepkg -si
```

Paket su bilesenleri sisteme kurar:
- `/usr/bin/polycodex`: Rust cekirdek CLI
- `/usr/bin/polycodex-gui`: QML masaustu baslatici
- `/usr/share/polycodex/qml/*`: Quickshell arayuz bilesenleri
- `/usr/share/applications/polycodex.desktop`: XDG masaustu kisayolu
- `/usr/share/icons/hicolor/scalable/apps/polycodex.svg`: Vektorel uygulama ikonu
- `/usr/share/licenses/polycodex/*`: MIT ve Apache-2.0 lisanslari

### Kaynaktan Derleme

Gereksinimler:
- Rust (Cargo) 1.75+
- Quickshell ve Zenity (Masaustu arayuzu icin)

```bash
cargo build --release
```

Olusturulan ikili dosya: `target/release/polycodex`

---

## Kullanim

### 1. Masaustu Grafik Arayuzu (GUI)

```bash
polycodex-gui
```

Ozellikler:
- Surukle-birak veya yerel sistem dosya secici (`zenity`) ile girdi belirleme.
- Tum dilleri iceren aciklamali acilir liste (Varsayilan uygulama dili: Ingilizce `en`).
- Tekil kilit rozeti ile sistem cakismalarini anlik bildirme.
- JetBrainsMono Nerd Font tabanli modern koyu tema.

### 2. Komut Satiri (CLI)

```bash
# Interaktif mod (Dosya, dil ve klasor adimlarini sorar)
polycodex

# Otomasyon ve dogrudan cevirim
polycodex --input /yol/belge.pdf --source-lang en --target-lang tr --non-interactive

# Ollama yerel yapay zeka modeli ile cevirim
polycodex --input /yol/belge.pdf --source-lang en --target-lang tr --provider ollama --ollama-model llama3 --non-interactive
```

---

## Testlerin Calistirilmasi

```bash
cargo test
```

Test kapsami:
- Kullaniciya ozel izole tekil kilit ve symlink savunmasi (`test_single_instance_lock_concurrency`).
- Varsayilan dosya adi cozumlemesi (`test_default_output_filename_resolution`).
- Varsayilan cikti klasoru cozumlemesi (`test_default_output_directory_resolution`).
- 5000 sayfa sinir sabiti ve kontrolleri (`test_page_limit_constant`).
- Ceviri onbellegi ve mock calismasi (`test_translator_cache_and_mock_backend`).
- Mizanpaj ve vektor koruma uctan uca testi (`test_pdf_layout_and_vector_preservation_e2e`).

---

## Lisans

Bu proje MIT veya Apache-2.0 cift lisansi altinda yayimlanmaktadir.
Ayrintilar icin `LICENSE-MIT` ve `LICENSE-APACHE` dosyalarina bakiniz.
