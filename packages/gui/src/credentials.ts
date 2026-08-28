import { promises as fs } from "node:fs";
import path from "node:path";
import crypto from "node:crypto";

export interface CredentialStore {
  tokens: Record<string, string>;
}

interface EncryptedBlob {
  salt: string;    // hex
  iv: string;      // hex
  authTag: string; // hex
  data: string;    // hex (ciphertext)
}

const CREDENTIALS_FILENAME = "credentials.enc.json";
const SCRYPT_KEYLEN = 32;

function credentialsPath(driveRoot: string): string {
  return path.join(driveRoot, ".cat", CREDENTIALS_FILENAME);
}

function deriveKey(passphrase: string, salt: Buffer): Buffer {
  return crypto.scryptSync(passphrase, salt, SCRYPT_KEYLEN);
}

function encrypt(plaintext: string, passphrase: string): EncryptedBlob {
  const salt = crypto.randomBytes(16);
  const key = deriveKey(passphrase, salt);
  const iv = crypto.randomBytes(12);

  const cipher = crypto.createCipheriv("aes-256-gcm", key, iv);
  const encrypted = Buffer.concat([cipher.update(plaintext, "utf-8"), cipher.final()]);
  const authTag = cipher.getAuthTag();

  return {
    salt: salt.toString("hex"),
    iv: iv.toString("hex"),
    authTag: authTag.toString("hex"),
    data: encrypted.toString("hex"),
  };
}

function decrypt(blob: EncryptedBlob, passphrase: string): string {
  const salt = Buffer.from(blob.salt, "hex");
  const key = deriveKey(passphrase, salt);
  const iv = Buffer.from(blob.iv, "hex");
  const authTag = Buffer.from(blob.authTag, "hex");
  const data = Buffer.from(blob.data, "hex");

  const decipher = crypto.createDecipheriv("aes-256-gcm", key, iv);
  decipher.setAuthTag(authTag);

  const decrypted = Buffer.concat([decipher.update(data), decipher.final()]);
  return decrypted.toString("utf-8");
}

export async function readCredentials(
  driveRoot: string,
  passphrase: string
): Promise<CredentialStore> {
  try {
    const raw = await fs.readFile(credentialsPath(driveRoot), "utf-8");
    const blob = JSON.parse(raw) as EncryptedBlob;
    const plaintext = decrypt(blob, passphrase);
    return JSON.parse(plaintext) as CredentialStore;
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return { tokens: {} };
    }
    // Wrong passphrase (auth tag mismatch) or corrupt file both land here.
    throw new Error("Failed to decrypt credentials — wrong passphrase or corrupted file.");
  }
}

export async function writeCredentials(
  driveRoot: string,
  store: CredentialStore,
  passphrase: string
): Promise<void> {
  const p = credentialsPath(driveRoot);
  await fs.mkdir(path.dirname(p), { recursive: true });
  const blob = encrypt(JSON.stringify(store), passphrase);
  await fs.writeFile(p, JSON.stringify(blob, null, 2), "utf-8");
}

export async function setToken(
  driveRoot: string,
  profileName: string,
  token: string,
  passphrase: string
): Promise<void> {
  const store = await readCredentials(driveRoot, passphrase);
  store.tokens[profileName] = token;
  await writeCredentials(driveRoot, store, passphrase);
}

export async function getToken(
  driveRoot: string,
  profileName: string,
  passphrase: string
): Promise<string | null> {
  const store = await readCredentials(driveRoot, passphrase);
  return store.tokens[profileName] ?? null;
}