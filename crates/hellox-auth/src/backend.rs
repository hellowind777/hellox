use std::path::{Path, PathBuf};

use anyhow::Result;
use hellox_config::config_root_for;

use crate::store::{load_auth_store, save_auth_store, AuthStore};

pub trait AuthStoreBackend {
    fn load(&self) -> Result<AuthStore>;
    fn save(&self, store: &AuthStore) -> Result<()>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalAuthStoreBackend {
    store_path: Option<PathBuf>,
    keys_path: Option<PathBuf>,
}

impl LocalAuthStoreBackend {
    pub fn new(store_path: Option<PathBuf>, keys_path: Option<PathBuf>) -> Self {
        Self {
            store_path,
            keys_path,
        }
    }

    pub fn from_config_path(config_path: &Path) -> Self {
        let root = config_root_for(config_path);
        Self::new(
            Some(root.join("oauth-tokens.json")),
            Some(root.join("provider-keys.json")),
        )
    }

    pub fn load_auth_store(&self) -> Result<AuthStore> {
        load_auth_store(self.store_path.clone(), self.keys_path.clone())
    }

    pub fn save_auth_store(&self, store: &AuthStore) -> Result<()> {
        save_auth_store(self.store_path.clone(), self.keys_path.clone(), store)
    }
}

impl AuthStoreBackend for LocalAuthStoreBackend {
    fn load(&self) -> Result<AuthStore> {
        self.load_auth_store()
    }

    fn save(&self, store: &AuthStore) -> Result<()> {
        self.save_auth_store(store)
    }
}
