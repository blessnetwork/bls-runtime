#![allow(non_upper_case_globals)]
use crate::{LlmErrorKind, llm_driver};
use log::error;
use wasi_common::WasiCtx;
use wiggle::{GuestMemory, GuestPtr};

wiggle::from_witx!({
    witx: ["$BLOCKLESS_DRIVERS_ROOT/witx/blockless_llm.witx"],
    errors: { llm_error => LlmErrorKind },
    async: *,
    wasmtime: false,
});

impl types::UserErrorConversion for WasiCtx {
    fn llm_error_from_llm_error_kind(
        &mut self,
        e: self::LlmErrorKind,
    ) -> wiggle::anyhow::Result<types::LlmError> {
        Ok(e.into())
    }
}

impl From<LlmErrorKind> for types::LlmError {
    fn from(e: LlmErrorKind) -> types::LlmError {
        use types::LlmError;
        match e {
            LlmErrorKind::ModelNotSet => LlmError::ModelNotSet,
            LlmErrorKind::ModelNotSupported => LlmError::ModelNotSupported,
            LlmErrorKind::ModelInitializationFailed => LlmError::ModelInitializationFailed,
            LlmErrorKind::ModelCompletionFailed => LlmError::ModelCompletionFailed,
            LlmErrorKind::ModelOptionsNotSet => LlmError::ModelOptionsNotSet,
            LlmErrorKind::ModelShutdownFailed => LlmError::ModelShutdownFailed,
            LlmErrorKind::Utf8Error => LlmError::Utf8Error,
            LlmErrorKind::RuntimeError => LlmError::RuntimeError,
            LlmErrorKind::MCPFunctionCallError => LlmError::McpFunctionCallError,
            LlmErrorKind::PermissionDeny => LlmError::PermissionDeny,
        }
    }
}

impl wiggle::GuestErrorType for types::LlmError {
    fn success() -> Self {
        Self::Success
    }
}

#[wiggle::async_trait]
impl blockless_llm::BlocklessLlm for WasiCtx {
    /// Sets the LLM model
    /// - Mutates the handle to point to the new model/instance
    async fn llm_set_model_request(
        &mut self,
        memory: &mut GuestMemory<'_>,
        handle: GuestPtr<types::LlmHandle>,
        model: GuestPtr<str>,
    ) -> Result<(), LlmErrorKind> {
        let model: &str = memory
            .as_str(model)
            .map_err(|e| {
                error!("guest model error: {}", e);
                LlmErrorKind::Utf8Error
            })?
            .unwrap();
        // Use a closure that captures self to check URL permissions
        let fd = llm_driver::llm_set_model(model, |url: &url::Url| -> bool {
            self.check_url_permissions(url, "llm_set_model")
        })
        .await?;
        memory
            .write(handle, fd)
            .map_err(|_| LlmErrorKind::RuntimeError)?;
        return Ok(());
    }

    /// Gets the current model name
    /// - Writes the model name to the buffer
    /// - Returns the number of bytes written to the buffer
    async fn llm_get_model_response(
        &mut self,
        memory: &mut GuestMemory<'_>,
        handle: types::LlmHandle,
        buf: GuestPtr<u8>,
        buf_len: u8,
    ) -> Result<u8, LlmErrorKind> {
        let model = llm_driver::llm_get_model(handle).await?;
        let bytes = model.as_bytes();
        let copyn = buf_len.min(bytes.len() as u8);
        memory
            .copy_from_slice(&bytes[..copyn as usize], buf.as_array(copyn as u32))
            .map_err(|_| LlmErrorKind::RuntimeError)?;
        Ok(copyn as u8)
    }

    /// Sets the LLM model
    /// - Mutates the handle to point to the new model/instance
    async fn llm_set_model_options_request(
        &mut self,
        memory: &mut GuestMemory<'_>,
        handle: types::LlmHandle,
        options: GuestPtr<str>,
    ) -> Result<(), LlmErrorKind> {
        let options: &str = memory
            .as_str(options)
            .map_err(|e| {
                error!("guest options error: {}", e);
                LlmErrorKind::Utf8Error
            })?
            .unwrap();
        llm_driver::llm_set_options(handle, options.as_bytes()).await?;
        return Ok(());
    }

    /// Gets the model options
    /// - Writes the model options to the buffer
    /// - Returns the number of bytes written to the buffer
    async fn llm_get_model_options(
        &mut self,
        memory: &mut GuestMemory<'_>,
        handle: types::LlmHandle,
        buf: GuestPtr<u8>,
        buf_len: u16,
    ) -> Result<u16, LlmErrorKind> {
        let options = llm_driver::llm_get_options(handle).await?;
        let bytes = serde_json::to_vec(&options).map_err(|_| LlmErrorKind::RuntimeError)?;
        let copyn = buf_len.min(bytes.len() as u16);
        memory
            .copy_from_slice(&bytes[..copyn as usize], buf.as_array(copyn as u32))
            .map_err(|_| LlmErrorKind::RuntimeError)?;
        Ok(copyn as u16)
    }

    async fn llm_prompt_request(
        &mut self,
        memory: &mut GuestMemory<'_>,
        handle: types::LlmHandle,
        prompt: GuestPtr<str>,
    ) -> Result<(), LlmErrorKind> {
        let prompt: &str = memory
            .as_str(prompt)
            .map_err(|e| {
                error!("guest prompt error: {}", e);
                LlmErrorKind::Utf8Error
            })?
            .unwrap();
        llm_driver::llm_prompt(handle, prompt).await?;
        Ok(())
    }

    async fn llm_read_prompt_response(
        &mut self,
        memory: &mut GuestMemory<'_>,
        handle: types::LlmHandle,
        buf: GuestPtr<u8>,
        buf_len: u16,
    ) -> Result<u16, LlmErrorKind> {
        let response = llm_driver::llm_read_response(handle).await?;
        let bytes = response.as_bytes();
        let copyn = buf_len.min(bytes.len() as u16);
        memory
            .copy_from_slice(&bytes[..copyn as usize], buf.as_array(copyn as u32))
            .map_err(|_| LlmErrorKind::RuntimeError)?;
        Ok(copyn as u16)
    }

    async fn llm_close(
        &mut self,
        _memory: &mut GuestMemory<'_>,
        handle: types::LlmHandle,
    ) -> Result<(), LlmErrorKind> {
        llm_driver::llm_close(handle).await
    }
}
