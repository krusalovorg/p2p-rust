// use k256::{SecretKey, PublicKey, ecdh::SharedSecret};
// use sha2::{Sha256, Digest};
// use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
// use rand_core::OsRng;

// // private - мой
// // public - собеседника
// pub fn get_shared_secret(private: str, public: str) {
//     let shared = SharedSecret::new(private, public);
//     let hash = Sha256::digest(shared.as_bytes());
//     hash.into()
// }

// fn encrypt(plaintext: &[u8], key_bytes: [u8; 32]) -> (Vec<u8>, [u8; 12]) {
//     let key = Key::from_slice(&key_bytes);
//     let cipher = ChaCha20Poly1305::new(key);

//     let mut nonce_bytes = [0u8; 12];
//     OsRng.fill_bytes(&mut nonce_bytes);
//     let nonce = Nonce::from_slice(&nonce_bytes);

//     let ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption failure!");
//     (ciphertext, nonce_bytes)
// }

// pub fn decrypt() {
//     let key = Key::from_slice(&key_bytes);
//     let cipher = ChaCha20Poly1305::new(key);
//     let nonce = Nonce::from_slice(&nonce_bytes);

//     cipher.decrypt(nonce, ciphertext).expect("decryption failure!")
// }