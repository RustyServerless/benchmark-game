mod operations;

pub use data_model::appsync_types::*;
use lambda_appsync::env_logger;

// For the types and operations specific to this AppSync project, it uses the GraphQL schema file as a reference
// for the Lambda handler and integration types, it uses generic (and opiniated) event structs
lambda_appsync::make_operation!("graphql/schema.gql");

lambda_appsync::make_handlers!();

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    // log_init → call directly in main
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("info")
            .default_write_style_or("never"),
    )
    .format_timestamp_micros()
    .init();

    dynamodb_facade::init_global_client(dynamodb_facade::Client::new(
        &aws_config::load_from_env().await,
    ));

    lambda_runtime::run(lambda_runtime::service_fn(
        lambda_appsync::default_service_fn!(),
    ))
    .await
}
