use std::collections::HashMap;

use dynamodb_facade::{
    Condition, DynamoDBItem, DynamoDBItemBatchOp, DynamoDBItemOp, Item, KeyId, Update,
    dynamodb_batch_write,
};
use dynamodb_utils::{SimpleTable, appsync_types::PlayerWithSecret};
use lambda_appsync::{ID, log};

use crate::{GameStatus, Player, Team, dynamodb};

/// Retrieves all [Player] items from DynamoDB as raw [DynamoItem]
/// Used internally by query functions that need access to the full item data
async fn dynamodb_list_player_items() -> Result<Vec<Item<SimpleTable>>, dynamodb_facade::Error> {
    Player::scan(dynamodb())
        .filter(Condition::eq("type", Player::PK_TYPE))
        .raw()
        .all()
        .await
}
pub async fn dynamodb_list_players() -> Result<Vec<Player>, dynamodb_facade::Error> {
    Player::scan(dynamodb())
        .filter(Condition::eq("type", Player::PK_TYPE))
        .all()
        .await
}

/// Resets the game state and clears all player scores
///
/// First sets game status to [GameStatus::Reset], then removes all score-related attributes
/// from player records while preserving other player data
pub async fn dynamodb_reset_game() -> Result<(), dynamodb_facade::Error> {
    log::debug!("ENTER dynamodb_reset_game");
    // Start by changing the state to Reset
    // It serves to verify we are actualy in the correct state pour doing that
    // It also prevents any further usage of the "click" button
    dynamodb_set_game_status(GameStatus::Reset).await?;

    // Note that from this point and until we finish cleaning the players, the game is
    // in a somewhat incorrect state: the status is technically `Reset` but players still have scores.
    // This is just a demo, so we will accept that fact.

    // List players as DynamoItem
    // Because we want to retrieve the `secret` field and put it back with the PutItem
    let players = dynamodb_list_player_items().await?;

    // Create the iterator of BatchWriteRequest that will PUT every players without clicks/latency
    let batch_write_requests = players
        .into_iter()
        .map(|player_item| {
            let (key, mut rest) = player_item.extract_key();
            let dynamodb_facade::AttributeValue::S(secret) =
                rest.remove("secret").expect("valid schema")
            else {
                unreachable!("valid schema")
            };
            let mut player = Player::from_item(Item::from_key_and_attributes(key, rest));
            player.clicks = None;
            player.avg_latency = None;
            player.avg_latency_clicks = None;
            // Create the BatchWriteRequest
            PlayerWithSecret::new(&player, secret).batch_put()
        })
        .collect::<Vec<_>>();
    dynamodb_batch_write::<SimpleTable>(dynamodb(), batch_write_requests).await
}

/// Updates the game status in DynamoDB
///
/// Enforces valid state transitions by checking the current status matches
/// what is expected for the requested new status
pub async fn dynamodb_set_game_status(status: GameStatus) -> Result<(), dynamodb_facade::Error> {
    log::debug!("ENTER dynamodb_set_game_status - status={status:?}");
    // Can only set GameStatus in some order
    let current_status = status.valid_from_status();

    status
        .put(dynamodb())
        .condition(
            GameStatus::not_exists()
                | Condition::eq(
                    GameStatus::PROPERTY_NAME,
                    current_status.to_attribute_value(),
                ),
        )
        .await
}

/// Creates a new player record in DynamoDB
///
/// Adds the provided secret along with the player data for future authentication of the player
pub async fn dynamodb_put_new_player(
    new_player: &Player,
    secret: String,
) -> Result<(), dynamodb_facade::Error> {
    log::debug!("ENTER dynamodb_put_new_player - new_player={new_player:?}");
    PlayerWithSecret::new(new_player, secret)
        .put(dynamodb())
        .not_exists()
        .await
}

/// Updates a player's name after verifying their secret
///
/// Returns the updated [Player] record
pub async fn dynamodb_update_player_name(
    player_id: ID,
    new_name: String,
    secret: String,
) -> Result<Player, dynamodb_facade::Error> {
    log::debug!("ENTER dynamodb_update_player_name - player_id={player_id} new_name={new_name}");
    Player::update_by_id(
        dynamodb(),
        KeyId::pk(player_id),
        Update::set("name", new_name),
    )
    .condition(Player::exists() & Condition::eq("secret", secret))
    .await
}

/// Deletes a player record from DynamoDB
///
/// Returns the deleted [Player] if it existed
pub async fn dynamodb_delete_player(
    player_id: ID,
) -> Result<Option<Player>, dynamodb_facade::Error> {
    log::debug!("ENTER dynamodb_delete_player - player_id={player_id}");
    Player::delete_by_id(dynamodb(), KeyId::pk(player_id)).await
}

/// Queries DynamoDB to get a count of players per team
///
/// Returns a vector of ([Team], count) tuples
pub async fn dynamodb_query_teams_player_count()
-> Result<Vec<(Team, usize)>, dynamodb_facade::Error> {
    log::debug!("ENTER dynamodb_query_teams_player_count");

    let players = dynamodb_list_players().await?;

    let counts =
        players
            .into_iter()
            .map(|player| player.team)
            .fold(HashMap::new(), |mut counts, team| {
                *counts.entry(team).or_insert(0usize) += 1;
                counts
            });

    Ok(counts.into_iter().collect())
}
