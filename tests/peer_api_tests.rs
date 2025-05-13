#[cfg(test)]
mod tests {
    use p2p_server::connection::Connection;
    use p2p_server::db::P2PDatabase;
    use p2p_server::packets::{
        PeerWaitConnection, Protocol, Status, TransportData, TransportPacket,
    };
    use p2p_server::peer::peer_api::PeerAPI;
    use std::sync::Arc;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    // #[tokio::test]
    // async fn test_peer_api_creation() {
    //     let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    //     let addr = listener.local_addr().unwrap();

    //     let db = Arc::new(P2PDatabase::new(":memory:").unwrap());
    //     let peer_id = db.get_or_create_peer_id().unwrap();

    //     let connection = Connection::new(
    //         "127.0.0.1".to_string(),
    //         addr.port() as i64,
    //         "127.0.0.1".to_string(),
    //         8080,
    //         &db,
    //     )
    //     .await;

    //     let (mut socket, _) = listener.accept().await.unwrap();

    //     let peer_api = PeerAPI::new(
    //         Arc::new(connection.clone()),
    //         Arc::new("127.0.0.1:8080".to_string()),
    //         &db,
    //     );

    //     // Проверяем, что PeerAPI создан
    //     assert!(true);
    // }

    // #[tokio::test]
    // async fn test_peer_api_wait_connection() {
    //     let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    //     let addr = listener.local_addr().unwrap();

    //     let db = Arc::new(P2PDatabase::new(":memory:").unwrap());
    //     let peer_id = db.get_or_create_peer_id().unwrap();

    //     let connection = Connection::new(
    //         "127.0.0.1".to_string(),
    //         addr.port() as i64,
    //         "127.0.0.1".to_string(),
    //         8080,
    //         &db,
    //     )
    //     .await;

    //     let (mut socket, _) = listener.accept().await.unwrap();

    //     let peer_api = PeerAPI::new(
    //         Arc::new(connection.clone()),
    //         Arc::new("127.0.0.1:8080".to_string()),
    //         &db,
    //     );

    //     // Тестируем ожидание соединения
    //     let wait_conn = PeerWaitConnection {
    //         connect_peer_id: "other_peer".to_string(),
    //         public_addr: "".to_string()
    //     };

    //     let packet = TransportPacket {
    //         act: "wait_connection".to_string(),
    //         to: None,
    //         data: Some(TransportData::PeerWaitConnection(wait_conn)),
    //         status: Some(Status::SUCCESS),
    //         protocol: Protocol::SIGNAL,
    //         uuid: peer_id.clone(),
    //     };

    //     // Отправляем пакет через соединение
    //     connection.send_packet(packet.clone()).await.unwrap();

    //     // Проверяем получение пакета на сервере
    //     let mut len_bytes = [0u8; 4];
    //     socket.read_exact(&mut len_bytes).await.unwrap();
    //     let packet_len = u32::from_be_bytes(len_bytes) as usize;

    //     let mut packet_bytes = vec![0u8; packet_len];
    //     socket.read_exact(&mut packet_bytes).await.unwrap();

    //     let received_packet: TransportPacket = serde_json::from_slice(&packet_bytes).unwrap();
    //     assert_eq!(packet.act, received_packet.act);
    //     assert_eq!(packet.protocol, received_packet.protocol);
    // }
}
