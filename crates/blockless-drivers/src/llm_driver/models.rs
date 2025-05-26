use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub allowed_domains: Vec<String>,
    pub require_https: bool,
    pub allowed_file_extensions: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            allowed_domains: vec![
                "huggingface.co".to_string(),
                "github.com".to_string(),
                "releases.github.com".to_string(),
            ],
            require_https: true,
            allowed_file_extensions: vec![".llamafile".to_string()],
        }
    }
}

impl SecurityConfig {
    pub fn validate_model_url(&self, url: &url::Url) -> Result<(), String> {
        // Validate HTTPS requirement
        if self.require_https && url.scheme() != "https" {
            return Err("Only HTTPS URLs are allowed for security".to_string());
        }

        // Validate domain allowlist
        let host = url.host_str().ok_or("Invalid URL: no host")?;
        if !self
            .allowed_domains
            .iter()
            .any(|domain| host == domain || host.ends_with(&format!(".{}", domain)))
        {
            return Err(format!(
                "Untrusted domain: {}. Allowed domains: {:?}",
                host, self.allowed_domains
            ));
        }

        // Check for suspicious paths that might indicate path traversal attempts
        let path = url.path();
        if path.contains("..") {
            return Err("Path contains suspicious '..' segments".to_string());
        }

        // Extract filename from URL path
        let filename = url
            .path_segments()
            .and_then(|segments| segments.last())
            .ok_or("Invalid URL: no filename in path")?;

        // Validate filename
        self.validate_filename(filename)?;

        // Additional security: ensure the path looks like a reasonable model path
        if path.starts_with("/etc/")
            || path.starts_with("/windows/")
            || path.starts_with("/system32/")
        {
            return Err("Suspicious system path detected".to_string());
        }

        Ok(())
    }

    pub fn validate_filename(&self, filename: &str) -> Result<(), String> {
        // Check for path traversal attempts
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err("Invalid filename: path traversal detected".to_string());
        }

        // Validate file extension
        if !self
            .allowed_file_extensions
            .iter()
            .any(|ext| filename.ends_with(ext))
        {
            return Err(format!(
                "Invalid file extension. Allowed extensions: {:?}",
                self.allowed_file_extensions
            ));
        }

        // Additional security checks
        if filename.is_empty() {
            return Err("Filename cannot be empty".to_string());
        }

        // Check for control characters or other suspicious characters
        if filename
            .chars()
            .any(|c| c.is_control() || c.is_ascii_control())
        {
            return Err("Filename contains invalid control characters".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Models {
    Llama321BInstruct(Option<String>),
    Llama323BInstruct(Option<String>),
    Mistral7BInstructV03(Option<String>),
    Mixtral8x7BInstructV01(Option<String>),
    Gemma22BInstruct(Option<String>),
    Gemma27BInstruct(Option<String>),
    Gemma29BInstruct(Option<String>),
    Url(url::Url),
}

impl Models {
    pub fn model_repo(&self) -> Option<String> {
        match self {
            Models::Llama321BInstruct(_) => {
                Some("Mozilla/Llama-3.2-1B-Instruct-llamafile".to_string())
            }
            Models::Llama323BInstruct(_) => {
                Some("Mozilla/Llama-3.2-3B-Instruct-llamafile".to_string())
            }
            Models::Mistral7BInstructV03(_) => {
                Some("Mozilla/Mistral-7B-Instruct-v0.3-llamafile".to_string())
            }
            Models::Mixtral8x7BInstructV01(_) => {
                Some("Mozilla/Mixtral-8x7B-Instruct-v0.1-llamafile".to_string())
            }
            Models::Gemma22BInstruct(_) => Some("Mozilla/gemma-2-2b-it-llamafile".to_string()),
            Models::Gemma27BInstruct(_) => Some("Mozilla/gemma-2-27b-it-llamafile".to_string()),
            Models::Gemma29BInstruct(_) => Some("Mozilla/gemma-2-9b-it-llamafile".to_string()),
            Models::Url(_) => None,
        }
    }

    pub fn model_file(&self) -> String {
        match self {
            Models::Llama321BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Llama-3.2-1B-Instruct.{}", suffix)
            }
            Models::Llama323BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Llama-3.2-3B-Instruct.{}", suffix)
            }
            Models::Mistral7BInstructV03(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Mistral-7B-Instruct-v0.3.{}", suffix)
            }
            Models::Mixtral8x7BInstructV01(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Mixtral-8x7B-Instruct-v0.1.{}", suffix)
            }
            Models::Gemma22BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("gemma-2-2b-it.{}", suffix)
            }
            Models::Gemma27BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("gemma-2-27b-it.{}", suffix)
            }
            Models::Gemma29BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("gemma-2-9b-it.{}", suffix)
            }
            // Assume format is `https://huggingface.co/Mozilla/Meta-Llama-3.1-8B-Instruct-llamafile/resolve/main/Meta-Llama-3.1-8B-Instruct.Q6_K.llamafile?download=true`
            // and return the last part before any query parameters
            Models::Url(model_url) => model_url
                .path_segments()
                .unwrap()
                .last()
                .unwrap()
                .to_string(),
        }
    }
}

