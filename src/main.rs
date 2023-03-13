use hyper::body::HttpBody;
use hyper::client::{HttpConnector, ResponseFuture};
use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use hyper::{Body, Client, Request, Uri};
use hyper_proxy::{Intercept, Proxy, ProxyConnector};
use hyper_tls::HttpsConnector;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::error::Error;
use std::io::Write;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt};
use tokio::signal::ctrl_c;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum Role {
    User,
    System,
    Assistant,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ChatMessage {
    role: Option<Role>,
    content: Option<String>,
}
/// POST https://api.openai.com/v1/chat/completions
///
/// Creates a completion for the chat message
#[derive(Deserialize, Serialize, Debug, Clone)]
struct Chat {
    /// `string` `Required`
    ///
    /// ID of the model to use. Currently, only `gpt-3.5-turbo` and `gpt-3.5-turbo-0301` are supported.
    model: String,
    /// `array` `Required`
    /// The messages to generate chat completions for, in the chat format.
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    /// An alternative to sampling with temperature, called nucleus sampling,
    /// where the model considers the results of the tokens with top_p probability mass.
    /// So 0.1 means only the tokens comprising the top 10% probability mass are considered.
    /// We generally recommend altering this or temperature but not both.
    top_p: Option<f32>,
    /// How many chat completion choices to generate for each input message.
    n: Option<u32>,
    /// `boolean` `Optional` `Defaults to false`
    ///
    /// If set, partial message deltas will be sent, like in ChatGPT.
    /// Tokens will be sent as data-only server-sent events as they become available,
    /// with the stream terminated by a data: [DONE] message.
    stream: Option<bool>,
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
    delta: ChatMessage,
    index: u32,
    finish_reason: Option<String>,
}

#[derive(Clone)]
pub struct ChatGPT {
    chat: Arc<Mutex<Chat>>,
    client: Arc<MultiClient>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

pub struct MultiClient(Box<dyn Any + Send + Sync>);

impl MultiClient {
    pub fn new() -> Self {
        let https_connector = HttpsConnector::new();
        let proxy_connector = if let Ok(proxy_uri) = std::env::var("HTTP_PROXY") {
            let proxy_uri = proxy_uri.parse().unwrap();
            let proxy = Proxy::new(Intercept::All, proxy_uri);
            let proxy_connector =
                ProxyConnector::from_proxy(https_connector.clone(), proxy).unwrap();
            Some(proxy_connector)
        } else {
            None
        };
        let client = proxy_connector.map_or_else(
            || {
                Box::new(Client::builder().build::<_, hyper::Body>(https_connector))
                    as Box<dyn Any + Send + Sync>
            },
            |proxy| Box::new(Client::builder().build::<_, hyper::Body>(proxy)),
        );
        Self(client)
    }
    pub fn request<B>(&self, req: Request<B>) -> ResponseFuture
    where
        B: HttpBody + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
    {
        if let Some(c) = self
            .0
            .downcast_ref::<Client<HttpsConnector<HttpConnector>, B>>()
        {
            c.request(req)
        } else if let Some(c) = self
            .0
            .downcast_ref::<Client<ProxyConnector<HttpsConnector<HttpConnector>>, B>>()
        {
            c.request(req)
        } else {
            panic!("Unknown client type");
        }
    }
    pub async fn get(&self, uri: Uri) -> ResponseFuture {
        if let Some(c) = self
            .0
            .downcast_ref::<Client<HttpsConnector<HttpConnector>>>()
        {
            c.get(uri)
        } else if let Some(c) = self
            .0
            .downcast_ref::<Client<ProxyConnector<HttpsConnector<HttpConnector>>>>()
        {
            c.get(uri)
        } else {
            panic!("Unknown client type");
        }
    }
}

impl ChatGPT {
    const URL: &'static str = "https://api.openai.com/v1/chat/completions";
    const MODEL: &'static str = "gpt-3.5-turbo-0301";

    pub fn new() -> Self {
        Self {
            chat: Arc::new(Mutex::new(Chat {
                model: Self::MODEL.to_string(),
                messages: Vec::new(),
                temperature: None,
                top_p: None,
                n: None,
                stream: Some(true),
            })),
            client: Arc::new(MultiClient::new()),
        }
    }

    async fn add_message(&mut self, message: ChatMessage) {
        self.chat.lock().await.messages.push(message);
    }
    pub async fn system(&mut self, system_message: String) {
        self.add_message(ChatMessage {
            role: Some(Role::System),
            content: Some(system_message),
        })
        .await;
    }
    pub async fn question(
        &mut self,
        question: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        self.add_message(ChatMessage {
            role: Some(Role::User),
            content: Some(question),
        })
        .await;
        let stream = self.completion().await?;
        let mut out = std::io::stdout().lock();

        let answer = stream
            .map(|s| {
                out.write(s.as_bytes()).ok();
                out.flush().ok();
                s
            })
            .collect::<String>()
            .await;

        println!("\n-- end of stream --");
        Ok(answer)
    }
    async fn completion(
        &mut self,
    ) -> Result<impl Stream<Item = String>, Box<dyn std::error::Error>> {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set");
        let uri: Uri = Self::URL.parse()?;

        let body = Body::from(serde_json::to_string(&self.chat.lock().await.clone())?);

        let mut request_body = Request::new(body);

        *request_body.method_mut() = hyper::Method::POST;
        *request_body.uri_mut() = uri.clone();

        request_body
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        request_body.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap(),
        );

        let mut response = self.client.request(request_body).await?;

        let (sender, receiver) = mpsc::channel::<String>(100);

        let mut _self = self.clone();
        tokio::spawn(async move {
            let mut pending_done = ChatMessage {
                role: None,
                content: None,
            };

            while let Some(chunk) = response.body_mut().data().await {
                match chunk {
                    Ok(chunk) => {
                        for raw in std::str::from_utf8(&chunk)
                            .unwrap()
                            .split("data: ")
                            .filter_map(|v| if v.is_empty() { None } else { Some(v.trim()) })
                        {
                            if raw.starts_with("[DONE]") {
                                _self.add_message(pending_done.clone()).await;
                                break;
                            }
                            match serde_json::from_str::<ChatCompletion>(&raw) {
                                Ok(chat_completion) => {
                                    if let Some(role) = &chat_completion.choices[0].delta.role {
                                        pending_done.role.replace(role.clone());
                                    }
                                    if let Some(content) = &chat_completion.choices[0].delta.content
                                    {
                                        if let Some(old_content) = pending_done.content.as_mut() {
                                            old_content.push_str(content);
                                        } else {
                                            pending_done.content.replace(content.clone());
                                        }
                                        sender.send(content.clone()).await.unwrap();
                                    }
                                }
                                Err(e) => {
                                    println!("Error: {:?}, raw: {:#?}", e, raw);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error: {:?}", e);
                        break;
                    }
                }
            }
        });

        Ok(ReceiverStream::new(receiver))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    let mut chat = ChatGPT::new();
    if let Ok(system_message) = std::env::var("SYSTEM_MESSAGE") {
        chat.system(system_message).await;
    }
    loop {
        tokio::select! {
            line = stdin.next_line() => {
                if let Some(line) = line? {
                    chat.question(line).await?;
                }
            }
            _ = ctrl_c() => break

        }
    }
    Ok(())
}
