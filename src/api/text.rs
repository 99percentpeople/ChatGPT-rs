use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Text {
    model: String,
    prompt: String,
    suffix: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    n: Option<u32>,
    stream: Option<bool>,
    logprobs: Option<u32>,
}
