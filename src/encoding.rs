use encoding_rs::Encoding as RsEncoding;

use crate::i18n::Lang;

// Поддерживаемые текстовые кодировки для чтения/записи DXF-файлов
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextEncoding {
    Utf8,
    ShiftJis,
    EucJp,
    Gb18030,
    Big5,
    EucKr,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl TextEncoding {
    pub const ALL: [TextEncoding; 9] = [
        TextEncoding::Utf8,
        TextEncoding::ShiftJis,
        TextEncoding::EucJp,
        TextEncoding::Gb18030,
        TextEncoding::Big5,
        TextEncoding::EucKr,
        TextEncoding::Utf16Le,
        TextEncoding::Utf16Be,
        TextEncoding::Windows1252,
    ];

    pub fn label(&self, lang: Lang) -> &'static str {
        match (self, lang) {
            (TextEncoding::Utf8, _) => "UTF-8",

            (TextEncoding::ShiftJis, Lang::Ja) => "Shift_JIS (CP932) — 日本語",
            (TextEncoding::ShiftJis, Lang::En) => "Shift_JIS (CP932) — Japanese",
            (TextEncoding::ShiftJis, Lang::Ru) => "Shift_JIS (CP932) — японский",

            (TextEncoding::EucJp, Lang::Ja) => "EUC-JP — 日本語",
            (TextEncoding::EucJp, Lang::En) => "EUC-JP — Japanese",
            (TextEncoding::EucJp, Lang::Ru) => "EUC-JP — японский",

            (TextEncoding::Gb18030, Lang::Ja) => "GB18030 — 簡体字中国語",
            (TextEncoding::Gb18030, Lang::En) => "GB18030 — Simplified Chinese",
            (TextEncoding::Gb18030, Lang::Ru) => "GB18030 — упрощённый китайский",

            (TextEncoding::Big5, Lang::Ja) => "Big5 — 繁体字中国語",
            (TextEncoding::Big5, Lang::En) => "Big5 — Traditional Chinese",
            (TextEncoding::Big5, Lang::Ru) => "Big5 — традиционный китайский",

            (TextEncoding::EucKr, Lang::Ja) => "EUC-KR — 韓国語",
            (TextEncoding::EucKr, Lang::En) => "EUC-KR — Korean",
            (TextEncoding::EucKr, Lang::Ru) => "EUC-KR — корейский",

            (TextEncoding::Utf16Le, _) => "UTF-16 LE",
            (TextEncoding::Utf16Be, _) => "UTF-16 BE",

            (TextEncoding::Windows1252, Lang::Ja) => "Windows-1252 (Latin-1) — 西欧言語",
            (TextEncoding::Windows1252, Lang::En) => "Windows-1252 (Latin-1) — Western European",
            (TextEncoding::Windows1252, Lang::Ru) => {
                "Windows-1252 (Latin-1) — западноевропейские языки"
            }
        }
    }

    fn rs_encoding(&self) -> &'static RsEncoding {
        match self {
            TextEncoding::Utf8 => encoding_rs::UTF_8,
            TextEncoding::ShiftJis => encoding_rs::SHIFT_JIS,
            TextEncoding::EucJp => encoding_rs::EUC_JP,
            TextEncoding::Gb18030 => encoding_rs::GB18030,
            TextEncoding::Big5 => encoding_rs::BIG5,
            TextEncoding::EucKr => encoding_rs::EUC_KR,
            TextEncoding::Utf16Le => encoding_rs::UTF_16LE,
            TextEncoding::Utf16Be => encoding_rs::UTF_16BE,
            TextEncoding::Windows1252 => encoding_rs::WINDOWS_1252,
        }
    }

    fn from_rs(enc: &'static RsEncoding) -> Option<TextEncoding> {
        TextEncoding::ALL
            .into_iter()
            .find(|e| e.rs_encoding() == enc)
    }

    pub fn decode(&self, bytes: &[u8]) -> (String, bool) {
        let (cow, _, had_errors) = self.rs_encoding().decode(bytes);
        (cow.into_owned(), had_errors)
    }

    pub fn decode_with(&self, bytes: &[u8]) -> String {
        let (decoded, _had_errors) = self.decode(bytes);
        decoded
    }

    pub fn encode(&self, text: &str) -> Vec<u8> {
        match self {
            TextEncoding::Utf16Le => encode_utf16(text, true),
            TextEncoding::Utf16Be => encode_utf16(text, false),
            _ => {
                let (cow, _, _had_errors) = self.rs_encoding().encode(text);
                cow.into_owned()
            }
        }
    }
}

// Результат определения кодировки: либо уверенно определена,
// либо есть несколько кандидатов и нужно спросить пользователя
pub enum EncodingDetection {
    Confident(TextEncoding),

    Ambiguous(Vec<TextEncoding>),
}

pub const SELECTABLE_ENCODINGS: [TextEncoding; 9] = TextEncoding::ALL;

// Пытается определить кодировку файла: сначала по BOM, затем как UTF-8,
// затем эвристически (chardetng); если уверенности нет — возвращает кандидатов
pub fn detect_encoding(bytes: &[u8]) -> EncodingDetection {
    if let Some((enc, bom_len)) = RsEncoding::for_bom(bytes) {
        let (_, _, had_errors) = enc.decode(&bytes[bom_len..]);
        if !had_errors {
            if let Some(text_enc) = TextEncoding::from_rs(enc) {
                return EncodingDetection::Confident(text_enc);
            }
        }
    }

    if std::str::from_utf8(bytes).is_ok() {
        return EncodingDetection::Confident(TextEncoding::Utf8);
    }

    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let guessed = detector.guess(None, true);

    let (_, _, had_errors) = guessed.decode(bytes);
    if !had_errors {
        if let Some(text_enc) = TextEncoding::from_rs(guessed) {
            return EncodingDetection::Confident(text_enc);
        }
    }

    let mut candidates: Vec<TextEncoding> = Vec::with_capacity(SELECTABLE_ENCODINGS.len());
    if let Some(guessed_enc) = TextEncoding::from_rs(guessed) {
        candidates.push(guessed_enc);
    }
    for enc in SELECTABLE_ENCODINGS {
        if !candidates.contains(&enc) {
            candidates.push(enc);
        }
    }

    EncodingDetection::Ambiguous(candidates)
}

// Кодирует строку в UTF-16 (LE или BE) вручную, т.к. encoding_rs это не поддерживает
fn encode_utf16(text: &str, little_endian: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for unit in text.encode_utf16() {
        if little_endian {
            out.extend_from_slice(&unit.to_le_bytes());
        } else {
            out.extend_from_slice(&unit.to_be_bytes());
        }
    }
    out
}
