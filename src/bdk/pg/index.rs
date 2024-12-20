use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use super::convert::BdkKeychainKind;
use crate::primitives::*;

pub struct Indexes {
    pool: PgPool,
    keychain_id: KeychainId,
}

impl Indexes {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    #[instrument(name = "bdk.indexes.increment", skip_all)]
    pub async fn increment(&self, keychain: impl Into<BdkKeychainKind>) -> Result<u32, anyhow::Error> {
        let kind = keychain.into();
        let result = sqlx::query!(
            r#"
              INSERT INTO bdk_indexes (keychain_id, keychain_kind)
              VALUES ($1, $2)
              ON CONFLICT (keychain_id, keychain_kind)
              DO UPDATE SET index = bdk_indexes.index + 1, modified_at = NOW()
              WHERE bdk_indexes.keychain_id = $1 AND bdk_indexes.keychain_kind = $2
              RETURNING index;
              "#,
            self.keychain_id as KeychainId,
            kind as BdkKeychainKind
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::Generic(e.to_string()))?;

        let new_idx = result.index;
        Ok(new_idx as u32)
    }

    #[instrument(name = "bdk.indexes.persist_last_index", skip_all)]
    pub async fn persist_last_index(
        &self,
        keychain: impl Into<BdkKeychainKind>,
        idx: u32,
    ) -> Result<(), anyhow::Error> {
        let kind = keychain.into();
        sqlx::query!(
            r#"INSERT INTO bdk_indexes (keychain_id, keychain_kind, index)
               VALUES ($1, $2, $3)
               ON CONFLICT (keychain_id, keychain_kind)
               DO UPDATE SET index = $3, modified_at = NOW()
               WHERE bdk_indexes.index < $3 AND bdk_indexes.keychain_id = $1 AND bdk_indexes.keychain_kind = $2"#,
            self.keychain_id as KeychainId,
            kind as BdkKeychainKind,
            idx as i32
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
        Ok(())
    }

    #[instrument(name = "bdk.indexes.get_latest", skip_all)]
    pub async fn get_latest(
        &self,
        keychain: impl Into<BdkKeychainKind>,
    ) -> Result<Option<u32>, anyhow::Error> {
        let kind = keychain.into();
        let rows = sqlx::query!(
            r#"SELECT index FROM bdk_indexes WHERE keychain_id = $1 AND keychain_kind = $2"#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
        Ok(rows.first().map(|row| row.index as u32))
    }
}
