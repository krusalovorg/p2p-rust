use crate::connection::Connection;
use crate::db::{Fragment, P2PDatabase, Storage};
use super::ConnectionManager::ConnectionManager;
use crate::packets::{
    EncryptedData, FileData, PeerFileSaved, PeerUploadFile, Protocol, TransportData, TransportPacket
};
use base64;
use colored::*;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::crypto::token::{get_metadata_from_token, validate_signature_token};
use serde_json;

impl ConnectionManager {
    pub async fn handle_file_upload(
        &self,
        db: &P2PDatabase,
        data: PeerUploadFile,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        println!("Get token for peer: {}", data.peer_id);
        let token_info = self.db.get_token(&data.peer_id)
            .map_err(|e| format!("Ошибка при проверке токена в базе данных: {}", e))?
            .ok_or_else(|| "Токен не найден в базе данных. Возможно, он был отозван или истек срок его действия".to_string())?;

        if token_info.token != data.token {
            println!("Token info: {}. Token in request: {}", token_info.token, data.token);
            return Err("Токен в запросе не совпадает с токеном в базе данных".to_string());
        }

        let validated_token = validate_signature_token(data.token.clone(), &self.db).await
            .map_err(|e| format!("Ошибка при проверке подписи токена: {}", e))?;

        let contents = base64::decode(&data.contents)
            .map_err(|e| format!("Ошибка при декодировании содержимого файла: {}", e))?;
        
        if contents.len() as u64 > validated_token.file_size {
            return Err(format!(
                "Размер файла ({}) превышает разрешенный размер в токене ({})",
                contents.len(),
                validated_token.file_size
            ));
        }

        let free_space = self.db.get_storage_free_space().await
            .map_err(|e| format!("Ошибка при получении свободного места: {:?}", e))?;

        if free_space < contents.len() as u64 {
            return Err(format!(
                "Недостаточно свободного места. Требуется: {}, Доступно: {}",
                contents.len(),
                free_space
            ));
        }

        let dir_path: String = format!("{}/files", self.db.path.as_str());
        if !std::path::Path::new(&dir_path).exists() {
            tokio::fs::create_dir_all(&dir_path).await
                .map_err(|e| format!("Ошибка при создании директории: {}", e))?;
        }

        let path = format!("{}/{}", dir_path, data.filename);
        let mut file = File::create(&path).await
            .map_err(|e| format!("Ошибка при создании файла: {}", e))?;
        file.write_all(&contents).await
            .map_err(|e| format!("Ошибка при записи файла: {}", e))?;

        let peer_id = data.peer_id.clone();
        
        self.db.add_storage_fragment(Storage {
            filename: data.filename.clone(),
            token: data.token.clone(),
            owner_id: peer_id.clone(),
            storage_peer_id: self.db.get_or_create_peer_id().unwrap(),
        }).map_err(|e| format!("Ошибка при добавлении информации о фрагменте: {}", e))?;

        println!("{}", "[Peer] Файл успешно сохранен".green());

        let packet_feedback = TransportPacket {
            act: "file_saved".to_string(),
            to: Some(from_uuid),
            data: Some(TransportData::PeerFileSaved(PeerFileSaved {
                filename: data.filename,
                token: data.token,
                peer_id: self.db.get_or_create_peer_id().unwrap(),
            })),
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        connection.send_packet(packet_feedback).await
            .map_err(|e| format!("Ошибка при отправке подтверждения: {}", e))
    }

    pub async fn handle_file_saved(
        &self,
        data: PeerFileSaved,
    ) -> Result<(), String> {
        let token_clone = data.token.clone().to_string();
        let _ = self.db.add_myfile_fragment(Fragment {
            uuid_peer: data.peer_id,
            token: token_clone.clone(),
            filename: data.filename.clone(),
        });

        println!("{}", format!("\x1b[32m[Peer] File saved. Token: {}\x1b[0m", token_clone).green());
        Ok(())
    }

    pub async fn handle_file_get(
        &self,
        token: String,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let contents = self.db.get_storage_fragments_by_key(&token);
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
                protocol: Protocol::TURN,
                uuid: self.db.get_or_create_peer_id().unwrap(),
                nodes: vec![],
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
        let json_data: EncryptedData = serde_json::from_slice(&contents).unwrap();
        let decrypted_contents = self.db.decrypt_data(&json_data.content, &json_data.nonce).unwrap();
        let uncompressed_contents = self.db.uncompress_data(&decrypted_contents).unwrap();
        file.write_all(&uncompressed_contents).await.unwrap();
        println!("{}", format!("[Peer] File saved: {}", data.filename).green());
        Ok(())
    }
} 