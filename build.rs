use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// Находит путь к шрифту для языка; при отсутствии — использует общий шрифт-заглушку
fn resolve_font_path(lang_code: &str) -> PathBuf {
    let specific = format!("assets/fonts/{}.ttf", lang_code);
    let general = "assets/fonts/general.ttf";

    let source = if Path::new(&specific).exists() {
        specific
    } else if Path::new(general).exists() {
        println!(
            "cargo:warning=Файл assets/fonts/{}.ttf не найден, вместо него используется assets/fonts/general.ttf.",
            lang_code
        );
        general.to_string()
    } else {
        panic!(
            "Файл шрифта не найден: отсутствуют и assets/fonts/{}.ttf, и assets/fonts/general.ttf.\n\
             Поместите один из этих файлов в assets/fonts/ и повторите сборку.",
            lang_code
        );
    };

    println!("cargo:rerun-if-changed={}", source);

    let canonical = fs::canonicalize(&source).unwrap_or_else(|e| {
        panic!(
            "Не удалось определить абсолютный путь к файлу шрифта ({}): {}",
            source, e
        )
    });

    strip_unc_prefix(canonical)
}

fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

fn path_literal(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "\\\\")
}

// Генерирует Rust-код, встраивающий шрифты в бинарник через include_bytes!
fn generate_font_bindings(out_dir: &str) {
    let langs = ["ja", "ru", "en"];
    let paths: Vec<PathBuf> = langs.iter().map(|l| resolve_font_path(l)).collect();

    let mut unique_paths: Vec<PathBuf> = Vec::new();
    for p in &paths {
        if !unique_paths.contains(p) {
            unique_paths.push(p.clone());
        }
    }

    let mut code = String::new();
    for (i, p) in unique_paths.iter().enumerate() {
        code.push_str(&format!(
            "static GEN_FONT_{}: &[u8] = include_bytes!(\"{}\");\n",
            i,
            path_literal(p)
        ));
    }
    code.push('\n');

    let const_names = ["FONT_JA", "FONT_RU", "FONT_EN"];
    for (lang_path, const_name) in paths.iter().zip(const_names.iter()) {
        let idx = unique_paths.iter().position(|p| p == lang_path).unwrap();
        code.push_str(&format!(
            "pub static {}: &[u8] = GEN_FONT_{};\n",
            const_name, idx
        ));
    }

    let dest = Path::new(out_dir).join("fonts_generated.rs");
    fs::write(&dest, code)
        .unwrap_or_else(|e| panic!("Не удалось записать fonts_generated.rs: {}", e));
}

// Готовит иконку приложения (или прозрачную заглушку 1x1, если её нет)
fn resolve_icon(out_dir: &str) {
    let icon_src = "assets/icon.png";
    let icon_dest = Path::new(out_dir).join("icon.png");
    let flag_dest = Path::new(out_dir).join("icon_info.rs");

    if Path::new(icon_src).exists() {
        fs::copy(icon_src, &icon_dest)
            .unwrap_or_else(|e| panic!("Не удалось скопировать icon.png: {}", e));
        fs::write(&flag_dest, "pub const HAS_ICON: bool = true;\n")
            .unwrap_or_else(|e| panic!("Не удалось записать icon_info.rs: {}", e));
        println!("cargo:rerun-if-changed={}", icon_src);
    } else {
        const DUMMY_PNG_1X1_TRANSPARENT: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        fs::write(&icon_dest, DUMMY_PNG_1X1_TRANSPARENT)
            .unwrap_or_else(|e| panic!("Не удалось записать заглушку icon.png: {}", e));
        fs::write(&flag_dest, "pub const HAS_ICON: bool = false;\n")
            .unwrap_or_else(|e| panic!("Не удалось записать icon_info.rs: {}", e));
        println!(
            "cargo:warning=Файл assets/icon.png не найден, будет использована иконка окна по умолчанию."
        );
    }
}

#[cfg(windows)]
// Встраивает иконку .exe для Windows (на других платформах — пустая функция ниже)
fn embed_windows_icon() {
    let ico_path = "assets/icon.ico";
    if Path::new(ico_path).exists() {
        let mut res = winres::WindowsResource::new();
        res.set_icon(ico_path);
        if let Err(e) = res.compile() {
            panic!("Не удалось встроить иконку Windows ({}): {}", ico_path, e);
        }
        println!("cargo:rerun-if-changed={}", ico_path);
    } else {
        println!("cargo:warning=Файл assets/icon.ico не найден, у .exe будет иконка по умолчанию.");
    }
}

#[cfg(not(windows))]
fn embed_windows_icon() {}

// Скрипт сборки: подготавливает шрифты и иконку до компиляции основного кода
fn main() {
    let out_dir = env::var("OUT_DIR").expect("Переменная окружения OUT_DIR не задана");

    generate_font_bindings(&out_dir);

    println!("cargo:rerun-if-changed=assets/fonts/general.ttf");
    println!("cargo:rerun-if-changed=assets/fonts");

    resolve_icon(&out_dir);

    embed_windows_icon();
}
