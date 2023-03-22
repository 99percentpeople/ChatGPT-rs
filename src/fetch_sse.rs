use std::{fmt::Debug, ops::Not};

use futures::Stream;
use hyper::{body::HttpBody, Body, Response};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

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
                    let completion = match serde_json::from_str::<C>(&raw) {
                        Ok(chat_completion) => chat_completion,
                        Err(e) => {
                            tracing::error!("error: {}", e);
                            break 'stream Err(e.into());
                        }
                    };
                    sender.send(Ok(completion)).await.unwrap();
                }
            }
            Ok(())
        };
        if let Err(e) = res {
            sender.send(Err(e)).await.unwrap();
        }
    });
    ReceiverStream::new(receiver)
}
