use std::collections::HashSet;

use crate::{
    GameStatus, Player, Team, dynamodb,
    dynamodb_helpers::{
        dynamodb_delete_player, dynamodb_list_players, dynamodb_put_new_player,
        dynamodb_query_teams_player_count, dynamodb_reset_game, dynamodb_set_game_status,
        dynamodb_update_player_name,
    },
};
use dynamodb_facade::{DynamoDBItemOp, KeyId};
use dynamodb_utils::facade2appsync;
use lambda_appsync::{AppsyncError, ID, appsync_operation};

fn player_not_found() -> AppsyncError {
    AppsyncError::new("PlayerNotFound", "Player does not exist")
}

#[appsync_operation(query(players))]
pub async fn players() -> Result<Vec<Player>, AppsyncError> {
    dynamodb_list_players().await.map_err(facade2appsync)
}

#[appsync_operation(query(gameStatus))]
pub async fn game_status() -> Result<GameStatus, AppsyncError> {
    Ok(GameStatus::get(dynamodb(), KeyId::NONE)
        .await
        .map_err(facade2appsync)?
        .unwrap_or(GameStatus::Reset))
}

// This is a declarative macro that helps reduce boilerplate code for game status mutation operations.
// It generates a function for each game status mutation (like startGame and stopGame) that follows
// the same pattern but with different GameStatus values.
macro_rules! game_status_mut {
    // The macro takes two parameters:
    // $mut_name: The identifier for the mutation name (like startGame)
    // $status: The path to the GameStatus variant to set (like GameStatus::Started)
    ($mut_name:ident, $status:path ) => {
        // The macro generates an async function annotated with appsync_operation
        // indicating this is a GraphQL mutation handler
        #[appsync_operation(mutation($mut_name))]
        pub async fn _discarded() -> Result<GameStatus, AppsyncError> {
            // Update the game status in DynamoDB to the new status
            dynamodb_set_game_status($status)
                .await
                .map_err(facade2appsync)?;
            // Return the new status on success
            Ok($status)
        }
    };
}

// Generate two mutation handlers:
// - startGame: Sets game status to Started
// - stopGame: Sets game status to Stopped
game_status_mut!(startGame, GameStatus::Started);
game_status_mut!(stopGame, GameStatus::Stopped);

#[appsync_operation(mutation(resetGame))]
pub async fn reset_game() -> Result<GameStatus, AppsyncError> {
    dynamodb_reset_game().await.map_err(facade2appsync)?;
    Ok(GameStatus::Reset)
}

#[appsync_operation(mutation(registerNewPlayer))]
pub async fn register_new_player(name: String, secret: String) -> Result<Player, AppsyncError> {
    // Query DynamoDB to get the current count of players in each team
    let mut teams_player_count = dynamodb_query_teams_player_count()
        .await
        .map_err(facade2appsync)?;

    // Choose which team to assign this player to
    let team = if teams_player_count.len() < Team::COUNT {
        // If all teams are not yet used, choose one of the unused teams
        let mut all_teams = HashSet::from(Team::all());
        while let Some((team, _)) = teams_player_count.pop() {
            all_teams.remove(&team);
        }
        // Get the first unused team
        all_teams
            .into_iter()
            .next()
            .expect("we ensured teams_player_count had less element than all_teams")
    } else {
        // If all teams are used, choose the one with the fewest players
        teams_player_count.sort_by_key(|o| o.1);
        teams_player_count[0].0
    };

    // Generate a new unique ID for this player
    let id = ID::new();

    // Create the new player record
    let new_player = Player {
        id,
        name,
        team,
        clicks: None,
        avg_latency: None,
        avg_latency_clicks: None,
    };

    // Save the new player to DynamoDB
    dynamodb_put_new_player(&new_player, secret)
        .await
        .map_err(facade2appsync)?;

    // Return the newly created player
    Ok(new_player)
}

#[appsync_operation(mutation(updatePlayerName))]
pub async fn update_player_name(
    player_id: ID,
    new_name: String,
    secret: String,
) -> Result<Player, AppsyncError> {
    dynamodb_update_player_name(player_id, new_name, secret)
        .await
        .map_err(facade2appsync)
}

#[appsync_operation(mutation(removePlayer))]
pub async fn remove_player(player_id: ID) -> Result<Player, AppsyncError> {
    dynamodb_delete_player(player_id)
        .await
        .map_err(facade2appsync)?
        .ok_or_else(player_not_found)
}
