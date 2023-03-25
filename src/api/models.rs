use std::sync::{atomic, Arc};

use hyper::{body, header::AUTHORIZATION, http::HeaderValue, Body, Request};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::client::MultiClient;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Models {
    pub data: Vec<ModelData>,
    object: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelData {
    pub id: String,
    pub object: String,
    pub created: u32,
    pub owned_by: String,
}

#[derive(Clone)]
pub struct ModelsAPI {
    pub models: Arc<RwLock<Option<Models>>>,
    pub is_ready: Arc<atomic::AtomicBool>,
    api_key: String,
    client: Arc<MultiClient>,
}
impl ModelsAPI {
    pub fn new(api_key: String) -> Self {
        Self {
            models: Arc::new(RwLock::new(None)),
            client: Arc::new(MultiClient::new()),
            is_ready: Arc::new(atomic::AtomicBool::new(true)),
            api_key,
        }
    }
    pub async fn get_models(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.is_ready.store(false, atomic::Ordering::Relaxed);
        let mut request_body = Request::new(Body::default());
        *request_body.method_mut() = hyper::Method::GET;
        *request_body.uri_mut() = "https://api.openai.com/v1/models".parse().unwrap();
        request_body.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key)).unwrap(),
        );
        let response = self.client.request(request_body).await?;
        let body = body::to_bytes(response.into_body()).await?;
        let models: Models = serde_json::from_slice(&body)?;
        println!("{:?}", models);
        self.models.write().await.replace(models);
        self.is_ready.store(true, atomic::Ordering::Relaxed);
        Ok(())
    }
}
