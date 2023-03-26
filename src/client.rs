use hyper::body::HttpBody;
use hyper::client::{HttpConnector, ResponseFuture};

use hyper::{Client, Request, Uri};
use hyper_proxy::{Intercept, Proxy, ProxyConnector};
use hyper_tls::HttpsConnector;

use std::any::Any;
use std::error::Error;
use std::{fmt::Debug, ops::Not};

use futures::Stream;
use hyper::{Body, Response};

use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug)]
pub struct MultiClient(Box<dyn Any + Send + Sync>);

impl MultiClient {
    pub fn new() -> Self {
        let https_connector = HttpsConnector::new();
        let proxy = std::env::var("HTTP_PROXY");
        #[cfg(target_os = "windows")]
        let proxy = {
            use proxyconf::internet_settings::modern::registry::{get_current_user_location, read};
            let local = get_current_user_location();
            proxy.or_else(|_| {
                let config = read(&local).map_err(|e| anyhow::anyhow!("{e}"))?;
                Ok::<String, anyhow::Error>(format!("http://{}", config.manual_proxy_address))
            })
        };
        let proxy_connector = if let Ok(proxy_uri) = proxy {
            tracing::info!("Using proxy: {}", proxy_uri);
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
        match self
            .0
            .downcast_ref::<Client<HttpsConnector<HttpConnector>, B>>()
        {
            Some(c) => c.request(req),
            None => match self
                .0
                .downcast_ref::<Client<ProxyConnector<HttpsConnector<HttpConnector>>, B>>()
            {
                Some(c) => c.request(req),
                None => panic!("Unknown client type"),
            },
        }
    }
    pub fn get(&self, uri: Uri) -> ResponseFuture {
        match self
            .0
            .downcast_ref::<Client<HttpsConnector<HttpConnector>>>()
        {
            Some(c) => c.get(uri),
            None => match self
                .0
                .downcast_ref::<Client<ProxyConnector<HttpsConnector<HttpConnector>>>>()
            {
                Some(c) => c.get(uri),
                None => panic!("Unknown client type"),
            },
        }
    }
}

pub fn fetch_sse<C>(mut response: Response<Body>) -> impl Stream<Item = Result<C, anyhow::Error>>
where
    for<'a> C: Deserialize<'a> + Debug + Send + 'static,
{
    let (sender, receiver) = mpsc::channel::<Result<C, anyhow::Error>>(100);
    tokio::spawn(async move {
        let res: Result<(), anyhow::Error> = 'stream: {
            while let Some(chunk) = response.body_mut().data().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        tracing::error!("{}", e);
                        break 'stream Err(e.into());
                    }
                };
                for raw in std::str::from_utf8(&chunk)
                    .unwrap()
                    .split("data: ")
                    .filter_map(|v| v.trim().is_empty().not().then_some(v))
                {
                    tracing::info!("received: {}", raw);
                    if raw.starts_with("[DONE]") {
                        tracing::info!("received: [DONE]");
                        break 'stream Ok(());
                    }
                    let completion = match serde_json::from_str::<C>(raw) {
                        Ok(chat_completion) => chat_completion,
                        Err(e) => {
                            tracing::error!("error: {}", e);
                            break 'stream Err(e.into());
                        }
                    };
                    if (sender.send(Ok(completion)).await).is_err() {
                        return;
                    }
                }
            }
            Ok(())
        };
        if let Err(e) = res {
            sender.send(Err(e)).await.ok();
        }
    });
    ReceiverStream::new(receiver)
}
