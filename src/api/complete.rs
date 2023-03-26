use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use hyper::{Body, Request, Uri};
use serde::{Deserialize, Serialize};

use crate::client::fetch_sse;
use crate::client::MultiClient;
use futures::StreamExt;

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::Stream;

#[derive(Debug, Clone)]
pub struct CompleteAPI {
    pub complete: Arc<RwLock<Complete>>,
    pub pending_generate: Arc<RwLock<Option<String>>>,
    api_key: String,
    client: Arc<MultiClient>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CompleteCompletion {
    id: Option<String>,
    object: Option<String>,
    created: Option<u64>,
    model: Option<String>,
    choices: Option<Vec<CompleteChoice>>,
    usage: Option<CompleteUsage>,
    error: Option<CompleteError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CompleteError {
    message: String,
    r#type: String,
    param: Option<String>,
    code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CompleteChoice {
    text: String,
    index: u32,
    logprobs: Option<u32>,
    finish_reason: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
struct CompleteUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
impl CompleteAPI {
    const DEFAULT_MODEL: &'static str = "text-davinci-003";
    const URL: &'static str = "https://api.openai.com/v1/completions";
    pub async fn set_prompt(&mut self, prompt: String) {
        self.complete.write().await.prompt = prompt;
    }
    pub async fn generate(&mut self) -> Result<String, anyhow::Error> {
        let mut stream = self.complete().await?;
        *self.pending_generate.write().await = Some(self.complete.read().await.prompt.clone());
        while let Some(res) = stream.next().await {
            let res = match res {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Error: {}", e);
                    self.pending_generate.write().await.take();
                    return Err(e);
                }
            };
            let mut pending_generate = self.pending_generate.write().await;
            let pending_generate = pending_generate.as_mut().unwrap();
            let Some(choices) = &res.choices else {
                continue;
            };
            let Some(first_choice) = &choices.first() else{
                continue;
            };
            let text = &first_choice.text;
            // if text == "\n\n" || text == "\n\n\n" {
            //     continue;
            // }
            pending_generate.push_str(&text);
        }
        let Some(text) = self.pending_generate.write().await.take()  else {
            return Err(anyhow::anyhow!("No text generated"));
        };
        let text = if let Some(suffix) = &self.complete.write().await.suffix.take() {
            format!("{}{}", text, suffix)
        } else {
            text
        };
        self.complete.write().await.prompt = text.clone();
        Ok(text)
    }
    pub async fn insert(&mut self, index: usize) -> Result<String, anyhow::Error> {
        {
            let mut complete = self.complete.write().await;
            let prompt = complete.prompt.clone();
            let (prompt, suffix) = prompt.split_at(index);
            complete.prompt = prompt.to_string();
            complete.suffix = Some(suffix.to_string());
        }
        let res = self.generate().await?;
        Ok(res)
    }
    async fn complete(
        &self,
    ) -> Result<impl Stream<Item = Result<CompleteCompletion, anyhow::Error>>, anyhow::Error> {
        let uri: Uri = Self::URL.parse()?;
        let body = Body::from(serde_json::to_string(&self.complete.write().await.clone())?);
        let mut request_body = Request::new(body);
        *request_body.method_mut() = hyper::Method::POST;
        *request_body.uri_mut() = uri.clone();
        request_body
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        request_body.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))?,
        );
        let response = self.client.request(request_body).await?;
        let stream = fetch_sse::<CompleteCompletion>(response);
        Ok(stream)
    }
}

pub struct CompleteAPIBuilder {
    api_key: String,
    complete: Complete,
}

impl CompleteAPIBuilder {
    pub fn new(api_key: String) -> Self {
        let complete = Complete {
            model: CompleteAPI::DEFAULT_MODEL.to_string(),
            prompt: "".to_string(),
            suffix: None,
            max_tokens: Some(2048),
            temperature: None,
            top_p: None,
            n: None,
            stream: Some(true),
            logprobs: None,
        };
        Self { api_key, complete }
    }
    pub fn with_complete(mut self, complete: Complete) -> Self {
        self.complete = complete;
        self
    }
    pub fn build(self) -> CompleteAPI {
        CompleteAPI {
            complete: Arc::new(RwLock::new(self.complete)),
            pending_generate: Arc::new(RwLock::new(None)),
            api_key: self.api_key,
            client: Arc::new(MultiClient::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Complete {
    model: String,
    pub prompt: String,
    pub suffix: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    n: Option<u32>,
    stream: Option<bool>,
    logprobs: Option<u32>,
}
