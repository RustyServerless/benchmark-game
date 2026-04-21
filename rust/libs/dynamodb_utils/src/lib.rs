//! Utility module for working with DynamoDB tables that use a simple partition key ("PK") schema.

pub mod appsync_types;

use dynamodb_facade::{StringAttribute, attribute_definitions, table_definitions};
use lambda_appsync::AppsyncError;

attribute_definitions! {
    PK {
        "PK": StringAttribute
    }
    ItemType {
        "type": StringAttribute
    }
}

table_definitions! {
    SimpleTable {
        type PartitionKey = PK;
        fn table_name() -> String {
            let table_name = std::env::var("BACKEND_TABLE_NAME")
                .expect("Mandatory environment variable `BACKEND_TABLE_NAME` is not set");
            log::debug!("BACKEND_TABLE_NAME={table_name}");
            table_name
        }
    }
}

pub fn facade2appsync(error: dynamodb_facade::Error) -> AppsyncError {
    use dynamodb_facade::Error;
    match error {
        Error::DynamoDB(error) => (*error).into(),
        Error::Serde(error) => AppsyncError::new("SerdeError", error.to_string()),
        Error::Custom(s) => AppsyncError::new("CustomDynDBFacade", s),
        Error::Other(error) => AppsyncError::new("OtherDynDBFacade", error.to_string()),
        Error::FailedBatchWrite(write_requests) => AppsyncError::new(
            "FailedDynamoDBBatchWrite",
            format!(
                "Failed to process {} batch write items",
                write_requests.len()
            ),
        ),
    }
}
