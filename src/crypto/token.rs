use crate::packets::StorageToken;
use base64;
use hex;
use k256;
use serde_json;

pub async fn get_metadata_from_token(token: String) -> Result<StorageToken, String> {
    let token_bytes = base64::decode(&token).map_err(|e| e.to_string())?;
    let token_str = String::from_utf8(token_bytes).map_err(|e| e.to_string())?;
    let token: StorageToken = serde_json::from_str(&token_str).map_err(|e| e.to_string())?;
    Ok(token)
}

pub async fn validate_signature_token(token: String, db: &crate::db::P2PDatabase) -> Result<StorageToken, String> {
    let token_bytes = base64::decode(&token).map_err(|e| e.to_string())?;
    let token_str = String::from_utf8(token_bytes).map_err(|e| e.to_string())?;
    let token: StorageToken = serde_json::from_str(&token_str).map_err(|e| e.to_string())?;
    
    let mut signing_key = db.get_private_key().map_err(|e| e.to_string())?;
    let verifying_key = signing_key.verifying_key();
    let pub_key = verifying_key.to_encoded_point(true);
    let pub_key_hex = hex::encode(pub_key.as_bytes());

    if pub_key_hex == token.storage_provider {
        Ok(token)
    } else {
        Err("Invalid signature".to_string())
    }
}


