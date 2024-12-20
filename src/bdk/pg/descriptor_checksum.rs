use anyhow::anyhow;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use super::convert::BdkKeychainKind;
use crate::primitives::*;

pub struct DescriptorChecksums {
    keychain_id: KeychainId,
    pool: PgPool,
}

impl DescriptorChecksums {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    #[instrument(
        name = "bdk.descriptor_checksums.check_or_persist_descriptor_checksum",
        skip_all
    )]
    pub async fn check_or_persist_descriptor_checksum(
        &self,
        keychain: impl Into<BdkKeychainKind>,
        script_bytes: &[u8],
    ) -> Result<(), anyhow::Error> {
        let kind = keychain.into();
        let record = sqlx::query!(
            r#"SELECT script_bytes
            FROM bdk_descriptor_checksums WHERE keychain_id = $1 AND keychain_kind = $2"#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e|  anyhow!(e.to_string()))?;
        if let Some(record) = record.first() {
            if script_bytes == record.script_bytes {
                Ok(())
            } else {
                Err(anyhow!("Descriptor checksum mismatch"))
            }
        } else {
            sqlx::query!(
                r#"INSERT INTO bdk_descriptor_checksums (script_bytes, keychain_kind, keychain_id)
            VALUES ($1, $2, $3)"#,
                script_bytes,
                kind as BdkKeychainKind,
                Uuid::from(self.keychain_id),
            )
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
            Ok(())
        }
    }
}
