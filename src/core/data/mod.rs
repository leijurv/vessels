use crate::{
    core::{
        acquire,
        hal::crypto::{HashData, Hasher},
        CoreError,
    },
    kind::{Fallible, Infallible, Serde},
    replicate::Share,
    Kind,
};

use anyhow::{anyhow, Error};
use core::fmt::{self, Debug, Formatter};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

#[derive(Hash, Kind, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum(pub(crate) [u8; 32]);

impl Checksum {
    pub async fn new<T: Serialize + DeserializeOwned + Sync + Send + 'static>(
        item: &T,
    ) -> Result<Checksum, CoreError> {
        acquire::<Box<dyn Hasher>>()
            .await?
            .hash_data(item)
            .await
            .map_err(CoreError::Transport)
    }
}

impl Debug for Checksum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Checksum {}",
            self.0
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join("")
        )
    }
}

#[derive(Kind)]
pub struct Resource<T: Serialize + DeserializeOwned + Sync + Send + 'static> {
    checksum: Checksum,
    acquire: Option<Box<dyn FnOnce() -> Infallible<Serde<T>> + Sync + Send>>,
}

#[derive(Error, Kind)]
#[error("reification failed: {source}")]
pub struct ReifyError<T: Serialize + DeserializeOwned + Sync + Send + 'static> {
    #[source]
    source: Error,
    pub resource: Resource<T>,
}

impl<T: Serialize + DeserializeOwned + Sync + Send + 'static> Debug for ReifyError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ReifyError {{ source: {:?} }}", self.source)
    }
}

impl<T: Serialize + DeserializeOwned + Sync + Send + 'static> Resource<T> {
    pub async fn new_shared(item: &T) -> Result<Self, CoreError>
    where
        T: Share,
    {
        let item = item.share();
        Ok(Resource {
            checksum: Checksum::new(&item).await?,
            acquire: Some(Box::new(move || Box::pin(async move { Ok(Serde(item)) }))),
        })
    }
    pub async fn new(item: T) -> Result<Self, CoreError> {
        Ok(Resource {
            checksum: Checksum::new(&item).await?,
            acquire: Some(Box::new(move || Box::pin(async move { Ok(Serde(item)) }))),
        })
    }
    pub async fn new_ref(item: &T) -> Result<Self, CoreError> {
        Ok(Resource {
            checksum: Checksum::new(item).await?,
            acquire: None,
        })
    }
    pub fn reify(self) -> Fallible<T, ReifyError<T>> {
        Box::pin(async move {
            if let Some(acquire) = self.acquire {
                Ok(acquire().await.unwrap().0)
            } else {
                // TODO reify from abstract acquisition methods
                Err(ReifyError {
                    source: anyhow!("no suitable acquisition method"),
                    resource: self,
                })
            }
        })
    }
    pub fn clone_ref(&self) -> Self {
        Resource {
            checksum: self.checksum.clone(),
            acquire: None,
        }
    }
}
