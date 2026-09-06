use std::fs;

use ai_file_organizer_lib::models::ai::{AiSuggestionPayload, Category};
use ai_file_organizer_lib::services::content_chunker::chunk_text;
use ai_file_organizer_lib::services::content_extractor::{
    ExtractionLimits, document_language, extract_document, is_supported_text_document,
};
use ai_file_organizer_lib::services::suggestion_validator::validate_suggestion;
use std::io::Write;

fn temp_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ai-file-organizer-ai-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn extracts_utf8_text_and_computes_a_stable_content_fingerprint() {
    let root = temp_dir("utf8");
    let file = root.join("hello.txt");
    fs::write(&file, b"hello").unwrap();

    let extracted = extract_document(&root, &file, ExtractionLimits::default()).unwrap();

    assert_eq!(extracted.text, "hello");
    assert_eq!(
        extracted.content_fingerprint,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn extracts_bom_marked_utf16_and_rejects_non_utf_text() {
    let root = temp_dir("encoding");
    let utf16 = root.join("utf16.md");
    fs::write(&utf16, [0xff, 0xfe, 0x60, 0x4f, 0x7d, 0x59]).unwrap();
    assert_eq!(
        extract_document(&root, &utf16, ExtractionLimits::default())
            .unwrap()
            .text,
        "你好"
    );

    let invalid = root.join("invalid.txt");
    fs::write(&invalid, [0x81, 0x81, 0x81]).unwrap();
    assert!(
        extract_document(&root, &invalid, ExtractionLimits::default())
            .unwrap_err()
            .contains("UTF")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recognizes_common_code_config_extensions_and_special_filenames() {
    let supported = [
        "main.c",
        "main.cpp",
        "header.hpp",
        "Program.cs",
        "Main.java",
        "app.kt",
        "server.go",
        "lib.rs",
        "script.py",
        "app.tsx",
        "index.html",
        "style.scss",
        "query.sql",
        "config.yaml",
        "settings.toml",
        "data.json",
        "Dockerfile",
        "Makefile",
        "CMakeLists.txt",
    ];
    for name in supported {
        assert!(
            is_supported_text_document(std::path::Path::new(name)),
            "expected {name} to be supported"
        );
    }

    for name in ["legacy.doc", "archive.zip", "photo.png"] {
        assert!(!is_supported_text_document(std::path::Path::new(name)));
    }
}

#[test]
fn maps_code_files_to_language_labels() {
    assert_eq!(
        document_language(std::path::Path::new("main.cpp")),
        Some("C++")
    );
    assert_eq!(
        document_language(std::path::Path::new("Main.java")),
        Some("Java")
    );
    assert_eq!(
        document_language(std::path::Path::new("Dockerfile")),
        Some("Dockerfile")
    );
    assert_eq!(
        document_language(std::path::Path::new("notes.md")),
        Some("Markdown")
    );
    assert_eq!(document_language(std::path::Path::new("legacy.doc")), None);
}

#[test]
fn extracts_code_as_utf8_text() {
    let root = temp_dir("code");
    let file = root.join("main.cpp");
    fs::write(&file, "#include <iostream>\nint main() { return 0; }\n").unwrap();

    let extracted = extract_document(&root, &file, ExtractionLimits::default()).unwrap();
    assert!(extracted.text.contains("int main()"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn extracts_docx_paragraph_text_without_persisting_the_archive_contents() {
    let root = temp_dir("docx");
    let file = root.join("meeting.docx");
    let output = fs::File::create(&file).unwrap();
    let mut archive = zip::ZipWriter::new(output);
    archive
        .start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    archive
        .write_all(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>项目会议</w:t></w:r><w:r><w:t>纪要</w:t></w:r></w:p></w:body>
</w:document>"#
                .as_bytes(),
        )
        .unwrap();
    archive.finish().unwrap();

    let extracted = extract_document(&root, &file, ExtractionLimits::default()).unwrap();
    assert_eq!(extracted.text, "项目会议纪要");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn extracts_text_pdf_and_rejects_a_pdf_without_extractable_text() {
    let root = temp_dir("pdf");
    let text_pdf = root.join("text.pdf");
    fs::write(&text_pdf, minimal_pdf(Some("Hello PDF"))).unwrap();
    assert!(
        extract_document(&root, &text_pdf, ExtractionLimits::default())
            .unwrap()
            .text
            .contains("Hello PDF")
    );

    let scanned = root.join("scanned.pdf");
    fs::write(&scanned, minimal_pdf(None)).unwrap();
    assert!(
        extract_document(&root, &scanned, ExtractionLimits::default())
            .unwrap_err()
            .contains("正文为空")
    );
    fs::remove_dir_all(root).unwrap();
}

fn minimal_pdf(text: Option<&str>) -> Vec<u8> {
    let content = text
        .map(|value| format!("BT /F1 12 Tf 72 720 Td ({value}) Tj ET"))
        .unwrap_or_default();
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

#[test]
fn rejects_empty_and_over_limit_documents_before_provider_use() {
    let root = temp_dir("limits");
    let empty = root.join("empty.txt");
    fs::write(&empty, "  \n\t").unwrap();
    assert!(
        extract_document(&root, &empty, ExtractionLimits::default())
            .unwrap_err()
            .contains("正文为空")
    );

    let large = root.join("large.md");
    fs::write(&large, "一二三四五六").unwrap();
    assert!(
        extract_document(&root, &large, ExtractionLimits { max_characters: 5 })
            .unwrap_err()
            .contains("资源上限")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn chunks_unicode_text_on_character_boundaries_with_overlap() {
    let chunks = chunk_text("一二三四五六七八九十", 4, 1).unwrap();
    assert_eq!(chunks, vec!["一二三四", "四五六七", "七八九十"]);
    assert!(chunk_text("text", 4, 4).is_err());
}

fn category() -> Category {
    Category {
        id: "work".into(),
        name: "工作".into(),
        description: "工作资料".into(),
        directory_path: r"C:\organized\work".into(),
        enabled: true,
    }
}

fn valid_payload() -> AiSuggestionPayload {
    AiSuggestionPayload {
        summary: "项目会议纪要".into(),
        keywords: vec!["项目".into(), "会议".into()],
        suggested_filename: "项目会议纪要.md".into(),
        category_id: Some("work".into()),
        confidence: 0.92,
        reason: "内容属于工作会议记录".into(),
    }
}

#[test]
fn validates_closed_suggestions_against_filename_and_category_policy() {
    let source = std::path::Path::new(r"C:\inbox\notes.md");
    let valid = validate_suggestion(source, valid_payload(), &[category()]).unwrap();
    assert_eq!(valid.category_id.as_deref(), Some("work"));

    let mut unknown = valid_payload();
    unknown.category_id = Some("missing".into());
    assert!(validate_suggestion(source, unknown, &[category()]).is_err());

    let mut path = valid_payload();
    path.suggested_filename = r"..\escape.md".into();
    assert!(validate_suggestion(source, path, &[category()]).is_err());

    let mut extension = valid_payload();
    extension.suggested_filename = "notes.txt".into();
    assert!(validate_suggestion(source, extension, &[category()]).is_err());
}

#[test]
fn rejects_empty_fields_duplicate_keywords_and_out_of_range_confidence() {
    let source = std::path::Path::new(r"C:\inbox\notes.md");
    let mut payload = valid_payload();
    payload.summary = " ".into();
    assert!(validate_suggestion(source, payload, &[category()]).is_err());

    let mut payload = valid_payload();
    payload.keywords = vec!["项目".into(), "项目".into()];
    assert!(validate_suggestion(source, payload, &[category()]).is_err());

    let mut payload = valid_payload();
    payload.confidence = 1.01;
    assert!(validate_suggestion(source, payload, &[category()]).is_err());
}
