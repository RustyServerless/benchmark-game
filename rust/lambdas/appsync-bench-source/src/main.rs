mod dynamodb_helpers;
mod operations;

pub use dynamodb_utils::appsync_types::*;

// For the types and operations specific to this AppSync project, it uses the GraphQL schema file as a reference
// for the Lambda handler and integration types, it uses generic (and opiniated) event structs
lambda_appsync::appsync_lambda_main! ("graphql/schema.gql", exclude_appsync_types = true, dynamodb() -> aws_sdk_dynamodb::Client);
