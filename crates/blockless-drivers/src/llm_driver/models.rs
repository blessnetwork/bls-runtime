use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum SupportedModels {
    Llama321BInstruct(Option<String>),
    Llama323BInstruct(Option<String>),
    Mistral7BInstructV03(Option<String>),
    Mixtral8x7BInstructV01(Option<String>),
    Gemma22BInstruct(Option<String>),
    Gemma27BInstruct(Option<String>),
    Gemma29BInstruct(Option<String>),
}

impl SupportedModels {
    pub fn model_repo(&self) -> String {
        match self {
            SupportedModels::Llama321BInstruct(_) => "Llama-3.2-1B-Instruct-llamafile".to_string(),
            SupportedModels::Llama323BInstruct(_) => "Llama-3.2-3B-Instruct-llamafile".to_string(),
            SupportedModels::Mistral7BInstructV03(_) => {
                "Mistral-7B-Instruct-v0.3-llamafile".to_string()
            }
            SupportedModels::Mixtral8x7BInstructV01(_) => {
                "Mixtral-8x7B-Instruct-v0.1-llamafile".to_string()
            }
            SupportedModels::Gemma22BInstruct(_) => "gemma-2-2b-it-llamafile".to_string(),
            SupportedModels::Gemma27BInstruct(_) => "gemma-2-27b-it-llamafile".to_string(),
            SupportedModels::Gemma29BInstruct(_) => "gemma-2-9b-it-llamafile".to_string(),
        }
    }

    pub fn model_file(&self) -> String {
        match self {
            SupportedModels::Llama321BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Llama-3.2-1B-Instruct.{}", suffix)
            }
            SupportedModels::Llama323BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Llama-3.2-3B-Instruct.{}", suffix)
            }
            SupportedModels::Mistral7BInstructV03(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Mistral-7B-Instruct-v0.3.{}", suffix)
            }
            SupportedModels::Mixtral8x7BInstructV01(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("Mixtral-8x7B-Instruct-v0.1.{}", suffix)
            }
            SupportedModels::Gemma22BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("gemma-2-2b-it.{}", suffix)
            }
            SupportedModels::Gemma27BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("gemma-2-27b-it.{}", suffix)
            }
            SupportedModels::Gemma29BInstruct(quantization) => {
                let suffix = quantization.clone().unwrap_or("Q6_K.llamafile".to_string());
                format!("gemma-2-9b-it.{}", suffix)
            }
        }
    }
}

impl FromStr for SupportedModels {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // Llama 3.2 1B
            "Llama-3.2-1B-Instruct" => Ok(SupportedModels::Llama321BInstruct(None)),
            "Llama-3.2-1B-Instruct-Q6_K"
            | "Llama-3.2-1B-Instruct_Q6_K"
            | "Llama-3.2-1B-Instruct.Q6_K" => {
                Ok(SupportedModels::Llama321BInstruct(Some("Q6_K".to_string())))
            }
            "Llama-3.2-1B-Instruct-q4f16_1" | "Llama-3.2-1B-Instruct.q4f16_1" => Ok(
                SupportedModels::Llama321BInstruct(Some("q4f16_1".to_string())),
            ),

            // Llama 3.2 3B
            "Llama-3.2-3B-Instruct" => Ok(SupportedModels::Llama323BInstruct(None)),
            "Llama-3.2-3B-Instruct-Q6_K"
            | "Llama-3.2-3B-Instruct_Q6_K"
            | "Llama-3.2-3B-Instruct.Q6_K" => {
                Ok(SupportedModels::Llama323BInstruct(Some("Q6_K".to_string())))
            }
            "Llama-3.2-3B-Instruct-q4f16_1" | "Llama-3.2-3B-Instruct.q4f16_1" => Ok(
                SupportedModels::Llama323BInstruct(Some("q4f16_1".to_string())),
            ),

            // Mistral 7B
            "Mistral-7B-Instruct-v0.3" => Ok(SupportedModels::Mistral7BInstructV03(None)),
            "Mistral-7B-Instruct-v0.3-q4f16_1" | "Mistral-7B-Instruct-v0.3.q4f16_1" => Ok(
                SupportedModels::Mistral7BInstructV03(Some("q4f16_1".to_string())),
            ),

            // Mixtral 8x7B
            "Mixtral-8x7B-Instruct-v0.1" => Ok(SupportedModels::Mixtral8x7BInstructV01(None)),
            "Mixtral-8x7B-Instruct-v0.1-q4f16_1" | "Mixtral-8x7B-Instruct-v0.1.q4f16_1" => Ok(
                SupportedModels::Mixtral8x7BInstructV01(Some("q4f16_1".to_string())),
            ),

            // Gemma models
            "gemma-2-2b-it" => Ok(SupportedModels::Gemma22BInstruct(None)),
            "gemma-2-2b-it-q4f16_1" | "gemma-2-2b-it.q4f16_1" => Ok(
                SupportedModels::Gemma22BInstruct(Some("q4f16_1".to_string())),
            ),

            "gemma-2-27b-it" => Ok(SupportedModels::Gemma27BInstruct(None)),
            "gemma-2-27b-it-q4f16_1" | "gemma-2-27b-it.q4f16_1" => Ok(
                SupportedModels::Gemma27BInstruct(Some("q4f16_1".to_string())),
            ),

            "gemma-2-9b-it" => Ok(SupportedModels::Gemma29BInstruct(None)),
            "gemma-2-9b-it-q4f16_1" | "gemma-2-9b-it.q4f16_1" => Ok(
                SupportedModels::Gemma29BInstruct(Some("q4f16_1".to_string())),
            ),

            _ => Err(format!("Unsupported model: {}", s)),
        }
    }
}

impl ToString for SupportedModels {
    fn to_string(&self) -> String {
        match self {
            SupportedModels::Llama321BInstruct(_) => "Llama-3.2-1B-Instruct".to_string(),
            SupportedModels::Llama323BInstruct(_) => "Llama-3.2-3B-Instruct".to_string(),
            SupportedModels::Mistral7BInstructV03(_) => "Mistral-7B-Instruct-v0.3".to_string(),
            SupportedModels::Mixtral8x7BInstructV01(_) => "Mixtral-8x7B-Instruct-v0.1".to_string(),
            SupportedModels::Gemma22BInstruct(_) => "gemma-2-2b-it".to_string(),
            SupportedModels::Gemma27BInstruct(_) => "gemma-2-27b-it".to_string(),
            SupportedModels::Gemma29BInstruct(_) => "gemma-2-9b-it".to_string(),
        }
    }
}
