use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use hyper::{Body, Request, Uri};
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::client::fetch_sse;
use crate::client::MultiClient;
use futures::StreamExt;

use std::cell::RefCell;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::Stream;

use super::{Param, ParameterControl};

#[derive(Debug, Clone)]
pub struct CompleteAPI {
    pub data: Arc<RwLock<Complete>>,
    pub pending_generate: Arc<RwLock<Option<String>>>,
    api_key: Arc<RwLock<String>>,
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

    pub fn data(&self) -> Complete {
        task::block_in_place(|| self.data.blocking_read().clone())
    }

    pub async fn set_prompt(&mut self, prompt: String) {
        self.data.write().await.prompt = prompt;
    }
    pub async fn generate(&self) -> Result<String, anyhow::Error> {
        let mut stream = self.complete().await?;
        *self.pending_generate.write().await = Some(self.data.read().await.prompt.clone());
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
        let text = if let Some(suffix) = &self.data.write().await.suffix.take() {
            format!("{}{}", text, suffix)
        } else {
            text
        };
        self.data.write().await.prompt = text.clone();
        Ok(text)
    }
    pub async fn insert(&self, index: usize) -> Result<String, anyhow::Error> {
        {
            let mut complete = self.data.write().await;
            let prompt = complete.prompt.clone();
            let (prompt, suffix) = split_by_char(&prompt, index);
            complete.prompt = prompt.to_string();
            complete.suffix = Some(suffix.to_string());
        }
        // tracing::info!(
        //     prompt = complete.prompt,
        //     suffix = complete.suffix.as_ref().unwrap_or(&"".to_string())
        // );
        Ok(self.generate().await?)
    }
    async fn complete(
        &self,
    ) -> Result<impl Stream<Item = Result<CompleteCompletion, anyhow::Error>>, anyhow::Error> {
        let uri: Uri = Self::URL.parse()?;
        let body = Body::from(serde_json::to_string(&self.data.write().await.clone())?);
        let mut request_body = Request::new(body);
        *request_body.method_mut() = hyper::Method::POST;
        *request_body.uri_mut() = uri.clone();
        request_body
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        request_body.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key.read().await))?,
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
            max_tokens: Some(100),
            temperature: Some(0.3),
            top_p: None,
            presence_penalty: Some(0.),
            frequency_penalty: Some(0.),
            stop: Vec::new(),
            n: None,
            stream: Some(true),
            logprobs: None,
        };
        Self { api_key, complete }
    }
    pub fn with_data(mut self, complete: Complete) -> Self {
        self.complete = complete;
        self
    }
    pub fn build(self) -> CompleteAPI {
        CompleteAPI {
            data: Arc::new(RwLock::new(self.complete)),
            pending_generate: Arc::new(RwLock::new(None)),
            api_key: Arc::new(RwLock::new(self.api_key)),
            client: Arc::new(MultiClient::new()),
        }
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Complete {
    model: String,
    pub prompt: String,
    pub suffix: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    n: Option<u32>,
    stream: Option<bool>,
    logprobs: Option<u32>,
}

fn split_by_char(string: &str, mid: usize) -> (&str, &str) {
    let mut len = 0;
    for ch in string.chars().by_ref().take(mid) {
        len += ch.len_utf8();
    }

    (&string[..len], &string[len..])
}

impl ParameterControl for CompleteAPI {
    fn params(&self) -> Vec<Box<dyn super::Parameter>> {
        let mut v = Vec::new();
        v.push(Box::new(Param {
            name: "max_tokens",
            range: Some((1, 4000).into()),
            default: 2048.into(),
            store: RefCell::new(tokio::task::block_in_place(|| {
                self.data.blocking_read().max_tokens
            })),
            getter: {
                let complete = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| complete.blocking_read().max_tokens)
                })
            },
            setter: {
                let complete = self.data.clone();
                Box::new(move |max_tokens| {
                    let complete = complete.clone();
                    tokio::spawn(async move {
                        complete.write().await.max_tokens = max_tokens;
                    });
                })
            },
        }) as Box<dyn super::Parameter>);
        v.push(Box::new(Param {
            name: "temperature",
            range: Some((0., 2.).into()),
            default: (0.3).into(),
            store: RefCell::new(0.3),
            getter: {
                let complete = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| {
                        complete.blocking_read().temperature.unwrap_or(0.3)
                    })
                })
            },
            setter: {
                let complete = self.data.clone();
                Box::new(move |temperature| {
                    let complete = complete.clone();
                    tokio::spawn(async move {
                        complete.write().await.temperature = Some(temperature);
                    });
                })
            },
        }));
        v.push(Box::new(Param {
            name: "top_p",
            range: Some((0., 2.).into()),
            default: (1.).into(),
            store: RefCell::new(1.),
            getter: {
                let complete = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| complete.blocking_read().top_p.unwrap_or(1.))
                })
            },
            setter: {
                let complete = self.data.clone();
                Box::new(move |top_p| {
                    let complete = complete.clone();
                    tokio::spawn(async move {
                        complete.write().await.top_p = Some(top_p);
                    });
                })
            },
        }));
        v.push(Box::new(Param {
            name: "presence_penalty",
            range: Some((-2., 2.).into()),
            default: (0.).into(),
            store: RefCell::new(0.),
            getter: {
                let complete = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| {
                        complete.blocking_read().presence_penalty.unwrap_or(0.)
                    })
                })
            },
            setter: {
                let complete = self.data.clone();
                Box::new(move |presence_penalty| {
                    let data = complete.clone();
                    tokio::spawn(async move {
                        data.write().await.presence_penalty = Some(presence_penalty);
                    });
                })
            },
        }));
        v.push(Box::new(Param {
            name: "frequency_penalty",
            range: Some((-2., 2.).into()),
            default: (0.).into(),
            store: RefCell::new(0.),
            getter: {
                let complete = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| {
                        complete.blocking_read().frequency_penalty.unwrap_or(0.)
                    })
                })
            },
            setter: {
                let complete = self.data.clone();
                Box::new(move |frequency_penalty| {
                    let data = complete.clone();
                    tokio::spawn(async move {
                        data.write().await.frequency_penalty = Some(frequency_penalty);
                    });
                })
            },
        }));
        v.push(Box::new(Param::<String> {
            name: "api_key",
            range: None,
            default: task::block_in_place(|| self.api_key.blocking_read().clone()).into(),
            store: task::block_in_place(|| self.api_key.blocking_read().clone()).into(),
            getter: {
                let _self = self.clone();
                Box::new(move || {
                    task::block_in_place(|| _self.api_key.blocking_read().clone()).into()
                })
            },
            setter: {
                let mut _self = self.clone();
                Box::new(move |api_key| {
                    let _self = _self.clone();
                    tokio::spawn(async move {
                        *_self.api_key.write().await = api_key;
                    });
                })
            },
        }));
        v
    }
}
