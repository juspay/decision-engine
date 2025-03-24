use crate::{
    ephemeral_store::{success_rate::SuccessRateEphemeralStoreInterface, InMemoryEphemeralStore},
    success_rate::{
        block::{Block, CurrentBlock},
        error::SuccessRateError,
        types::{BlockFields, KeyDeletionStatus},
    },
};

#[async_trait::async_trait]
impl SuccessRateEphemeralStoreInterface for InMemoryEphemeralStore {
    async fn set_aggregates(
        &self,
        aggregates_key: &str,
        aggregates: Vec<Block>,
    ) -> error_stack::Result<(), SuccessRateError> {
        let mut storage = self.success_rate.write().await;
        storage
            .aggregates
            .insert(aggregates_key.to_string(), aggregates);
        Ok(())
    }

    async fn fetch_aggregates(
        &self,
        aggregates_key: &str,
    ) -> error_stack::Result<Vec<Block>, SuccessRateError> {
        let storage = self.success_rate.read().await;
        let aggregates = storage
            .aggregates
            .get(aggregates_key)
            .cloned()
            .unwrap_or_default();
        Ok(aggregates)
    }

    async fn initialize_current_block(
        &self,
        current_block_key: &str,
        success_count: u64,
        total_count: u64,
    ) -> error_stack::Result<(), SuccessRateError> {
        let block = Block::new(success_count, total_count)?;

        let current_block = CurrentBlock::from_block(block);

        let mut storage = self.success_rate.write().await;
        storage
            .current_blocks
            .insert(current_block_key.to_string(), current_block);
        Ok(())
    }

    async fn fetch_current_block(
        &self,
        current_block_key: &str,
    ) -> error_stack::Result<CurrentBlock, SuccessRateError> {
        let storage = self.success_rate.read().await;
        let current_block = storage
            .current_blocks
            .get(current_block_key)
            .cloned()
            .unwrap_or_default();
        Ok(current_block)
    }

    async fn incr_current_block_fields(
        &self,
        current_block_key: &str,
        fields_to_increment: &[(BlockFields, i64)],
    ) -> error_stack::Result<Vec<usize>, SuccessRateError> {
        let mut storage = self.success_rate.write().await;

        let current_block = storage
            .current_blocks
            .entry(current_block_key.to_string())
            .or_insert_with(CurrentBlock::new)
            .inner_mut();

        let mut results = Vec::with_capacity(fields_to_increment.len());

        #[allow(clippy::as_conversions)]
        if let Some(current_block) = current_block {
            for (field, increment) in fields_to_increment {
                let new_value = match field {
                    BlockFields::SuccessCount => {
                        if *increment >= 0 {
                            current_block.success_count = current_block
                                .success_count
                                .saturating_add(*increment as u64);
                        } else {
                            current_block.success_count = current_block
                                .success_count
                                .saturating_sub((-*increment) as u64);
                        }
                        current_block.success_count as usize
                    }
                    BlockFields::TotalCount => {
                        if *increment >= 0 {
                            current_block.total_count =
                                current_block.total_count.saturating_add(*increment as u64);
                        } else {
                            current_block.total_count = current_block
                                .total_count
                                .saturating_sub((-*increment) as u64);
                        }
                        current_block.total_count as usize
                    }
                    BlockFields::CreatedAt => current_block.created_at as usize,
                };

                results.push(new_value);
            }
        }

        Ok(results)
    }

    async fn delete_key(
        &self,
        key: &str,
    ) -> error_stack::Result<KeyDeletionStatus, SuccessRateError> {
        let mut storage = self.success_rate.write().await;
        let mut is_key_deleted = KeyDeletionStatus::NotDeleted;

        if storage.aggregates.remove(key).is_some() {
            is_key_deleted = KeyDeletionStatus::Deleted;
        }

        if storage.current_blocks.remove(key).is_some() {
            is_key_deleted = KeyDeletionStatus::Deleted;
        }

        Ok(is_key_deleted)
    }

    async fn delete_keys_matching_prefix(
        &self,
        prefix: &str,
    ) -> error_stack::Result<Vec<(String, KeyDeletionStatus)>, SuccessRateError> {
        let mut storage = self.success_rate.write().await;
        let mut result = Vec::new();

        let block_keys: Vec<String> = storage
            .aggregates
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();

        for key in block_keys {
            storage.aggregates.remove(&key);
            result.push((key, KeyDeletionStatus::Deleted));
        }

        let current_block_keys: Vec<String> = storage
            .current_blocks
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();

        for key in current_block_keys {
            storage.current_blocks.remove(&key);
            result.push((key, KeyDeletionStatus::Deleted));
        }

        Ok(result)
    }
}
