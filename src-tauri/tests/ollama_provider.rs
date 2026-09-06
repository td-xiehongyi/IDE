use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use ai_file_organizer_lib::ai::ollama::OllamaProvider;
use ai_file_organizer_lib::ai::{AiProvider, ProviderAnalysisRequest};
use ai_file_organizer_lib::models::ai::Category;

fn serve_once(response_body: &'static str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + length {
                    break;
                }
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8(bytes).unwrap()
    });
    (format!("http://{address}"), handle)
}

fn request() -> ProviderAnalysisRequest {
    ProviderAnalysisRequest {
        filename: "notes.md".into(),
        language: Some("Markdown".into()),
        text: "项目会议纪要".into(),
        categories: vec![Category {
            id: "work".into(),
            name: "工作".into(),
            description: "工作资料".into(),
            directory_path: r"C:\organized\work".into(),
            enabled: true,
        }],
    }
}

#[test]
fn health_check_reports_the_configured_model_as_available() {
    let (endpoint, server) = serve_once(r#"{"models":[{"name":"qwen2.5:7b"}]}"#);
    let provider = OllamaProvider::new(endpoint, "qwen2.5:7b".into()).unwrap();

    let status = provider.health().unwrap();

    assert!(status.available);
    assert_eq!(status.provider, "ollama");
    assert_eq!(status.model, "qwen2.5:7b");
    assert!(server.join().unwrap().starts_with("GET /api/tags"));
}

#[test]
fn analysis_sends_a_json_schema_and_parses_the_closed_result() {
    let content = r#"{\"summary\":\"会议纪要\",\"keywords\":[\"项目\"],\"suggested_filename\":\"项目会议.md\",\"category_id\":\"work\",\"confidence\":0.9,\"reason\":\"属于工作资料\"}"#;
    let body = format!(r#"{{"message":{{"content":"{content}"}}}}"#);
    let leaked: &'static str = Box::leak(body.into_boxed_str());
    let (endpoint, server) = serve_once(leaked);
    let provider = OllamaProvider::new(endpoint, "qwen2.5:7b".into()).unwrap();

    let result = provider.analyze(&request()).unwrap();

    assert_eq!(result.category_id.as_deref(), Some("work"));
    let wire_request = server.join().unwrap();
    assert!(wire_request.starts_with("POST /api/chat"));
    assert!(wire_request.contains("\"format\":{\"additionalProperties\":false"));
    assert!(wire_request.contains("\"temperature\":0.1"));
}

#[test]
fn analysis_rejects_model_output_with_extra_fields() {
    let content = r#"{\"summary\":\"会议纪要\",\"keywords\":[\"项目\"],\"suggested_filename\":\"项目会议.md\",\"category_id\":null,\"confidence\":0.9,\"reason\":\"工作资料\",\"path\":\"C:\\\\escape\"}"#;
    let body = format!(r#"{{"message":{{"content":"{content}"}}}}"#);
    let leaked: &'static str = Box::leak(body.into_boxed_str());
    let (endpoint, server) = serve_once(leaked);
    let provider = OllamaProvider::new(endpoint, "qwen2.5:7b".into()).unwrap();

    assert!(provider.analyze(&request()).unwrap_err().contains("结构"));
    server.join().unwrap();
}

#[test]
fn provider_configuration_rejects_non_loopback_endpoints() {
    assert!(OllamaProvider::new("http://192.0.2.10:11434".into(), "qwen2.5:7b".into()).is_err());
}
