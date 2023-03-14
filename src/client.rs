use hyper::body::HttpBody;
use hyper::client::{HttpConnector, ResponseFuture};

use hyper::{Client, Request, Uri};
use hyper_proxy::{Intercept, Proxy, ProxyConnector};
use hyper_tls::HttpsConnector;

use std::any::Any;
use std::error::Error;

#[derive(Debug)]
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
    pub fn get(&self, uri: Uri) -> ResponseFuture {
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
