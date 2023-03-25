use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use hyper::{Body, Request, Uri};
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::instrument;

use crate::client::fetch_sse;
use crate::client::MultiClient;
use futures::StreamExt;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::Stream;

use super::{Param, Parameter, ParameterControl};

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
    pub messages: VecDeque<ChatMessage>,
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
#[derive(Deserialize, Serialize, Debug, Display, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    System,
    Assistant,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
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
    id: Option<String>,
    object: Option<String>,
    created: Option<u64>,
    model: Option<String>,
    choices: Option<Vec<ChatChoice>>,
    usage: Option<ChatUsage>,
    error: Option<ChatError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatChoice {
    delta: ResponseChatMessage,
    index: u32,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatError {
    message: String,
    r#type: String,
    param: Option<String>,
    code: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
#[derive(Clone, Debug)]
pub struct ChatAPI {
    pub data: Arc<RwLock<Chat>>,
    client: Arc<MultiClient>,
    api_key: Arc<RwLock<String>>,

    pub pending_generate: Arc<RwLock<Option<Result<ResponseChatMessage, anyhow::Error>>>>,
}

#[derive(Clone, Debug)]
pub struct ChatAPIBuilder {
    chat: Chat,
    api_key: String,
}

impl ChatAPIBuilder {
    pub fn new(api_key: String) -> Self {
        Self {
            chat: Chat {
                model: ChatAPI::DEFAULT_MODEL.to_string(),
                messages: VecDeque::new(),
                temperature: Some(1.),
                top_p: Some(1.),
                n: Some(1),
                stream: Some(true),
                stop: None,
                max_tokens: None,
                presence_penalty: Some(0.),
                frequency_penalty: Some(0.),
            },
            api_key,
        }
    }
    pub fn with_chat(mut self, chat: Chat) -> Self {
        self.chat = chat;
        self
    }

    pub fn build(self) -> ChatAPI {
        ChatAPI {
            data: Arc::new(RwLock::new(self.chat)),
            api_key: Arc::new(RwLock::new(self.api_key)),
            client: Arc::new(MultiClient::new()),
            pending_generate: Arc::new(RwLock::new(None)),
        }
    }
}

impl ChatAPI {
    const URL: &'static str = "https://api.openai.com/v1/chat/completions";
    const DEFAULT_MODEL: &'static str = "gpt-3.5-turbo";

    pub async fn set_model(&mut self, model: String) {
        self.data.write().await.model = model;
    }
    pub async fn clear_message(&mut self) {
        self.data.write().await.messages.clear();
    }
    pub async fn set_system_message(&self, system_message: Option<String>) {
        let mut data = self.data.write().await;
        if let Some(system_message) = system_message {
            if let Some(msg) = data.messages.front_mut() {
                if msg.role == Role::System {
                    msg.content = system_message;
                    return;
                }
            }
            data.messages.push_front(ChatMessage {
                role: Role::System,
                content: system_message,
            })
        } else {
            if let Some(msg) = data.messages.front() {
                if msg.role == Role::System {
                    data.messages.pop_front();
                }
            }
        }
    }
    pub fn get_system_message(&self) -> Option<String> {
        let data = tokio::task::block_in_place(|| self.data.blocking_read());
        if let Some(msg) = data.messages.front() {
            if msg.role == Role::System {
                return Some(msg.content.clone());
            }
        }
        None
    }
    pub fn get_api_key(&self) -> String {
        tokio::task::block_in_place(|| self.api_key.blocking_read()).clone()
    }
    pub async fn set_api_key(&self, api_key: String) {
        *self.api_key.write().await = api_key;
    }

    async fn add_message(&mut self, message: ChatMessage) {
        self.data.write().await.messages.push_back(message);
    }
    pub async fn question(&mut self, question: String) -> Result<(), anyhow::Error> {
        self.add_message(ChatMessage {
            role: Role::User,
            content: question,
        })
        .await;
        match self.generate().await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!("Error generating response: {:?}", e);
                Err(e)
            }
        }?;
        Ok(())
    }
    pub async fn remove_last(&mut self) {
        match self.data.write().await.messages.pop_back() {
            Some(v) => tracing::info!("Removed last message: {:?}", v),
            None => tracing::info!("No message to remove"),
        };
    }
    pub fn get_generate(&self) -> Option<Result<String, String>> {
        tokio::task::block_in_place(|| {
            let pending_generate = self.pending_generate.blocking_read();
            match pending_generate.as_ref() {
                Some(Ok(v)) => v.content.as_ref().map(|content| Ok(content.clone())),
                Some(Err(e)) => Some(Err(e.to_string())),
                None => None,
            }
        })
    }
    pub async fn generate(&mut self) -> Result<(), anyhow::Error> {
        *self.pending_generate.write().await = Some(Ok(ResponseChatMessage::default()));
        let mut stream = match self.complete().await {
            Ok(stream) => stream,
            Err(e) => {
                tracing::error!("Error while generating: {:?}", e);
                self.pending_generate.write().await.replace(Err(e.into()));
                return Ok(());
            }
        };
        while let Some(res) = stream.next().await {
            let mut pending_generate = self.pending_generate.write().await;
            let pending_generate = pending_generate.as_mut().unwrap().as_mut().unwrap();
            let res = match res {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Error while generating: {:?}", e);
                    self.pending_generate.write().await.replace(Err(e));
                    break;
                }
            };
            if let Some(error) = &res.error {
                tracing::error!("Error message from server: {:?}", error);
                anyhow::bail!(error.message.clone());
            }
            let Some(choices) = &res.choices else {
                continue;
            };
            let Some(first_choice) = &choices.first() else{
                continue;
            };
            let message = &first_choice.delta;
            if let Some(role) = &message.role {
                pending_generate.role.replace(role.clone());
            }
            let Some(content) = &message.content else {
                continue;
            };
            // if content == "\n\n" || content == "\n\n\n" {
            //     continue;
            // }
            if let Some(old_content) = pending_generate.content.as_mut() {
                old_content.push_str(content);
            } else {
                pending_generate.content.replace(content.clone());
            }
        }

        let message = if let Some(result) = self.pending_generate.write().await.take() {
            result?
        } else {
            anyhow::bail!("pending_generate is None");
        };
        let Some(content) = message.content else{
            anyhow::bail!("content is empty");
        };
        self.add_message(ChatMessage {
            role: Role::Assistant,
            content,
        })
        .await;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn complete(
        &self,
    ) -> Result<impl Stream<Item = Result<ChatCompletion, anyhow::Error>>, anyhow::Error> {
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
            HeaderValue::from_str(&format!("Bearer {}", self.api_key.read().await)).unwrap(),
        );

        let response = self.client.request(request_body).await?;
        let stream = fetch_sse::<ChatCompletion>(response);
        Ok(stream)
    }
}
impl ParameterControl for ChatAPI {
    fn params(&self) -> Vec<Box<dyn super::Parameter>> {
        let mut v = Vec::new();
        v.push(Box::new(Param {
            name: "max_tokens",
            range: Some((1, 2048).into()),
            store: RefCell::new(tokio::task::block_in_place(|| {
                self.data.blocking_read().max_tokens
            })),
            default: 2048.into(),
            getter: {
                let data = self.data.clone();
                Box::new(move || tokio::task::block_in_place(|| data.blocking_read().max_tokens))
            },
            setter: {
                let data = self.data.clone();
                Box::new(move |max_tokens| {
                    let data = data.clone();
                    tokio::spawn(async move {
                        data.write().await.max_tokens = max_tokens;
                    });
                })
            },
        }) as Box<dyn Parameter>);
        v.push(Box::new(Param {
            name: "temperature",
            range: Some((0., 2.).into()),
            default: (1.).into(),
            store: RefCell::new(1.),
            getter: {
                let data = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| data.blocking_read().temperature.unwrap_or(1.))
                })
            },
            setter: {
                let data = self.data.clone();
                Box::new(move |temperature| {
                    let data = data.clone();
                    tokio::spawn(async move {
                        data.write().await.temperature = Some(temperature);
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
                let data = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| data.blocking_read().top_p.unwrap_or(1.))
                })
            },
            setter: {
                let data = self.data.clone();
                Box::new(move |top_p| {
                    let data = data.clone();
                    tokio::spawn(async move {
                        data.write().await.top_p = Some(top_p);
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
                let data = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| {
                        data.blocking_read().presence_penalty.unwrap_or(0.)
                    })
                })
            },
            setter: {
                let data = self.data.clone();
                Box::new(move |presence_penalty| {
                    let data = data.clone();
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
                let data = self.data.clone();
                Box::new(move || {
                    tokio::task::block_in_place(|| {
                        data.blocking_read().frequency_penalty.unwrap_or(0.)
                    })
                })
            },
            setter: {
                let data = self.data.clone();
                Box::new(move |frequency_penalty| {
                    let data = data.clone();
                    tokio::spawn(async move {
                        data.write().await.frequency_penalty = Some(frequency_penalty);
                    });
                })
            },
        }));
        v.push(Box::new(Param {
            name: "system_message",
            range: None,
            default: self.get_system_message().into(),
            store: RefCell::new(None),
            getter: {
                let _self = self.clone();
                Box::new(move || _self.get_system_message())
            },
            setter: {
                let _self = self.clone();
                Box::new(move |system_message| {
                    let mut _self = _self.clone();
                    tokio::spawn(async move {
                        _self.set_system_message(system_message).await;
                    });
                })
            },
        }));
        v.push(Box::new(Param::<String> {
            name: "api_key",
            range: None,
            default: self.get_api_key().into(),
            store: RefCell::new(self.get_api_key().into()),
            getter: {
                let _self = self.clone();
                Box::new(move || _self.get_api_key())
            },
            setter: {
                let mut _self = self.clone();
                Box::new(move |api_key| {
                    let _self = _self.clone();
                    tokio::spawn(async move {
                        _self.set_api_key(api_key).await;
                    });
                })
            },
        }));
        v
    }
}