impl FromStr for Models {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // Llama 3.2 1B
            "Llama-3.2-1B-Instruct" => Ok(Models::Llama321BInstruct(None)),
            "Llama-3.2-1B-Instruct-Q6_K"
            | "Llama-3.2-1B-Instruct_Q6_K"
            | "Llama-3.2-1B-Instruct.Q6_K" => {
                Ok(Models::Llama321BInstruct(Some("Q6_K".to_string())))
            }
            "Llama-3.2-1B-Instruct-q4f16_1" | "Llama-3.2-1B-Instruct.q4f16_1" => {
                Ok(Models::Llama321BInstruct(Some("q4f16_1".to_string())))
            }

            // Llama 3.2 3B
            "Llama-3.2-3B-Instruct" => Ok(Models::Llama323BInstruct(None)),
            "Llama-3.2-3B-Instruct-Q6_K"
            | "Llama-3.2-3B-Instruct_Q6_K"
            | "Llama-3.2-3B-Instruct.Q6_K" => {
                Ok(Models::Llama323BInstruct(Some("Q6_K".to_string())))
            }
            "Llama-3.2-3B-Instruct-q4f16_1" | "Llama-3.2-3B-Instruct.q4f16_1" => {
                Ok(Models::Llama323BInstruct(Some("q4f16_1".to_string())))
            }

            // Mistral 7B
            "Mistral-7B-Instruct-v0.3" => Ok(Models::Mistral7BInstructV03(None)),
            "Mistral-7B-Instruct-v0.3-q4f16_1" | "Mistral-7B-Instruct-v0.3.q4f16_1" => {
                Ok(Models::Mistral7BInstructV03(Some("q4f16_1".to_string())))
            }

            // Mixtral 8x7B
            "Mixtral-8x7B-Instruct-v0.1" => Ok(Models::Mixtral8x7BInstructV01(None)),
            "Mixtral-8x7B-Instruct-v0.1-q4f16_1" | "Mixtral-8x7B-Instruct-v0.1.q4f16_1" => {
                Ok(Models::Mixtral8x7BInstructV01(Some("q4f16_1".to_string())))
            }

            // Gemma models
            "gemma-2-2b-it" => Ok(Models::Gemma22BInstruct(None)),
            "gemma-2-2b-it-q4f16_1" | "gemma-2-2b-it.q4f16_1" => {
                Ok(Models::Gemma22BInstruct(Some("q4f16_1".to_string())))
            }

            "gemma-2-27b-it" => Ok(Models::Gemma27BInstruct(None)),
            "gemma-2-27b-it-q4f16_1" | "gemma-2-27b-it.q4f16_1" => {
                Ok(Models::Gemma27BInstruct(Some("q4f16_1".to_string())))
            }

            "gemma-2-9b-it" => Ok(Models::Gemma29BInstruct(None)),
            "gemma-2-9b-it-q4f16_1" | "gemma-2-9b-it.q4f16_1" => {
                Ok(Models::Gemma29BInstruct(Some("q4f16_1".to_string())))
            }
            // Model must be a valid URL
            _ => {
                let url =
                    url::Url::parse(s).map_err(|_| format!("Invalid model name or URL: {}", s))?;

                // Apply security validation to custom URLs
                let security_config = SecurityConfig::default();
                security_config.validate_model_url(&url)?;

                Ok(Models::Url(url))
            }
        }
    }
}

