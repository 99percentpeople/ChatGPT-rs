use hyper::body::HttpBody;
use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use hyper::{Body, Request, Uri};
use serde::{Deserialize, Serialize};

use crate::client::MultiClient;
use futures::TryStreamExt;
use std::ops::Not;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;

/// POST https://api.openai.com/v1/chat/completions
///
/// Creates a completion for the chat message
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Chat {
    /// `string` `Required`
    ///
    /// ID of the model to use. Currently, only `gpt-3.5-turbo` and `gpt-3.5-turbo-0301` are supported.
    pub model: String,
    /// `array` `Required`
    ///
    /// The messages to generate chat completions for, in the chat format.
    pub messages: Vec<ChatMessage>,
    /// `number` `Optional` `Defaults to 1`
    ///
    /// What sampling temperature to use, between 0 and 2.
    /// Higher values like 0.8 will make the output more random,
    /// while lower values like 0.2 will make it more focused and deterministic.
    pub temperature: Option<f32>,
    /// `number` `Optional` `Defaults to 1`
    ///
    /// An alternative to sampling with temperature, called nucleus sampling,
    /// where the model considers the results of the tokens with top_p probability mass.
    /// So 0.1 means only the tokens comprising the top 10% probability mass are considered.
    /// We generally recommend altering this or temperature but not both.
    pub top_p: Option<f32>,
    /// `number` `Optional` `Defaults to 1`
    ///
    /// How many chat completion choices to generate for each input message.
    pub n: Option<u32>,
    /// `boolean` `Optional` `Defaults to false`
    ///
    /// If set, partial message deltas will be sent, like in ChatGPT.
    /// Tokens will be sent as data-only server-sent events as they become available,
    /// with the stream terminated by a data: `[DONE]` message.
    pub stream: Option<bool>,
    /// `string or array` `Optional` `Defaults to null`
    ///
    /// Up to 4 sequences where the API will stop generating further tokens.
    pub stop: Option<Vec<String>>,
    /// `integer` `Optional` `Defaults to inf`
    ///
    /// The maximum number of tokens allowed for the generated answer.
    /// By default, the number of tokens the model can return will be (4096 - prompt tokens).
    pub max_tokens: Option<u32>,
    /// `number` `Optional` `Defaults to 0`
    ///
    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on whether they appear in the text so far,
    /// increasing the model's likelihood to talk about new topics.
    pub presence_penalty: Option<f32>,
    /// `number` `Optional` `Defaults to 0`
    ///
    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on their existing frequency in the text so far,
    /// decreasing the model's likelihood to repeat the same line verbatim.
    pub frequency_penalty: Option<f32>,
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    System,
    Assistant,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResponseChatMessage {
    pub role: Option<Role>,
    pub content: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}
#[derive(Debug, Deserialize, Serialize)]
struct ChatCompletion {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatChoice {
    delta: ResponseChatMessage,
    index: u32,
    finish_reason: Option<String>,
}

#[derive(Clone)]
pub struct ChatGPT {
    pub chat: Arc<RwLock<Chat>>,
    client: Arc<MultiClient>,
    api_key: String,

    pub pending_generate: Arc<RwLock<Option<ResponseChatMessage>>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl ChatGPT {
    const URL: &'static str = "https://api.openai.com/v1/chat/completions";
    const DEFAULT_MODEL: &'static str = "gpt-3.5-turbo";

    pub fn new() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set");
        Self {
            chat: Arc::new(RwLock::new(Chat {
                model: Self::DEFAULT_MODEL.to_string(),
                messages: Vec::new(),
                temperature: Some(1.),
                top_p: Some(1.),
                n: Some(1),
                stream: Some(true),
                stop: None,
                max_tokens: None,
                presence_penalty: Some(0.),
                frequency_penalty: Some(0.),
            })),
            api_key,
            client: Arc::new(MultiClient::new()),
            pending_generate: Arc::new(RwLock::new(None)),
        }
    }
    pub async fn set_max_tokens(&mut self, max_tokens: Option<u32>) {
        self.chat.write().await.max_tokens = max_tokens;
    }
    pub async fn set_temperature(&mut self, temperature: f32) {
        self.chat.write().await.temperature = Some(temperature);
    }
    pub async fn set_presence_penalty(&mut self, presence_penalty: f32) {
        self.chat.write().await.presence_penalty = Some(presence_penalty);
    }
    pub async fn set_frequency_penalty(&mut self, frequency_penalty: f32) {
        self.chat.write().await.frequency_penalty = Some(frequency_penalty);
    }
    pub async fn set_top_p(&mut self, top_p: f32) {
        self.chat.write().await.top_p = Some(top_p);
    }
    pub async fn set_model(&mut self, model: String) {
        self.chat.write().await.model = model;
    }
    pub async fn clear_message(&mut self) {
        self.chat.write().await.messages.clear();
    }
    async fn add_message(&mut self, message: ChatMessage) {
        self.chat.write().await.messages.push(message);
    }
    pub async fn system(&mut self, system_message: String) {
        self.add_message(ChatMessage {
            role: Role::System,
            content: system_message,
        })
        .await;
    }
    pub async fn question(&mut self, question: String) -> Result<String, anyhow::Error> {
        self.add_message(ChatMessage {
            role: Role::User,
            content: question,
        })
        .await;
        let stream = self.completion().await?;

        let answer = stream.try_collect().await?;

        Ok(answer)
    }

    async fn completion(
        &mut self,
    ) -> Result<impl Stream<Item = Result<String, anyhow::Error>>, anyhow::Error> {
        *self.pending_generate.write().await = Some(ResponseChatMessage {
            role: None,
            content: None,
        });
        let uri: Uri = Self::URL.parse()?;

        let body = Body::from(serde_json::to_string(&self.chat.write().await.clone())?);

        let mut request_body = Request::new(body);

        *request_body.method_mut() = hyper::Method::POST;
        *request_body.uri_mut() = uri.clone();

        request_body
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        request_body.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key)).unwrap(),
        );

        let mut response = self.client.request(request_body).await?;

        let (sender, receiver) = mpsc::channel::<Result<String, anyhow::Error>>(100);

        let mut _self = self.clone();

        tokio::spawn(async move {
            while let Some(chunk) = response.body_mut().data().await {
                match chunk {
                    Ok(chunk) => {
                        for raw in std::str::from_utf8(&chunk)
                            .unwrap()
                            .split("data: ")
                            .filter_map(|v| v.trim().is_empty().not().then_some(v))
                        {
                            if raw.starts_with("[DONE]") {
                                if let Some(pending_generate) =
                                    _self.pending_generate.clone().write().await.take()
                                {
                                    _self
                                        .add_message(ChatMessage {
                                            role: pending_generate.role.unwrap(),
                                            content: pending_generate.content.unwrap(),
                                        })
                                        .await;
                                }
                                break;
                            }
                            match serde_json::from_str::<ChatCompletion>(&raw) {
                                Ok(chat_completion) => {
                                    if let Some(pending_generate) =
                                        _self.pending_generate.write().await.as_mut()
                                    {
                                        let message = &chat_completion.choices[0].delta;
                                        if let Some(role) = &message.role {
                                            pending_generate.role.replace(role.clone());
                                        }
                                        if let Some(content) = &message.content {
                                            if content == "\n\n" {
                                                continue;
                                            }
                                            if let Some(old_content) =
                                                pending_generate.content.as_mut()
                                            {
                                                old_content.push_str(content);
                                            } else {
                                                pending_generate.content.replace(content.clone());
                                            }
                                            sender.send(Ok(content.clone())).await.unwrap();
                                        }
                                    }
                                }
                                Err(e) => {
                                    sender.send(Err(e.into())).await.unwrap();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        sender.send(Err(e.into())).await.unwrap();
                        break;
                    }
                }
            }
        });
        Ok(ReceiverStream::new(receiver))
    }
}
