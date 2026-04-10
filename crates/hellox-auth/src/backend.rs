use std::path::PathBuf;

use anyhow::Result;

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
