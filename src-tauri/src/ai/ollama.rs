use std::net::IpAddr;
use std::time::Duration;

use serde_json::json;

use crate::ai::{AiProvider, PROMPT_VERSION, ProviderAnalysisRequest, ProviderStatus};
use crate::models::ai::AiSuggestionPayload;

pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_MODEL: &str = "qwen2.5:7b";

pub struct OllamaProvider {
    endpoint: reqwest::Url,
    model: String,
    client: reqwest::blocking::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: String, model: String) -> Result<Self, String> {
        let endpoint =
            reqwest::Url::parse(&endpoint).map_err(|error| format!("Ollama 地址无效：{error}"))?;
        if endpoint.scheme() != "http" || !is_loopback_host(endpoint.host_str()) {
            return Err("首版 Ollama Provider 只允许本机环回 HTTP 地址".into());
        }
        if model.trim().is_empty() {
            return Err("模型名称不能为空".into());
        }
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("无法初始化 Ollama 客户端：{error}"))?;
        Ok(Self {
            endpoint,
            model,
            client,
        })
    }

    fn url(&self, path: &str) -> Result<reqwest::Url, String> {
        self.endpoint
            .join(path)
            .map_err(|error| format!("无法构造 Ollama 请求地址：{error}"))
    }
}

impl AiProvider for OllamaProvider {
    fn provider_id(&self) -> &'static str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn health(&self) -> Result<ProviderStatus, String> {
        let response = self
            .client
            .get(self.url("/api/tags")?)
            .send()
            .map_err(|error| format!("无法连接本地 Ollama：{error}"))?;
        if !response.status().is_success() {
            return Err(format!("Ollama 健康检查失败：HTTP {}", response.status()));
        }
        let body: serde_json::Value = response
            .json()
            .map_err(|error| format!("Ollama 健康检查响应无效：{error}"))?;
        let available = body["models"].as_array().is_some_and(|models| {
            models
                .iter()
                .any(|model| model["name"].as_str() == Some(&self.model))
        });
        Ok(ProviderStatus {
            available,
            provider: self.provider_id().into(),
            model: self.model.clone(),
            message: if available {
                "模型已就绪".into()
            } else {
                "Ollama 可用，但未找到配置的模型".into()
            },
        })
    }

    fn analyze(&self, request: &ProviderAnalysisRequest) -> Result<AiSuggestionPayload, String> {
        let categories: Vec<_> = request
            .categories
            .iter()
            .filter(|category| category.enabled)
            .map(|category| {
                json!({
                    "id": category.id,
                    "name": category.name,
                    "description": category.description,
                })
            })
            .collect();
        let schema = suggestion_schema();
        let body = json!({
            "model": self.model,
            "stream": false,
            "format": schema,
            "messages": [
                {
                    "role": "system",
                    "content": format!("你是本地文件整理助手。输出必须完全符合 JSON Schema。摘要和理由使用中文；不得输出路径、创建目录或改变扩展名。根据文件语言类型理解正文：{}。提示词版本：{PROMPT_VERSION}", request.language.as_deref().unwrap_or("未知") )
                },
                {
                    "role": "user",
                    "content": format!("文件名：{}\n语言类型：{}\n可选分类：{}\n正文：\n{}", request.filename, request.language.as_deref().unwrap_or("未知"), serde_json::to_string(&categories).unwrap_or_else(|_| "[]".into()), request.text)
                }
            ],
            "options": {
                "temperature": 0.1,
                "num_predict": 1024
            }
        });
        let response = self
            .client
            .post(self.url("/api/chat")?)
            .json(&body)
            .send()
            .map_err(|error| format!("Ollama 推理失败或超时：{error}"))?;
        if !response.status().is_success() {
            let status = response.status();
            return Err(format!("Ollama 推理失败：HTTP {status}"));
        }
        let response: serde_json::Value = response
            .json()
            .map_err(|error| format!("Ollama 响应不是有效 JSON：{error}"))?;
        let content = response["message"]["content"]
            .as_str()
            .ok_or_else(|| "Ollama 响应缺少 message.content".to_string())?;
        serde_json::from_str(content).map_err(|error| format!("模型输出不符合封闭结构：{error}"))
    }
}

fn is_loopback_host(host: Option<&str>) -> bool {
    match host {
        Some("localhost") => true,
        Some(value) => value
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        None => false,
    }
}

pub fn suggestion_schema() -> serde_json::Value {
    json!({
        "additionalProperties": false,
        "type": "object",
        "required": ["summary", "keywords", "suggested_filename", "category_id", "confidence", "reason"],
        "properties": {
            "summary": { "type": "string", "minLength": 1 },
            "keywords": { "type": "array", "minItems": 1, "items": { "type": "string", "minLength": 1 } },
            "suggested_filename": { "type": "string", "minLength": 1 },
            "category_id": { "type": ["string", "null"] },
            "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "reason": { "type": "string", "minLength": 1 }
        }
    })
}
