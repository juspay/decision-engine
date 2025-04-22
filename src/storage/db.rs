use super::Storage;
use crate::error::{self, ContainerError};
//https://github.com/juspay/hyperswitch-card-vault/blob/main/src/storage/db.rs

impl super::TestInterface for Storage {
    type Error = error::TestDBError;

    async fn test(&self) -> Result<(), ContainerError<Self::Error>> {
        // let mut conn = self.get_conn().await?;

        // let _data = conn
        //     .test_transaction(|x| {
        //         Box::pin(async {
        //             let query =
        //                 diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>("1 + 1"));
        //             let _x: i32 = query
        //                 .get_result(x)
        //                 .await
        //                 .change_error(error::StorageError::FindError)?;

        //             Ok::<_, ContainerError<Self::Error>>(())
        //         })
        //     })
        //     .await;

        Ok(())
    }
}
