use crate::{
    chatnexus_chat::{chat_server::Chat, ChatFilter, ChatRequest, ChatResponse, Empty},
};

use super::ChatService;

use std::{pin::Pin};

use tokio::sync::mpsc;
use tokio_stream::{ Stream};
use tonic::{ Request, Response, Status};

type ChatResult<T> = Result<Response<T>, Status>;
type ChatResponseStream = Pin<Box<dyn Stream<Item = Result<ChatResponse, Status>> + Send>>;

// Stores messages with id..

#[tonic::async_trait]
impl Chat for ChatService {
    type RecieveMessageStream = Pin<
        Box<dyn Stream<Item = Result<ChatResponse, tonic::Status>> + Send + Sync + 'static>,
    >;
    async fn send_message(&self, req: Request<ChatRequest>) -> ChatResult<Empty> {
        let data = req.get_ref();
        match self.get_chat_session(&data.session_id).await {
            Ok(val) => {
                let chat_response = ChatResponse {
                    username: val.username,
                    discriminator: val.discriminator,
                    message: data.message.to_string()
                };
                self.broadcast(chat_response).await;
                Ok(Response::new(Empty{}))
            },
            Err(_) => {Ok(Response::new(Empty{}))}, 
        }
    }

    async fn recieve_message(&self, req: Request<ChatFilter>) -> ChatResult<Self::RecieveMessageStream> {
        let (stream_tx, mut stream_rx) = mpsc::channel(1);
        let (tx, mut rx) = mpsc::channel(100);
        {
            self.senders.write().await.insert(req.get_ref().session_id.to_string(), tx);
        }
        let senders_clone = self.senders.clone();
        tokio::spawn(async move  {
            while let Some(msg) = rx.recv().await {
                match stream_tx.send(Ok(msg)).await {
                    Ok(_) => {}
                    Err(_) => {
                        senders_clone.write().await.remove(&req.get_ref().session_id.to_string());
                    }
                }
            }
        });
        
        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(stream_rx),
        )))
    }
}