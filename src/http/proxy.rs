use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn start_http_proxy(manager: Arc<ConnectionManager>, target_peer_id: String) {
    let listener = TcpListener::bind("0.0.0.0:80").await.unwrap();
    println!("HTTP proxy listening on 0.0.0.0:80");

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0; 8192];
        let n = socket.read(&mut buffer).await.unwrap();
        let request_data = &buffer[..n];

        let manager = manager.clone();
        let target_peer_id = target_peer_id.clone();

        tokio::spawn(async move {
            // Отправка запроса через TURN
            let packet = TransportPacket {
                act: "http_proxy_request".to_string(),
                to: Some(target_peer_id.clone()),
                data: Some(TransportData::Message(Message {
                    text: base64::encode(request_data),
                })),
                protocol: Protocol::TURN,
                uuid: manager.db.get_or_create_peer_id().unwrap(),
            };

            let conn = {
                let conn_map = manager.connections.lock().await;
                conn_map.get(&target_peer_id).cloned()
            };

            if let Some(conn) = conn {
                conn.send_packet(packet).await.unwrap();

                // Получение ответа
                let response_packet = conn.get_response().await.unwrap();
                if let Some(TransportData::Message(msg)) = response_packet.data {
                    let response_bytes = base64::decode(&msg.text).unwrap();
                    let _ = socket.write_all(&response_bytes).await;
                }
            }
        });
    }
}
