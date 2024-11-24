use tokio::sync::{mpsc, oneshot};
use tokio::net::TcpStream;
use std::error::Error;

#[derive(Debug)]
pub enum Message {
    SendData(String, oneshot::Sender<String>), // (data, callback)
}

pub struct Actor {
    // MPSC channel for receiving messages
    tx: mpsc::Sender<Message>,
}

impl Actor {
    pub fn new() -> (Self, mpsc::Receiver<Message>) {
        let (tx, rx) = mpsc::channel(100); // 100 - размер буфера канала
        let actor = Actor { tx };

        // Запускаем асинхронную задачу для обработки сообщений
        tokio::spawn(Self::process_messages(rx));

        actor
    }

    async fn process_messages(mut rx: mpsc::Receiver<Message>) {
        while let Some(message) = rx.recv().await {
            match message {
                Message::SendData(data, callback) => {
                    // Здесь вы можете обработать данные, 
                    // например, отправить их через TcpStream.
                    println!("Обработка данных: {}", data);
                    // Эмулируем отправку и получение ответа
                    let response = format!("Ответ на: {}", data);

                    // Отправляем ответ обратно через `oneshot`
                    let _ = callback.send(response);
                }
            }
        }
    }

    // Метод для отправки данных
    pub async fn send_data(&self, data: String) -> Result<String, Box<dyn Error>> {
        // Создаем однократный канал
        let (tx, rx) = oneshot::channel();

        // Отправляем сообщение в актора
        self.tx.send(Message::SendData(data, tx)).await.unwrap();

        // Ждем ответ
        let response = rx.await?;

        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (actor, _) = Actor::new();

    // Отправляем сообщение актёру и ждем ответ
    let response = actor.send_data("Hello, Actor!".to_string()).await?;
    println!("Получен ответ: {}", response);

    Ok(())
}
