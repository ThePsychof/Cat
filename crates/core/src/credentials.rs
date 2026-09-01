use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CredentialStore {
    pub tokens: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct EncryptedBlob {
    salt: String,
    nonce: String,
    data: String,
}

fn credentials_path(drive_root: &Path) -> PathBuf {
    drive_root.join(".cat").join("credentials.enc.json")
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("key derivation failed: {e}"))?;
    Ok(key)
}

fn encrypt(plaintext: &str, passphrase: &str) -> Result<EncryptedBlob, String> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("encryption failed: {e}"))?;

    Ok(EncryptedBlob {
        salt: hex::encode(salt),
        nonce: hex::encode(nonce_bytes),
        data: hex::encode(ciphertext),
    })
}

fn decrypt(blob: &EncryptedBlob, passphrase: &str) -> Result<String, String> {
    let salt = hex::decode(&blob.salt).map_err(|e| e.to_string())?;
    let nonce_bytes = hex::decode(&blob.nonce).map_err(|e| e.to_string())?;
    let data = hex::decode(&blob.data).map_err(|e| e.to_string())?;

    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, data.as_ref())
        .map_err(|_| "Failed to decrypt credentials — wrong passphrase or corrupted file.".to_string())?;

    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

pub fn read_credentials(drive_root: &Path, passphrase: &str) -> Result<CredentialStore, String> {
    let path = credentials_path(drive_root);
    if !path.exists() {
        return Ok(CredentialStore::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let blob: EncryptedBlob =
        serde_json::from_str(&raw).map_err(|e| format!("Failed to parse credentials: {e}"))?;
    let plaintext = decrypt(&blob, passphrase)?;
    serde_json::from_str(&plaintext).map_err(|e| format!("Failed to parse decrypted credentials: {e}"))
}

pub fn write_credentials(
    drive_root: &Path,
    store: &CredentialStore,
    passphrase: &str,
) -> Result<(), String> {
    let path = credentials_path(drive_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    let plaintext = serde_json::to_string(store).map_err(|e| format!("Failed to encode credentials: {e}"))?;
    let blob = encrypt(&plaintext, passphrase)?;
    let raw = serde_json::to_string_pretty(&blob).map_err(|e| format!("Failed to encode blob: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

pub fn set_token(
    drive_root: &Path,
    profile_name: &str,
    token: &str,
    passphrase: &str,
) -> Result<(), String> {
    let mut store = read_credentials(drive_root, passphrase)?;
    store.tokens.insert(profile_name.to_string(), token.to_string());
    write_credentials(drive_root, &store, passphrase)
}

pub fn get_token(
    drive_root: &Path,
    profile_name: &str,
    passphrase: &str,
) -> Result<Option<String>, String> {
    let store = read_credentials(drive_root, passphrase)?;
    Ok(store.tokens.get(profile_name).cloned())
}