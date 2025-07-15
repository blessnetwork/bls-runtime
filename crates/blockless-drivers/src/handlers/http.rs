use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr, time::Duration};

// Import RPC types from parent module
use crate::wasi::rpc::{JsonRpcError, JsonRpcErrorCode, JsonRpcResponse, RPC_VERSION};

// HTTP request structures matching the SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRpcRequest {
    pub url: String,
    pub options: HttpOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<HttpBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_params: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HttpBody {
    Text(String),
    Binary(Vec<u8>),
    Form(HashMap<String, String>),
    Multipart(Vec<MultipartField>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartField {
    pub name: String,
    pub value: MultipartValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultipartValue {
    Text(String),
    Binary {
        data: Vec<u8>,
        filename: Option<String>,
        content_type: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HttpResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_http_request(params: Option<serde_json::Value>, id: u32) -> JsonRpcResponse {
    // Parse the HTTP request parameters
    let http_request: HttpRpcRequest = match params {
        Some(p) => match serde_json::from_value(p) {
            Ok(req) => req,
            Err(e) => {
                return JsonRpcResponse {
                    jsonrpc: RPC_VERSION.to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: Some(serde_json::json!({
                            "error": format!("Failed to parse HTTP request: {}", e)
                        })),
                    }),
                    id,
                };
            }
        },
        None => {
            return JsonRpcResponse {
                jsonrpc: RPC_VERSION.to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: JsonRpcErrorCode::InvalidParams as i32,
                    message: "Invalid params".to_string(),
                    data: Some(serde_json::json!({
                        "error": "Missing HTTP request parameters"
                    })),
                }),
                id,
            };
        }
    };

    // Execute the HTTP request using the http_v2 driver
    let result = execute_http_request(http_request).await;
    JsonRpcResponse {
        jsonrpc: RPC_VERSION.to_string(),
        result: match serde_json::to_value(result) {
            Ok(value) => Some(value),
            Err(e) => {
                return JsonRpcResponse {
                    jsonrpc: RPC_VERSION.to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: JsonRpcErrorCode::InternalError as i32,
                        message: "Internal error".to_string(),
                        data: Some(serde_json::json!({
                            "error": format!("Failed to serialize result: {}", e)
                        })),
                    }),
                    id,
                };
            }
        },
        error: None,
        id,
    }
}

pub async fn execute_http_request(request: HttpRpcRequest) -> HttpResult {
    log::trace!("=== HTTP Request via RPC ===");
    log::trace!("URL: {}", request.url);
    log::trace!("Method: {:?}", request.options.method);
    log::trace!("Headers: {:?}", request.options.headers);
    log::trace!("Body: {:?}", request.options.body);
    log::trace!("Timeout: {:?}", request.options.timeout);
    log::trace!("Query Params: {:?}", request.options.query_params);
    log::trace!("============================");

    let result = async {
        // Create HTTP client with timeout
        let timeout = Duration::from_millis(request.options.timeout.unwrap_or(30000) as u64);
        let client = Client::builder().timeout(timeout).build()?;

        // Parse HTTP method
        let method = request.options.method.as_deref().unwrap_or("GET");
        let http_method = Method::from_str(method)?;

        // Build URL with query parameters
        let mut url = reqwest::Url::parse(&request.url)?;
        // Only add query parameters if the URL doesn't already have a query string
        if url.query().is_none() {
            if let Some(query_params) = &request.options.query_params {
                for (key, value) in query_params {
                    url.query_pairs_mut().append_pair(key, value);
                }
            }
        }

        // Create request builder
        let mut req_builder = client.request(http_method, url.clone());

        // Add headers
        if let Some(headers) = &request.options.headers {
            for (key, value) in headers {
                req_builder = req_builder.header(key, value);
            }
        }

        // Add body based on type
        if let Some(body) = &request.options.body {
            req_builder = match body {
                HttpBody::Text(text) => req_builder.body(text.clone()),
                HttpBody::Binary(data) => req_builder.body(data.clone()),
                HttpBody::Form(form_data) => {
                    let mut form = reqwest::multipart::Form::new();
                    for (key, value) in form_data {
                        form = form.text(key.clone(), value.clone());
                    }
                    req_builder.multipart(form)
                }
                HttpBody::Multipart(fields) => {
                    let mut form = reqwest::multipart::Form::new();
                    for field in fields {
                        match &field.value {
                            MultipartValue::Text(text) => {
                                form = form.text(field.name.clone(), text.clone());
                            }
                            MultipartValue::Binary {
                                data,
                                filename,
                                content_type,
                            } => {
                                let mut part = reqwest::multipart::Part::bytes(data.clone());
                                if let Some(filename) = filename {
                                    part = part.file_name(filename.clone());
                                }
                                if let Some(content_type) = content_type {
                                    part = part.mime_str(content_type)?;
                                }
                                form = form.part(field.name.clone(), part);
                            }
                        }
                    }
                    req_builder.multipart(form)
                }
            };
        }

        // Execute the request
        let response = req_builder.send().await?;
        let status = response.status().as_u16();
        let final_url = response.url().to_string();

        // Extract headers
        let mut headers = HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // Get response body
        let body = response.bytes().await?.to_vec();

        Ok::<HttpResponse, Box<dyn std::error::Error + Send + Sync>>(HttpResponse {
            status,
            headers,
            body,
            url: final_url,
        })
    }
    .await;

    match result {
        Ok(response) => HttpResult {
            success: true,
            data: Some(response),
            error: None,
        },
        Err(e) => {
            eprintln!("HTTP request failed: {}", e);
            HttpResult {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }
        }
    }
}
