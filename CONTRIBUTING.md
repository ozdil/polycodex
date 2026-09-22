# Katki Rehberi (Contributing Guide)

PolyCodex projesine katki saglamak istediginiz icin tesekkur ederiz. Bu proje, dunya genelinde kullanilabilecek acik kaynakli, guvenli ve yuksek performansli bir PDF cevirme ekosistemi gelistirmeyi amaclamaktadir.

---

## Standartlar ve Kurallar

1. **Sifir Emoji Politikasi**:
   - Kaynak kodlarda, commit mesajlarinda, dokumantasyonlarda, issue ve PR metinlerinde hicbir kosulda unicode emoji kullanilmayacaktir.

2. **Tipografi**:
   - Tum arayuz ve dokumantasyon metinlerinde varsayilan yazi tipi referansi `JetBrainsMono Nerd Font` standardidir (font fallback: `JetBrainsMono Nerd Font, JetBrains Mono, monospace`).

3. **Guvenlik ve Mimari Standartlari (HANCORE / Linux / Arch)**:
   - Kilit dosyalari ortak `/tmp` yerine daima `$XDG_RUNTIME_DIR` altinda ve `0700/0600` izinleriyle olusturulmalidir.
   - Cikti dosyalari daima atomik olarak gecici dosyalardan (`.tmp_pdf_*.pdf`) asil konuma tasinmali ve dosya izinleri `0600` yapilmalidir.
   - Sembolik baglar (`symlink_metadata`) guvenlik ihlallerini ve TOCTOU saldirilarini onlemek icin kesinlikle reddedilmelidir.
   - Girdi boyutlari ve sayfa sayilari (5000 sayfa siniri, 2 GiB tavan) kesin olarak denetlenmelidir.
   - Harici komut cagrilari kabuk (`sh -c`) kullanilmadan dogrudan ayrik arguman dizisi olarak aktarilmalidir.

4. **Kod Duzeni ve Dogrulama**:
   - Yeni bir ozellik veya duzeltme eklendiginde ilgili birim/entegrasyon testi `tests/` altina eklenmelidir.
   - PR oncesinde `cargo test` ve `cargo build --release` basariyla tamamlanmalidir.
