pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, value: &str) -> Result<(), String>;
    fn get(&self, account: &str) -> Result<String, String>;
}

pub struct KeychainStore;
impl SecretStore for KeychainStore {
    fn set(&self, account: &str, value: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            security_framework::passwords::set_generic_password(
                "com.devnook.desktop.services",
                account,
                value.as_bytes(),
            )
            .map_err(|_| "无法保存敏感环境变量到 Keychain，请检查钥匙串权限".to_string())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (account, value);
            Err("敏感环境变量仅支持 macOS Keychain".into())
        }
    }
    fn get(&self, account: &str) -> Result<String, String> {
        #[cfg(target_os = "macos")]
        {
            let value = security_framework::passwords::get_generic_password(
                "com.devnook.desktop.services",
                account,
            )
            .map_err(|_| {
                "无法读取 Keychain 中的敏感环境变量，请重新配置或解锁钥匙串".to_string()
            })?;
            String::from_utf8(value).map_err(|_| "Keychain 值不是有效 UTF-8".into())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = account;
            Err("敏感环境变量仅支持 macOS Keychain".into())
        }
    }
}
