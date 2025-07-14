#![allow(non_upper_case_globals)]
use crate::BlocklessRpcErrorKind;
use serde::{Deserialize, Serialize};
use wasi_common::WasiCtx;
use wiggle::{GuestMemory, GuestPtr};

wiggle::from_witx!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_rpc.witx"],
    errors: { blockless_rpc_error => BlocklessRpcErrorKind },
    async: *,
});

// JSON-RPC 2.0 structures
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl types::UserErrorConversion for WasiCtx {
    fn blockless_rpc_error_from_blockless_rpc_error_kind(
        &mut self,
        e: self::BlocklessRpcErrorKind,
    ) -> wiggle::anyhow::Result<types::BlocklessRpcError> {
        Ok(e.into())
    }
}

impl From<BlocklessRpcErrorKind> for types::BlocklessRpcError {
    fn from(e: BlocklessRpcErrorKind) -> types::BlocklessRpcError {
        use types::BlocklessRpcError;
        match e {
            BlocklessRpcErrorKind::InvalidJson => BlocklessRpcError::InvalidJson,
            BlocklessRpcErrorKind::MethodNotFound => BlocklessRpcError::MethodNotFound,
            BlocklessRpcErrorKind::InvalidParams => BlocklessRpcError::InvalidParams,
            BlocklessRpcErrorKind::InternalError => BlocklessRpcError::InternalError,
            BlocklessRpcErrorKind::BufferTooSmall => BlocklessRpcError::BufferTooSmall,
        }
    }
}

impl wiggle::GuestErrorType for types::BlocklessRpcError {
    fn success() -> Self {
        Self::Success
    }
}

#[wiggle::async_trait]
impl bless::Bless for WasiCtx {
    async fn rpc_call(
        &mut self,
        memory: &mut GuestMemory<'_>,
        request_buf: GuestPtr<u8>,
        request_len: u32,
        response_buf: GuestPtr<u8>,
        response_max_len: u32,
    ) -> Result<u32, BlocklessRpcErrorKind> {
        // Read the JSON-RPC request from WASM memory
        let request_bytes = memory
            .as_slice(request_buf.as_array(request_len))
            .map_err(|_| BlocklessRpcErrorKind::InternalError)?
            .unwrap();

        // Parse JSON-RPC request directly from bytes
        let request: JsonRpcRequest = serde_json::from_slice(request_bytes)
            .map_err(|_| BlocklessRpcErrorKind::InvalidJson)?;

        // Handle the request
        let response = handle_rpc_request(request).await;

        // Serialize response directly to bytes
        let response_bytes = serde_json::to_vec(&response)
            .map_err(|_| BlocklessRpcErrorKind::InternalError)?;

        // Check if response fits in buffer
        let response_len = response_bytes.len() as u32;
        if response_len > response_max_len {
            return Err(BlocklessRpcErrorKind::BufferTooSmall);
        }

        // Write response to WASM memory
        memory
            .copy_from_slice(&response_bytes, response_buf.as_array(response_len))
            .map_err(|_| BlocklessRpcErrorKind::InternalError)?;

        Ok(response_len)
    }
}

async fn handle_rpc_request(request: JsonRpcRequest) -> JsonRpcResponse {
    let id = request.id;
    
    match request.method.as_str() {
        "ping" => {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!("pong")),
                error: None,
                id,
            }
        }
        "echo" => {
            let params = request.params.unwrap_or(serde_json::Value::Null);
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(params),
                error: None,
                id,
            }
        }
        "version" => {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({
                    "runtime": "bls-runtime",
                    "version": env!("CARGO_PKG_VERSION"),
                    "rpc_version": "2.0"
                })),
                error: None,
                id,
            }
        }
        "http.request" => {
            crate::handlers::http::handle_http_request(request.params, id).await
        }
        _ => {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: Some(serde_json::json!({
                        "method": request.method
                    })),
                }),
                id,
            }
        }
    }
}