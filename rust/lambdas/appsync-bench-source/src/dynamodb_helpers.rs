use dynamodb_facade::{Condition, DynamoDBItemOp, KeyId, Update};

use lambda_appsync::{ID, log};

use crate::{Player, dynamodb};

/// Increments a player's click counter atomically, after verifying their secret
///
/// If the clicks attribute doesn't exist yet, it will be initialized to 1
pub async fn dynamodb_update_player_click(
    player_id: ID,
    secret: String,
) -> Result<Player, dynamodb_facade::Error> {
    log::debug!("ENTER dynamodb_player_click - player_id={player_id}");
    Player::update_by_id(
        dynamodb(),
        KeyId::pk(player_id),
        Update::init_increment("clicks", 0, 1),
    )
    .condition(Player::exists() & Condition::eq("secret", secret))
    .await
}

/// Updates a player's latency statistics, using optimistic locking to prevent concurrent updates
///
/// The old values are used as update conditions to ensure no concurrent update happened. They should either both be None
/// (for first update) or both be Some.
/// Note that concurrent updates should never happen because the frontend is set to send a report per second,
/// which is plently enough to finish an update before the following one.
pub async fn dynamodb_update_player_latency_stats(
    player_id: ID,
    secret: String,
    old_avg_latency: Option<f64>,
    old_avg_latency_clicks: Option<i32>,
    new_avg_latency: f64,
    new_avg_latency_clicks: i32,
) -> Result<Player, dynamodb_facade::Error> {
    log::debug!(
        "ENTER dynamodb_update_player_latency_stats - \
        player_id={player_id} \
        old_avg_latency={old_avg_latency:?} old_avg_latency_clicks={old_avg_latency_clicks:?} \
        new_avg_latency={new_avg_latency} new_avg_latency_clicks={new_avg_latency_clicks}"
    );

    // Start building the update operation with the new values
    let update = Update::set("avg_latency", new_avg_latency)
        .and(Update::set("avg_latency_clicks", new_avg_latency_clicks));
    let base_condition = Player::exists() & Condition::eq("secret", secret);

    // Add optimistic locking condition based on old values
    let latency_condition = match (old_avg_latency, old_avg_latency_clicks) {
        // If we had previous values, ensure they haven't changed
        (Some(old_avg_latency), Some(old_avg_latency_clicks)) => {
            Condition::eq("avg_latency", old_avg_latency)
                & Condition::eq("avg_latency_clicks", old_avg_latency_clicks)
        }

        // For first update, ensure attributes don't exist yet
        (None, None) => {
            Condition::not_exists("avg_latency") & Condition::not_exists("avg_latency_clicks")
        }
        _ => unreachable!(
            "Functionnal error, old_avg_latency and old_avg_latency_clicks \
            can only be both None or both Some"
        ),
    };

    log::debug!("update={update}");
    log::debug!("latency_condition={latency_condition}");

    Player::update_by_id(dynamodb(), KeyId::pk(player_id), update)
        .condition(base_condition & latency_condition)
        .await
}
