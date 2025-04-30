use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Models {
    Llama321BInstruct(Option<String>),
    Llama323BInstruct(Option<String>),
    Mistral7BInstructV03(Option<String>),
    Mixtral8x7BInstructV01(Option<String>),
    Gemma22BInstruct(Option<String>),
    Gemma27BInstruct(Option<String>),
    Gemma29BInstruct(Option<String>),
    URL(url::Url),
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
            Models::URL(_) => None,
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
            Models::URL(model_url) => model_url
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
            _ => url::Url::parse(s)
                .map(|url| Models::URL(url))
                .map_err(|_| format!("Invalid model url: {}", s)),
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
            Models::URL(model_url) => write!(f, "{}", model_url),
        }
    }
}
