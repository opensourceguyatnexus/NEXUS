// Windows Credential Manager entries that are part of a launcher's login.
// Saved copies are encrypted for the current Windows user before they touch disk.

use super::*;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST, CRED_TYPE_GENERIC,
};
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

pub(super) const CRED_FILE: &str = "credential.bin";

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub(super) struct SavedCred {
    pub(super) user: String,
    pub(super) persist: u32,
    pub(super) blob: String,
}

impl SavedCred {
    pub(super) fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub(super) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(super) fn read_cred(target: &str) -> Option<SavedCred> {
    let name = wide(target);
    let mut found: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, 0, &mut found).ok()?;
        let cred = &*found;
        let blob = if cred.CredentialBlob.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize)
        };
        let saved = SavedCred {
            user: if cred.UserName.is_null() {
                String::new()
            } else {
                cred.UserName.to_string().unwrap_or_default()
            },
            persist: cred.Persist.0,
            blob: STANDARD.encode(blob),
        };
        CredFree(found as *const std::ffi::c_void);
        Some(saved)
    }
}

pub(super) fn write_cred(target: &str, saved: &SavedCred) -> Result<(), String> {
    let mut name = wide(target);
    let mut user = wide(&saved.user);
    let mut blob = STANDARD
        .decode(&saved.blob)
        .map_err(|_| "The saved login is damaged.".to_string())?;
    let cred = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(name.as_mut_ptr()),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST(saved.persist),
        UserName: PWSTR(user.as_mut_ptr()),
        ..Default::default()
    };
    unsafe { CredWriteW(&cred, 0) }.map_err(|e| format!("Cannot write the saved login to Windows: {}", e))
}

pub(super) fn delete_cred(target: &str) {
    let name = wide(target);
    // Already missing is the wanted result.
    let _ = unsafe { CredDeleteW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, 0) };
}

// Saved copies of a credential are encrypted for this Windows user.
pub(super) fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    crypt(data, true).ok_or_else(|| "Windows could not protect the saved login.".to_string())
}

pub(super) fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    crypt(data, false).ok_or_else(|| "Windows could not read the saved login.".to_string())
}

pub(super) fn crypt(data: &[u8], encrypt: bool) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if encrypt {
            CryptProtectData(&input, PCWSTR::null(), None, None, None, 0, &mut output).ok()?;
        } else {
            CryptUnprotectData(&input, None, None, None, None, 0, &mut output).ok()?;
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData as *mut std::ffi::c_void));
        Some(bytes)
    }
}

// Make the live credential match a snapshot; one with no credential removes it.
pub(super) fn restore_cred(s: &Spec, snapshot: Option<&Path>) -> Result<(), String> {
    let Some(target) = s.cred else {
        return Ok(());
    };
    let Some(bytes) = snapshot.and_then(|dir| fs::read(dir.join(CRED_FILE)).ok()) else {
        delete_cred(target);
        return Ok(());
    };
    let saved: SavedCred = serde_json::from_slice(&unprotect(&bytes)?)
        .map_err(|_| "The saved login is damaged.".to_string())?;
    write_cred(target, &saved)
}
