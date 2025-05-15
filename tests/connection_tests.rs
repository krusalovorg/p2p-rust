#[cfg(test)]
mod tests {
    use async_std::path::PathBuf;
    use p2p_server::connection::Connection;
    use p2p_server::db::P2PDatabase;
    use p2p_server::packets::{TransportPacket, Protocol, Status, TransportData, Message};
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::io::AsyncReadExt;

    async fn create_temp_db() -> String {
        let path = PathBuf::from("tests/temp-db");
        if !path.exists().await {
            std::fs::create_dir_all(&path).unwrap();
        }
        let db = P2PDatabase::new(path.to_str().unwrap()).unwrap();    
        db.path.clone()
    }

    #[tokio::test]
    async fn test_connection_establishment() {
        // Создаем тестовый TCP сервер
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Создаем тестовую базу данных
        let db_path = create_temp_db().await;
        let db = Arc::new(P2PDatabase::new(&db_path).unwrap());

        // Создаем соединение
        let connection = Connection::new(
            "127.0.0.1".to_string(),
            3031,
            "127.0.0.1".to_string(),
            8080,
            &db,
        )
        .await;

        // Принимаем входящее соединение
        let (mut socket, _) = listener.accept().await.unwrap();

        // Проверяем, что соединение установлено
        assert!(connection.tx.capacity() > 0);
    }

    #[tokio::test]
    async fn test_send_and_receive_packet() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let db_path = create_temp_db().await;
        let db = Arc::new(P2PDatabase::new(&db_path).unwrap());
        let peer_id = db.get_or_create_peer_id().unwrap();

        let connection = Connection::new(
            "127.0.0.1".to_string(),
            3031,
            "127.0.0.1".to_string(),
            8080,
            &db,
        )
        .await;

        let (mut socket, _) = listener.accept().await.unwrap();

        let test_packet = TransportPacket {
            act: "test".to_string(),
            to: None,
            data: Some(TransportData::Message(Message { text: "test data".to_string() })),
            protocol: Protocol::SIGNAL,
            uuid: peer_id,
        };

        // Отправляем пакет
        connection.send_packet(test_packet.clone()).await.unwrap();

        // Читаем пакет с сервера
        let mut len_bytes = [0u8; 4];
        socket.read_exact(&mut len_bytes).await.unwrap();
        let packet_len = u32::from_be_bytes(len_bytes) as usize;

        let mut packet_bytes = vec![0u8; packet_len];
        socket.read_exact(&mut packet_bytes).await.unwrap();

        let received_packet: TransportPacket = serde_json::from_slice(&packet_bytes).unwrap();

        assert_eq!(test_packet.act, received_packet.act);
        assert_eq!(test_packet.protocol, received_packet.protocol);
    }
} 