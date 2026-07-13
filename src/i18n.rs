use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Lang {
    En,
    Ru,
    Ja,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Ru, Lang::Ja];

    pub fn label(&self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
            Lang::Ja => "日本語",
        }
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

// Возвращает локализованную строку по ключу; при отсутствии перевода — пустую строку
pub fn t(lang: Lang, key: &str) -> &'static str {
    match (lang, key) {
        (Lang::Ja, "subtitle") => "DXF図面クリーンアップツール",
        (Lang::En, "subtitle") => "DXF drawing cleanup tool",
        (Lang::Ru, "subtitle") => "Инструмент очистки DXF-чертежей",

        (Lang::Ja, "empty_state") => "「開く」からDXFファイルを読み込んでください",
        (Lang::En, "empty_state") => "Open a DXF file to get started",
        (Lang::Ru, "empty_state") => "Откройте файл DXF, чтобы начать",

        (Lang::Ja, "license_button_tooltip") => "ライセンス情報",
        (Lang::En, "license_button_tooltip") => "License Information",
        (Lang::Ru, "license_button_tooltip") => "Информация о лицензиях",

        (Lang::Ja, "license_title") => "使用ライブラリ・フォントのライセンス",
        (Lang::En, "license_title") => "Library & Font Licenses",
        (Lang::Ru, "license_title") => "Лицензии библиотек и шрифтов",

        (Lang::Ja, "license_libraries") => "使用ライブラリ",
        (Lang::En, "license_libraries") => "Libraries Used",
        (Lang::Ru, "license_libraries") => "Используемые библиотеки",

        (Lang::Ja, "license_fonts") => "使用フォント",
        (Lang::En, "license_fonts") => "Fonts Used",
        (Lang::Ru, "license_fonts") => "Используемые шрифты",

        (Lang::Ja, "license_fonts_body") =>
            "Google Sans と Noto Sans JP は、いずれも SIL Open Font License, Version 1.1（OFL）のもとで Google LLC により提供されています。",
        (Lang::En, "license_fonts_body") =>
            "Google Sans and Noto Sans JP are both provided by Google LLC under the SIL Open Font License, Version 1.1 (OFL).",
        (Lang::Ru, "license_fonts_body") =>
            "Google Sans и Noto Sans JP предоставляются Google LLC по лицензии SIL Open Font License, версия 1.1 (OFL).",

        (Lang::Ja, "license_software") => "本ソフトウェアについて",
        (Lang::En, "license_software") => "About This Software",
        (Lang::Ru, "license_software") => "Об этом программном обеспечении",

        (Lang::Ja, "license_software_body") =>
            "AntiWaste CAD — DXF図面内のほぼ重複した線・円弧やレイヤーを自動検出し、クリック一つで修正できるツールです。\n作者: Rafych\nリポジトリ: https://github.com/Rafych/AntiWaste-CAD",
        (Lang::En, "license_software_body") =>
            "AntiWaste CAD — automatically detects near-duplicate lines, arcs, and layers in DXF drawings, fixable with a single click.\nAuthor: Rafych\nRepository: https://github.com/Rafych/AntiWaste-CAD",
        (Lang::Ru, "license_software_body") =>
            "AntiWaste CAD — автоматически находит почти дублирующиеся линии, дуги и слои в DXF-чертежах, которые можно исправить одним щелчком.\nАвтор: Rafych\nРепозиторий: https://github.com/Rafych/AntiWaste-CAD",

        (Lang::Ja, "open_button") => "開く",
        (Lang::En, "open_button") => "Open",
        (Lang::Ru, "open_button") => "Открыть",

        (Lang::Ja, "save_button") => "保存",
        (Lang::En, "save_button") => "Save",
        (Lang::Ru, "save_button") => "Сохранить",

        (Lang::Ja, "no_file_open") => "ファイルが開かれていません",
        (Lang::En, "no_file_open") => "No file open",
        (Lang::Ru, "no_file_open") => "Файл не открыт",

        (Lang::Ja, "issues_found_prefix") => "検出された修正候補: ",
        (Lang::En, "issues_found_prefix") => "Issues found: ",
        (Lang::Ru, "issues_found_prefix") => "Найдено проблем: ",

        (Lang::Ja, "no_issues") => "修正候補は見つかりませんでした",
        (Lang::En, "no_issues") => "No issues found",
        (Lang::Ru, "no_issues") => "Проблем не найдено",

        (Lang::Ja, "click_marker_hint") => "丸印をクリックすると自動修正されます",
        (Lang::En, "click_marker_hint") => "Click a marker to auto-fix it",
        (Lang::Ru, "click_marker_hint") => "Нажмите на маркер, чтобы исправить автоматически",

        (Lang::Ja, "saved_toast") => "保存しました",
        (Lang::En, "saved_toast") => "Saved",
        (Lang::Ru, "saved_toast") => "Сохранено",

        (Lang::Ja, "load_error_prefix") => "読み込みエラー: ",
        (Lang::En, "load_error_prefix") => "Load error: ",
        (Lang::Ru, "load_error_prefix") => "Ошибка загрузки: ",

        (Lang::Ja, "save_error_prefix") => "保存エラー: ",
        (Lang::En, "save_error_prefix") => "Save error: ",
        (Lang::Ru, "save_error_prefix") => "Ошибка сохранения: ",

        (Lang::Ja, "reset_view_button") => "表示をリセット",
        (Lang::En, "reset_view_button") => "Reset View",
        (Lang::Ru, "reset_view_button") => "Сбросить вид",

        (Lang::Ja, "zoom_hint") => "ホイール/ピンチで拡大縮小・ドラッグで移動・ダブルクリックでリセット",
        (Lang::En, "zoom_hint") => "Scroll or pinch to zoom, drag to pan, double-click to reset",
        (Lang::Ru, "zoom_hint") => "Колесо/щипок — масштаб, перетаскивание — сдвиг, двойной клик — сброс",

        (Lang::Ja, "undo_button") => "◀ 戻る",
        (Lang::En, "undo_button") => "◀ Undo",
        (Lang::Ru, "undo_button") => "◀ Отменить",

        (Lang::Ja, "redo_button") => "進む ▶",
        (Lang::En, "redo_button") => "Redo ▶",
        (Lang::Ru, "redo_button") => "Повторить ▶",

        (Lang::Ja, "undo_tooltip") => "元に戻す（Ctrl+Z）",
        (Lang::En, "undo_tooltip") => "Undo (Ctrl+Z)",
        (Lang::Ru, "undo_tooltip") => "Отменить (Ctrl+Z)",

        (Lang::Ja, "redo_tooltip") => "やり直す（Ctrl+X）",
        (Lang::En, "redo_tooltip") => "Redo (Ctrl+X)",
        (Lang::Ru, "redo_tooltip") => "Повторить (Ctrl+X)",

        (Lang::Ja, "undo_toast") => "元に戻しました",
        (Lang::En, "undo_toast") => "Undone",
        (Lang::Ru, "undo_toast") => "Отменено",

        (Lang::Ja, "redo_toast") => "やり直しました",
        (Lang::En, "redo_toast") => "Redone",
        (Lang::Ru, "redo_toast") => "Повторено",

        (Lang::Ja, "encoding_title") => "文字コードの選択",
        (Lang::En, "encoding_title") => "Select Character Encoding",
        (Lang::Ru, "encoding_title") => "Выбор кодировки",

        (Lang::Ja, "encoding_body") =>
            "ファイルの文字コードを自動判定できませんでした。以下から候補を選んでください。",
        (Lang::En, "encoding_body") =>
            "Couldn't automatically detect the file's character encoding. Please choose one below.",
        (Lang::Ru, "encoding_body") =>
            "Не удалось автоматически определить кодировку файла. Пожалуйста, выберите один из вариантов ниже.",

        (Lang::Ja, "encoding_cancel") => "キャンセル",
        (Lang::En, "encoding_cancel") => "Cancel",
        (Lang::Ru, "encoding_cancel") => "Отмена",

        (Lang::Ja, "encoding_confirm") => "このコードで開く",
        (Lang::En, "encoding_confirm") => "Open with this encoding",
        (Lang::Ru, "encoding_confirm") => "Открыть с этой кодировкой",

        _ => "",
    }
}
