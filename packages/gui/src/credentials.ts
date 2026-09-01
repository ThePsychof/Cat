// Swapped from Node's scrypt (unavailable outside Node) to Web Crypto's
// PBKDF2 — the browser-native equivalent, now that this runs in a real
// webview context instead of Electron's Node-backed main process.
// Iteration count follows OWASP's 2023 Password Storage Cheat Sheet
// guidance for PBKDF2-HMAC-SHA256 (600,000+).

import { filesystem } from "@neutralinojs/lib";
import { joinPath, ensureDir } from "./neutralino-paths.js";

export interface CredentialStore {
  tokens: Record<string, string>;
}

interface EncryptedBlob {
  salt: string;
  iv: string;
  data: string; // ciphertext, with AES-GCM's auth tag already appended by SubtleCrypto
}

const CREDENTIALS_FILENAME = "credentials.enc.json";
const PBKDF2_ITERATIONS = 600_000;

let cachedKey: { passphrase: string; salt: string; key: CryptoKey } | null = null;

async function getOrDeriveKey(passphrase: string, salt: Uint8Array): Promise<CryptoKey> {
  const saltHex = bytesToHex(salt);
  if (cachedKey && cachedKey.passphrase === passphrase && cachedKey.salt === saltHex) {
    return cachedKey.key;
  }
  const key = await deriveKey(passphrase, salt);
  cachedKey = { passphrase, salt: saltHex, key };
  return key;
}

function credentialsPath(driveRoot: string): string {
  return joinPath(driveRoot, ".cat", CREDENTIALS_FILENAME);
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join("");
}

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
  }
  return bytes;
}

async function deriveKey(passphrase: string, salt: Uint8Array): Promise<CryptoKey> {
  const baseKey = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(passphrase),
    "PBKDF2",
    false,
    ["deriveKey"]
  );
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt: salt as BufferSource, iterations: PBKDF2_ITERATIONS, hash: "SHA-256" },
    baseKey,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );
}

async function encrypt(plaintext: string, passphrase: string): Promise<EncryptedBlob> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await getOrDeriveKey(passphrase, salt);

  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: iv as BufferSource },
    key,
    new TextEncoder().encode(plaintext)
  );

  return {
    salt: bytesToHex(salt),
    iv: bytesToHex(iv),
    data: bytesToHex(new Uint8Array(ciphertext)),
  };
}

async function decrypt(blob: EncryptedBlob, passphrase: string): Promise<string> {
  const salt = hexToBytes(blob.salt);
  const iv = hexToBytes(blob.iv);
  const data = hexToBytes(blob.data);
  const key = await getOrDeriveKey(passphrase, salt);

  try {
    const plaintext = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: iv as BufferSource },
      key,
      data as BufferSource
    );
    return new TextDecoder().decode(plaintext);
  } catch {
    throw new Error("Failed to decrypt credentials — wrong passphrase or corrupted file.");
  }
}

export async function readCredentials(
  driveRoot: string,
  passphrase: string
): Promise<CredentialStore> {
  const p = credentialsPath(driveRoot);
  try {
    const raw = await filesystem.readFile(p);
    const blob = JSON.parse(raw) as EncryptedBlob;
    const plaintext = await decrypt(blob, passphrase);
    return JSON.parse(plaintext) as CredentialStore;
  } catch (err) {
    if (err instanceof Error && err.message.includes("decrypt")) {
      throw err;
    }
    return { tokens: {} };
  }
}

export async function writeCredentials(
  driveRoot: string,
  store: CredentialStore,
  passphrase: string
): Promise<void> {
  const p = credentialsPath(driveRoot);
  await ensureDir(joinPath(driveRoot, ".cat"));
  const blob = await encrypt(JSON.stringify(store), passphrase);
  await filesystem.writeFile(p, JSON.stringify(blob, null, 2));
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