impl std::fmt::Display for Models {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Models::Llama321BInstruct(_) => write!(f, "Llama-3.2-1B-Instruct"),
            Models::Llama323BInstruct(_) => write!(f, "Llama-3.2-3B-Instruct"),
            Models::Mistral7BInstructV03(_) => write!(f, "Mistral-7B-Instruct-v0.3"),
            Models::Mixtral8x7BInstructV01(_) => write!(f, "Mixtral-8x7B-Instruct-v0.1"),
            Models::Gemma22BInstruct(_) => write!(f, "gemma-2-2b-it"),
            Models::Gemma27BInstruct(_) => write!(f, "gemma-2-27b-it"),
            Models::Gemma29BInstruct(_) => write!(f, "gemma-2-9b-it"),
            Models::Url(model_url) => write!(f, "{}", model_url),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_models() {
        // Known models should work without security validation
        assert!(Models::from_str("Llama-3.2-1B-Instruct").is_ok());
        assert!(Models::from_str("Llama-3.2-3B-Instruct").is_ok());
        assert!(Models::from_str("Mistral-7B-Instruct-v0.3").is_ok());
        assert!(Models::from_str("Mixtral-8x7B-Instruct-v0.1").is_ok());
        assert!(Models::from_str("gemma-2-2b-it").is_ok());

        // Quantized variants should also work
        assert!(Models::from_str("Llama-3.2-1B-Instruct-Q6_K").is_ok());
        assert!(Models::from_str("gemma-2-2b-it-q4f16_1").is_ok());
    }

    #[test]
    fn test_valid_custom_urls() {
        // Valid custom URL should work
        let valid_url = "https://huggingface.co/Mozilla/Meta-Llama-3.1-8B-Instruct-llamafile/resolve/main/Meta-Llama-3.1-8B-Instruct.Q6_K.llamafile?download=true";
        let model = Models::from_str(valid_url).expect("Should parse valid URL");

        // Test model_file() extraction
        let file_name = model.model_file();
        assert_eq!(file_name, "Meta-Llama-3.1-8B-Instruct.Q6_K.llamafile");

        // Test subdomain support
        assert!(Models::from_str("https://files.huggingface.co/model.llamafile").is_ok());

        // Test GitHub domain
        assert!(
            Models::from_str("https://github.com/user/repo/releases/download/v1.0/model.llamafile")
                .is_ok()
        );

        // Test path normalization (should pass after normalization)
        assert!(
            Models::from_str("https://huggingface.co/model/../../../malicious.llamafile").is_ok()
        );
    }

    #[test]
    fn test_security_validations() {
        let config = SecurityConfig::default();

        // Test HTTPS requirement
        let http_url = url::Url::parse("http://huggingface.co/model.llamafile").unwrap();
        assert!(config.validate_model_url(&http_url).is_err());
        assert!(
            config
                .validate_model_url(&http_url)
                .unwrap_err()
                .contains("HTTPS")
        );

        // Test domain allowlisting
        let disallowed_url = url::Url::parse("https://malicious.com/model.llamafile").unwrap();
        assert!(config.validate_model_url(&disallowed_url).is_err());
        assert!(
            config
                .validate_model_url(&disallowed_url)
                .unwrap_err()
                .contains("Untrusted domain")
        );

        // Test file extension validation
        let invalid_ext_url = url::Url::parse("https://huggingface.co/malicious.exe").unwrap();
        assert!(config.validate_model_url(&invalid_ext_url).is_err());
        assert!(
            config
                .validate_model_url(&invalid_ext_url)
                .unwrap_err()
                .contains("Invalid file extension")
        );

        // Test filename security
        assert!(config.validate_filename("model.llamafile").is_ok());
        assert!(config.validate_filename("../model.llamafile").is_err());
        assert!(config.validate_filename("path/to/model.llamafile").is_err());
        assert!(config.validate_filename("").is_err());
        assert!(config.validate_filename("model\x00.llamafile").is_err());
    }

    #[test]
    fn test_invalid_custom_urls() {
        // HTTP URL should fail
        assert!(Models::from_str("http://huggingface.co/model.llamafile").is_err());

        // Untrusted domain should fail
        assert!(Models::from_str("https://malicious.com/model.llamafile").is_err());

        // Invalid extension should fail
        assert!(Models::from_str("https://huggingface.co/malicious.exe").is_err());

        // URLs that normalize to invalid files should fail
        assert!(Models::from_str("https://huggingface.co/../../../etc/passwd").is_err());
        assert!(Models::from_str("https://huggingface.co/windows/system32/cmd.exe").is_err());
    }
}
