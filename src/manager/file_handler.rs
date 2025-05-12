use crate::connection::Connection;
use crate::db::{Fragment, Storage};
use super::ConnectionManager::ConnectionManager;
use crate::packets::{
    FileData, PeerFileSaved, PeerUploadFile, Protocol, Status, TransportData, TransportPacket
};
use base64;
use colored::*;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

impl ConnectionManager {
    pub async fn handle_file_upload(
        &self,
        data: PeerUploadFile,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let session_key = self
            .db
            .generate_and_store_secret_key(&data.peer_id)
            .unwrap();

        let contents = base64::decode(&data.contents).unwrap();
        let dir_path: String = format!("{}/files", self.db.path.as_str());
        if !std::path::Path::new(&dir_path).exists() {
            tokio::fs::create_dir_all(&dir_path).await.unwrap();
        }
        let path = format!("{}/{}", dir_path, data.filename);
        let mut file = File::create(path).await.unwrap();
        file.write_all(&contents).await.unwrap();

        let _ = self.db.add_storage_fragment(Storage {
            filename: data.filename.clone(),
            session_key: session_key.clone(),
            session: session_key.clone(),
            owner_id: data.peer_id,
            storage_peer_id: self.db.get_or_create_peer_id().unwrap(),
        });

        println!("{}", "[Peer] File saved".green());

        let packet_feedback = TransportPacket {
            act: "file_saved".to_string(),
            to: Some(from_uuid),
            data: Some(TransportData::PeerFileSaved(PeerFileSaved {
                filename: data.filename,
                session_key: session_key,
                peer_id: self.db.get_or_create_peer_id().unwrap(),
            })),
            status: None,
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        connection.send_packet(packet_feedback).await.map_err(|e| e.to_string())
    }

    pub async fn handle_file_saved(
        &self,
        data: PeerFileSaved,
    ) -> Result<(), String> {
        let session_key_clone = data.session_key.clone().to_string();
        let _ = self.db.add_myfile_fragment(Fragment {
            uuid_peer: data.peer_id,
            session_key: session_key_clone.clone(),
            session: session_key_clone.clone(),
            filename: data.filename.clone(),
        });

        println!("{}", format!("\x1b[32m[Peer] File saved. Session key: {}\x1b[0m", session_key_clone).green());
        Ok(())
    }

    pub async fn handle_file_get(
        &self,
        session_key: String,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let contents = self.db.get_storage_fragments_by_key(&session_key);
        for fragment in contents.unwrap() {
            let dir_path = format!("{}/files", self.db.path.as_str());
            let path = format!("{}/{}", dir_path, fragment.filename);
            let mut file = File::open(path).await.unwrap();
            let mut contents = vec![];
            file.read_to_end(&mut contents).await.unwrap();

            let packet_file = TransportPacket {
                act: "file".to_string(),
                to: Some(from_uuid.clone()),
                data: Some(TransportData::FileData(FileData {
                    filename: fragment.filename.clone().to_string(),
                    contents: base64::encode(contents),
                    peer_id: self.db.get_or_create_peer_id().unwrap(),
                })),
                status: Some(Status::SUCCESS),
                protocol: Protocol::TURN,
                uuid: self.db.get_or_create_peer_id().unwrap(),
            };

            println!("{}", format!("[Peer] Sending file: {}", fragment.filename.clone()).cyan());
            connection.send_packet(packet_file).await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn handle_file_data(
        &self,
        data: FileData,
    ) -> Result<(), String> {
        let dir_path: String = format!("{}/recive_files", self.db.path.as_str());
        if !std::path::Path::new(&dir_path).exists() {
            tokio::fs::create_dir_all(&dir_path).await.unwrap();
        }
        let path = format!("{}/{}", dir_path, data.filename);
        let mut file = File::create(path).await.unwrap();
        let contents = base64::decode(data.contents).unwrap();
        file.write_all(&contents).await.unwrap();
        println!("{}", format!("[Peer] File saved: {}", data.filename).green());
        Ok(())
    }
} 