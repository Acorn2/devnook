//! System registration and bounded plist parsing; never loads application code.
use std::{ffi::c_void, path::PathBuf, ptr};

type Ref = *const c_void;
const UTF8: u32 = 0x0800_0100;
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: Ref);
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFStringCreateWithBytes(a: Ref, bytes: *const u8, len: isize, enc: u32, ext: u8) -> Ref;
    fn CFStringGetCString(value: Ref, buffer: *mut u8, size: isize, enc: u32) -> u8;
    fn CFDataCreate(a: Ref, bytes: *const u8, len: isize) -> Ref;
    fn CFPropertyListCreateWithData(
        a: Ref,
        data: Ref,
        options: usize,
        format: Ref,
        error: Ref,
    ) -> Ref;
    fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
    fn CFArrayGetCount(array: Ref) -> isize;
    fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    fn CFURLGetFileSystemRepresentation(url: Ref, resolve: u8, bytes: *mut u8, len: isize) -> u8;
}
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn LSCopyApplicationURLsForBundleIdentifier(identifier: Ref, error: Ref) -> Ref;
}
struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: Owned only wraps one owned Create/Copy result.
            unsafe { CFRelease(self.0) };
        }
    }
}
fn string(value: &str) -> Owned {
    // SAFETY: the byte slice stays alive throughout this copying call.
    Owned(unsafe {
        CFStringCreateWithBytes(ptr::null(), value.as_ptr(), value.len() as isize, UTF8, 0)
    })
}
unsafe fn read_string(value: Ref) -> Option<String> {
    if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
        return None;
    }
    let mut bytes = vec![0; 8192];
    if CFStringGetCString(value, bytes.as_mut_ptr(), bytes.len() as isize, UTF8) == 0 {
        return None;
    }
    let end = bytes.iter().position(|b| *b == 0)?;
    String::from_utf8(bytes[..end].into()).ok()
}
pub fn metadata(bytes: &[u8]) -> Result<super::BundleMetadata, String> {
    // SAFETY: CFData copies bounded bytes. Dictionary values stay borrowed only
    // while the immutable property list is alive. No executable is loaded.
    unsafe {
        let data = Owned(CFDataCreate(
            ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
        ));
        if data.0.is_null() {
            return Err("无法读取应用清单".into());
        }
        let plist = Owned(CFPropertyListCreateWithData(
            ptr::null(),
            data.0,
            0,
            ptr::null(),
            ptr::null(),
        ));
        if plist.0.is_null() || CFGetTypeID(plist.0) != CFDictionaryGetTypeID() {
            return Err("应用 Info.plist 无效".into());
        }
        let get = |key: &str| read_string(CFDictionaryGetValue(plist.0, string(key).0));
        if get("CFBundlePackageType").as_deref() != Some("APPL") {
            return Err("请选择 macOS 应用程序包".into());
        }
        Ok(super::BundleMetadata {
            bundle_id: get("CFBundleIdentifier").ok_or("应用缺少 Bundle ID")?,
            name: get("CFBundleDisplayName").or_else(|| get("CFBundleName")),
            executable: get("CFBundleExecutable").ok_or("应用缺少可执行文件")?,
            version: get("CFBundleShortVersionString").unwrap_or_default(),
        })
    }
}
pub fn registered_paths(bundle_id: &str) -> Vec<PathBuf> {
    // SAFETY: LaunchServices returns an owned array of borrowed CFURLs. Copy
    // paths before releasing it. No app is launched or loaded.
    unsafe {
        let identifier = string(bundle_id);
        if identifier.0.is_null() {
            return Vec::new();
        }
        let urls = Owned(LSCopyApplicationURLsForBundleIdentifier(
            identifier.0,
            ptr::null(),
        ));
        if urls.0.is_null() {
            return Vec::new();
        }
        let mut paths = Vec::new();
        for i in 0..CFArrayGetCount(urls.0).min(32) {
            let mut bytes = vec![0; 4096];
            let url = CFArrayGetValueAtIndex(urls.0, i);
            if CFURLGetFileSystemRepresentation(url, 1, bytes.as_mut_ptr(), bytes.len() as isize)
                != 0
            {
                if let Some(end) = bytes.iter().position(|b| *b == 0) {
                    if let Ok(path) = String::from_utf8(bytes[..end].into()) {
                        paths.push(PathBuf::from(path));
                    }
                }
            }
        }
        paths
    }
}
