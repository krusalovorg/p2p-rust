#[cfg(test)]
mod tests {
    use p2p_server::packets::{
        PeerFileSaved, PeerInfo, PeerWaitConnection, Protocol, Status, SyncPeerInfo,
        SyncPeerInfoData, TransportPacket, TransportData, Message
    };
    use p2p_server::db::P2PDatabase;
    use std::sync::Arc;

    #[test]
    fn test_peer_info_serialization() {
        let peer_info = PeerInfo {
            peer_id: "test_peer".to_string(),
        };

        let serialized = serde_json::to_string(&peer_info).unwrap();
        let deserialized: PeerInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(peer_info.peer_id, deserialized.peer_id);
    }

    #[test]
    fn test_peer_wait_connection_serialization() {
        let wait_conn = PeerWaitConnection {
            connect_peer_id: "peer2".to_string(),
            public_addr: "".to_string(),
        };

        let serialized = serde_json::to_string(&wait_conn).unwrap();
        let deserialized: PeerWaitConnection = serde_json::from_str(&serialized).unwrap();

        assert_eq!(wait_conn.connect_peer_id, deserialized.connect_peer_id);
    }

    #[test]
    fn test_peer_file_saved_serialization() {
        let file_saved = PeerFileSaved {
            peer_id: "peer1".to_string(),
            filename: "test.txt".to_string(),
            session_key: "session123".to_string(),
        };

        let serialized = serde_json::to_string(&file_saved).unwrap();
        let deserialized: PeerFileSaved = serde_json::from_str(&serialized).unwrap();

        assert_eq!(file_saved.peer_id, deserialized.peer_id);
        assert_eq!(file_saved.filename, deserialized.filename);
        assert_eq!(file_saved.session_key, deserialized.session_key);
    }

    #[test]
    fn test_sync_peer_info_serialization() {
        let peer_info = SyncPeerInfo {
            uuid: "peer1".to_string(),
        };

        let serialized = serde_json::to_string(&peer_info).unwrap();
        let deserialized: SyncPeerInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(peer_info.uuid, deserialized.uuid);
    }

    #[test]
    fn test_sync_peer_info_data_serialization() {
        let peer_info_data = SyncPeerInfoData {
            peers: vec![
                SyncPeerInfo {
                    // public_addr: "127.0.0.1:8080".to_string(),
                    uuid: "peer1".to_string(),
                },
                SyncPeerInfo {
                    // public_addr: "127.0.0.1:8081".to_string(),
                    uuid: "peer2".to_string(),
                },
            ],
        };

        let serialized = serde_json::to_string(&peer_info_data).unwrap();
        let deserialized: SyncPeerInfoData = serde_json::from_str(&serialized).unwrap();

        assert_eq!(peer_info_data.peers.len(), deserialized.peers.len());
        assert_eq!(peer_info_data.peers[0].uuid, deserialized.peers[0].uuid);
        assert_eq!(peer_info_data.peers[1].uuid, deserialized.peers[1].uuid);
    }

    #[test]
    fn test_transport_packet_display() {
        let packet = TransportPacket {
            act: "test".to_string(),
            to: Some("peer2".to_string()),
            data: Some(TransportData::Message(Message { text: "test data".to_string() })),
            protocol: Protocol::SIGNAL,
            uuid: "peer1".to_string(),
        };

        let display_str = format!("{}", packet);
        assert!(display_str.contains("public_addr: 127.0.0.1:8080"));
        assert!(display_str.contains("act: test"));
        assert!(display_str.contains("protocol: SIGNAL"));
        assert!(display_str.contains("uuid: peer1"));
    }
} 