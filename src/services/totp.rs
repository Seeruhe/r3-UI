//! TOTP (Time-based One-Time Password) service for two-factor authentication

use anyhow::{anyhow, Result};
use base64::Engine;
use qrcode::QrCode;
use totp_lite::{totp_custom, Sha1};
use rand::Rng;

/// TOTP configuration
const TOTP_DIGITS: u32 = 6;
const TOTP_PERIOD: u64 = 30;

/// Generate a new random TOTP secret (base32 encoded)
pub fn generate_secret() -> String {
    // Generate 20 random bytes (160 bits) for the secret
    let mut rng = rand::thread_rng();
    let bytes: [u8; 20] = rng.gen();

    // Base32 encode (without padding)
    base32_encode(&bytes)
}

/// Base32 encode (custom implementation for TOTP compatibility)
fn base32_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut result = String::new();
    let mut bits = 0u32;
    let mut bit_count = 0;

    for &byte in data {
        bits = (bits << 8) | (byte as u32);
        bit_count += 8;
        while bit_count >= 5 {
            bit_count -= 5;
            let idx = ((bits >> bit_count) & 0x1F) as usize;
            result.push(ALPHABET[idx] as char);
        }
    }

    if bit_count > 0 {
        let idx = ((bits << (5 - bit_count)) & 0x1F) as usize;
        result.push(ALPHABET[idx] as char);
    }

    result
}

/// Base32 decode
fn base32_decode(data: &str) -> Result<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let data = data.to_uppercase().replace("=", "");

    let mut bits = 0u32;
    let mut bit_count = 0;
    let mut result = Vec::new();

    for c in data.chars() {
        let idx = ALPHABET.iter().position(|&x| x as char == c)
            .ok_or_else(|| anyhow!("Invalid base32 character: {}", c))?;

        bits = (bits << 5) | (idx as u32);
        bit_count += 5;

        while bit_count >= 8 {
            bit_count -= 8;
            result.push(((bits >> bit_count) & 0xFF) as u8);
        }
    }

    Ok(result)
}

/// Generate TOTP code from secret
pub fn generate_totp(secret: &str) -> Result<String> {
    let secret_bytes = base32_decode(secret)
        .map_err(|e| anyhow!("Failed to decode secret: {:?}", e))?;

    let code = totp_custom::<Sha1>(TOTP_PERIOD, TOTP_DIGITS, &secret_bytes, current_timestamp());
    Ok(code)
}

/// Verify a TOTP code against the secret
pub fn verify_totp(secret: &str, code: &str) -> bool {
    if secret.is_empty() || code.is_empty() {
        return false;
    }

    let secret_bytes = match base32_decode(secret) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let current_time = current_timestamp();

    // Check current, previous, and next period (allow time drift)
    for offset in [-1i64, 0, 1] {
        let time = match (current_time as i64).checked_add(offset * TOTP_PERIOD as i64) {
            Some(t) if t >= 0 => t as u64,
            _ => continue, // Skip if underflow
        };
        let expected = totp_custom::<Sha1>(TOTP_PERIOD, TOTP_DIGITS, &secret_bytes, time);
        if expected == code {
            return true;
        }
    }

    false
}

/// Generate otpauth URL for QR code
pub fn generate_otpauth_url(secret: &str, username: &str, issuer: &str) -> String {
    let encoded_issuer = urlencoding::encode(issuer);
    let encoded_username = urlencoding::encode(username);
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits={}&period={}",
        encoded_issuer,
        encoded_username,
        secret,
        encoded_issuer,
        TOTP_DIGITS,
        TOTP_PERIOD
    )
}

/// Generate QR code as base64 PNG
pub fn generate_qr_code_base64(otpauth_url: &str) -> Result<String> {
    use image::EncodableLayout;

    let code = QrCode::new(otpauth_url)
        .map_err(|e| anyhow!("Failed to generate QR code: {:?}", e))?;

    // Generate PNG image
    let png_bytes = code.render::<image::LumaA<u8>>().build();

    // Convert to base64 - use the image's raw bytes
    let base64_str = base64::engine::general_purpose::STANDARD.encode(png_bytes.as_bytes());

    Ok(base64_str)
}

/// Generate QR code as data URI
pub fn generate_qr_code_data_uri(otpauth_url: &str) -> Result<String> {
    let base64 = generate_qr_code_base64(otpauth_url)?;
    Ok(format!("data:image/png;base64,{}", base64))
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_totp() {
        let secret = generate_secret();
        assert!(!secret.is_empty());

        let code = generate_totp(&secret).unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_numeric()));

        assert!(verify_totp(&secret, &code));
    }

    #[test]
    fn test_verify_invalid_code() {
        let secret = generate_secret();
        assert!(!verify_totp(&secret, "000000"));
        assert!(!verify_totp(&secret, "123456"));
    }

    #[test]
    fn test_generate_otpauth_url() {
        let secret = "JBSWY3DPEHPK3PXP";
        let url = generate_otpauth_url(secret, "admin", "r3-UI");
        assert!(url.starts_with("otpauth://totp/"));
        assert!(url.contains("secret=JBSWY3DPEHPK3PXP"));
        assert!(url.contains("issuer=r3-UI"));
    }

    #[test]
    fn test_generate_qr_code() {
        let otpauth_url = "otpauth://totp/r3-UI:admin?secret=JBSWY3DPEHPK3PXP&issuer=r3-UI";
        let base64 = generate_qr_code_base64(otpauth_url).unwrap();
        assert!(!base64.is_empty());

        let data_uri = generate_qr_code_data_uri(otpauth_url).unwrap();
        assert!(data_uri.starts_with("data:image/png;base64,"));
    }
}
