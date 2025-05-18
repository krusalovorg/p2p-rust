use super::ConnectionManager::ConnectionManager;
use crate::connection::Connection;
use crate::crypto::token::validate_signature_token;
use crate::db::{P2PDatabase, Storage};
use crate::packets::{
    EncryptedData, FileData, Message, PeerFileGet, PeerFileSaved, PeerUploadFile, Protocol,
    TransportData, TransportPacket, PeerFileAccessChange, PeerFileDelete, PeerFileMove,
};
use base64;
use colored::*;
use serde_json;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
            println!(
                "Token info: {}. Token in request: {}",
                token_info.token, data.token
            );
            return Err("Токен в запросе не совпадает с токеном в базе данных".to_string());
        }

        let validated_token = validate_signature_token(data.token.clone(), &self.db)
            .await
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

        let free_space = self
            .db
            .get_storage_free_space()
            .await
            .map_err(|e| format!("Ошибка при получении свободного места: {:?}", e))?;

        if free_space < contents.len() as u64 {
            return Err(format!(
                "Недостаточно свободного места. Требуется: {}, Доступно: {}",
                contents.len(),
                free_space
            ));
        }

        let dir_path: String = format!("{}/blobs", self.db.path.as_str());
        if !std::path::Path::new(&dir_path).exists() {
            tokio::fs::create_dir_all(&dir_path)
                .await
                .map_err(|e| format!("Ошибка при создании директории: {}", e))?;
        }

        let final_contents = if data.compressed && data.auto_decompress {
            println!("Распаковка сжатых данных...");
            self.db
                .uncompress_data(&contents)
                .map_err(|e| format!("Ошибка распаковки: {}", e))?
        } else {
            contents
        };

        let path = format!("{}/{}", dir_path, data.file_hash);
        let mut file = File::create(&path)
            .await
            .map_err(|e| format!("Ошибка при создании файла: {}", e))?;
        file.write_all(&final_contents)
            .await
            .map_err(|e| format!("Ошибка при записи файла: {}", e))?;

        let peer_id = data.peer_id.clone();

        let file_is_compressed = if data.auto_decompress {
            false
        } else {
            data.compressed
        };

        self.db
            .add_storage_fragment(Storage {
                file_hash: data.file_hash.clone(),
                filename: data.filename.clone(),
                token: data.token.clone(),
                uploaded_via_token: Some(data.token.clone()),
                owner_key: peer_id.clone(),
                storage_peer_key: self.db.get_or_create_peer_id().unwrap(),
                mime: data.mime.clone(),
                public: data.public,
                encrypted: data.encrypted,
                compressed: file_is_compressed,
                auto_decompress: data.auto_decompress,
                size: final_contents.len() as u64,
            })
            .map_err(|e| format!("Ошибка при добавлении информации о фрагменте: {}", e))?;

        println!("{}", "[Peer] Файл успешно сохранен".green());

        let packet_feedback = TransportPacket {
            act: "file_saved".to_string(),
            to: Some(from_uuid),
            data: Some(TransportData::PeerFileSaved(PeerFileSaved {
                filename: data.filename,
                token: data.token,
                storage_peer_key: self.db.get_or_create_peer_id().unwrap(),
                owner_key: peer_id.clone(),
                hash_file: data.file_hash,
                encrypted: data.encrypted,
                compressed: file_is_compressed,
                auto_decompress: data.auto_decompress,
                public: data.public,
                size: final_contents.len() as u64,
                mime: data.mime,
            })),
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        connection
            .send_packet(packet_feedback)
            .await
            .map_err(|e| format!("Ошибка при отправке подтверждения: {}", e))
    }

    pub async fn handle_file_saved(&self, data: PeerFileSaved) -> Result<(), String> {
        let token_clone = data.token.clone().to_string();
        let _ = self.db.add_storage_fragment(Storage {
            file_hash: data.hash_file.clone(),
            filename: data.filename.clone(),
            token: token_clone.clone(),
            uploaded_via_token: Some(data.token.clone()),
            owner_key: data.owner_key.clone(),
            storage_peer_key: data.storage_peer_key.clone(),
            mime: data.mime.clone(),
            public: data.public,
            encrypted: data.encrypted,
            compressed: data.compressed,
            auto_decompress: data.auto_decompress,
            size: data.size,
        });

        println!(
            "{}",
            format!("\x1b[32m[Peer] File saved. Hash: {}\x1b[0m", data.hash_file).green()
        );
        Ok(())
    }

    pub async fn handle_file_get(
        &self,
        data: PeerFileGet,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let contents = self
            .db
            .search_fragment_in_virtual_storage(&data.file_hash, None);

        if let Some(fragment) = contents.unwrap().first() {
            if !fragment.public
                && fragment.uploaded_via_token.is_some()
                && data.token.as_ref().map_or(false, |t| t != &fragment.token)
            {
                let packet_feedback = TransportPacket {
                    act: "file_get".to_string(),
                    to: Some(from_uuid.clone()),
                    data: Some(TransportData::Message(Message {
                        text: "Token is not valid".to_string(),
                        nonce: None,
                    })),
                    protocol: Protocol::TURN,
                    uuid: self.db.get_or_create_peer_id().unwrap(),
                    nodes: vec![],
                };

                connection
                    .send_packet(packet_feedback)
                    .await
                    .map_err(|e| e.to_string())?;
                return Err("Token is not valid".to_string());
            }

            let dir_path = format!("{}/blobs", self.db.path.as_str());
            let path = format!("{}/{}", dir_path, fragment.file_hash);
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
                    hash_file: fragment.file_hash.clone(),
                    encrypted: fragment.encrypted,
                    compressed: fragment.compressed,
                    public: fragment.public,
                    auto_decompress: fragment.auto_decompress,
                })),
                protocol: Protocol::TURN,
                uuid: self.db.get_or_create_peer_id().unwrap(),
                nodes: vec![],
            };

            println!(
                "{}",
                format!("[Peer] Sending file: {}", fragment.filename.clone()).cyan()
            );
            connection
                .send_packet(packet_file)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn handle_file_data(&self, data: FileData) -> Result<(), String> {
        let dir_path: String = format!("{}/recive_files", self.db.path.as_str());
        if !std::path::Path::new(&dir_path).exists() {
            tokio::fs::create_dir_all(&dir_path).await.unwrap();
        }

        println!("Filename: {:?}", data.filename);
        println!("File hash: {:?}", data.hash_file);
        println!("File encrypted: {:?}", data.encrypted);
        println!("File compressed: {:?}", data.compressed);
        println!("File public: {:?}", data.public);

        let path = format!("{}/{}", dir_path, data.filename);
        let mut file = File::create(path).await.unwrap();

        let contents = base64::decode(data.contents).unwrap();
        println!("Contents: {:?}", contents);

        let final_contents = if data.encrypted {
            let json_data: EncryptedData = serde_json::from_slice(&contents)
                .map_err(|e| format!("Ошибка десериализации зашифрованных данных: {}", e))?;

            let decrypted_contents = self
                .db
                .decrypt_data(&json_data.content, &json_data.nonce)
                .map_err(|e| format!("Ошибка расшифровки: {}", e))?;

            if data.compressed {
                self.db
                    .uncompress_data(&decrypted_contents)
                    .map_err(|e| format!("Ошибка распаковки: {}", e))?
            } else {
                decrypted_contents
            }
        } else {
            if data.compressed && data.auto_decompress {
                self.db
                    .uncompress_data(&contents)
                    .map_err(|e| format!("Ошибка распаковки: {}", e))?
            } else {
                contents
            }
        };

        file.write_all(&final_contents)
            .await
            .map_err(|e| format!("Ошибка записи файла: {}", e))?;

        println!(
            "{}",
            format!("[Peer] File saved: {}", data.filename).green()
        );
        Ok(())
    }

    pub async fn handle_file_access_change(
        &self,
        data: PeerFileAccessChange,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let fragments = self.db
            .search_fragment_in_virtual_storage(&data.file_hash, None)
            .map_err(|e| format!("Ошибка при поиске файла: {}", e))?;
        let fragment = fragments.first()
            .ok_or_else(|| "Файл не найден".to_string())?;

        if fragment.owner_key != from_uuid {
            return Err("У вас нет прав на изменение доступа к этому файлу".to_string());
        }

        let token_info = self.db.get_token(&data.peer_id)
            .map_err(|e| format!("Ошибка при проверке токена в базе данных: {}", e))?
            .ok_or_else(|| "Токен не найден в базе данных".to_string())?;

        if token_info.token != data.token {
            return Err("Токен в запросе не совпадает с токеном в базе данных".to_string());
        }

        let validated_token = validate_signature_token(data.token.clone(), &self.db)
            .await
            .map_err(|e| format!("Ошибка при проверке подписи токена: {}", e))?;

        self.db
            .update_fragment_public_access(&data.file_hash, data.public)
            .map_err(|e| format!("Ошибка при обновлении доступа: {}", e))?;

        println!("{}", format!("[Peer] Доступ к файлу {} изменен на {}", data.file_hash, if data.public { "публичный" } else { "приватный" }).green());

        let packet_feedback = TransportPacket {
            act: "file_access_changed".to_string(),
            to: Some(from_uuid),
            data: Some(TransportData::Message(Message {
                text: format!("Доступ к файлу {} изменен на {}", data.file_hash, if data.public { "публичный" } else { "приватный" }),
                nonce: None,
            })),
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        connection.send_packet(packet_feedback).await
    }

    pub async fn handle_file_delete(
        &self,
        data: PeerFileDelete,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let fragments = self.db
            .search_fragment_in_virtual_storage(&data.file_hash, None)
            .map_err(|e| format!("Ошибка при поиске файла: {}", e))?;
        let fragment = fragments.first()
            .ok_or_else(|| "Файл не найден".to_string())?;

        if fragment.owner_key != from_uuid {
            return Err("У вас нет прав на удаление этого файла".to_string());
        }

        let token_info = self.db.get_token(&data.peer_id)
            .map_err(|e| format!("Ошибка при проверке токена в базе данных: {}", e))?
            .ok_or_else(|| "Токен не найден в базе данных".to_string())?;

        if token_info.token != data.token {
            return Err("Токен в запросе не совпадает с токеном в базе данных".to_string());
        }

        let validated_token = validate_signature_token(data.token.clone(), &self.db)
            .await
            .map_err(|e| format!("Ошибка при проверке подписи токена: {}", e))?;

        // Удаляем физический файл
        let dir_path = format!("{}/blobs", self.db.path.as_str());
        let path = format!("{}/{}", dir_path, data.file_hash);
        if std::path::Path::new(&path).exists() {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| format!("Ошибка при удалении файла: {}", e))?;
        }

        // Удаляем запись из базы данных
        self.db
            .remove_fragment(&data.file_hash)
            .map_err(|e| format!("Ошибка при удалении записи из базы данных: {}", e))?;

        println!("{}", format!("[Peer] Файл {} успешно удален", data.file_hash).green());

        let packet_feedback = TransportPacket {
            act: "file_deleted".to_string(),
            to: Some(from_uuid),
            data: Some(TransportData::Message(Message {
                text: format!("Файл {} успешно удален", data.file_hash),
                nonce: None,
            })),
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        connection.send_packet(packet_feedback).await
    }

    pub async fn handle_file_move(
        &self,
        data: PeerFileMove,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let fragments = self.db
            .search_fragment_in_virtual_storage(&data.file_hash, None)
            .map_err(|e| format!("Ошибка при поиске файла: {}", e))?;
        let fragment = fragments.first()
            .ok_or_else(|| "Файл не найден".to_string())?;

        if fragment.owner_key != from_uuid {
            return Err("У вас нет прав на перемещение этого файла".to_string());
        }

        let token_info = self.db.get_token(&data.peer_id)
            .map_err(|e| format!("Ошибка при проверке токена в базе данных: {}", e))?
            .ok_or_else(|| "Токен не найден в базе данных".to_string())?;

        if token_info.token != data.token {
            return Err("Токен в запросе не совпадает с токеном в базе данных".to_string());
        }

        let validated_token = validate_signature_token(data.token.clone(), &self.db)
            .await
            .map_err(|e| format!("Ошибка при проверке подписи токена: {}", e))?;

        // Обновляем путь к файлу
        self.db
            .update_fragment_path(&data.file_hash, &data.new_path)
            .map_err(|e| format!("Ошибка при обновлении пути: {}", e))?;

        println!("{}", format!("[Peer] Файл {} перемещен в {}", data.file_hash, data.new_path).green());

        let packet_feedback = TransportPacket {
            act: "file_moved".to_string(),
            to: Some(from_uuid),
            data: Some(TransportData::Message(Message {
                text: format!("Файл {} успешно перемещен в {}", data.file_hash, data.new_path),
                nonce: None,
            })),
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        connection.send_packet(packet_feedback).await
    }
}
