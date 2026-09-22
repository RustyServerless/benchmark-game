use crate::{GameStatus, LatencyReport, Player};

use data_model::facade2appsync;
use dynamodb_facade::{Condition, DynamoDBItemOp, KeyId, Update};
use lambda_appsync::{AppsyncError, ID, appsync_operation, log};

fn player_not_found() -> AppsyncError {
    AppsyncError::new("PlayerNotFound", "Player does not exist")
}
fn invalid_game_status() -> AppsyncError {
    AppsyncError::new("InvalidGameStatus", "Game is not started")
}

async fn assert_game_started() -> Result<(), AppsyncError> {
    // Retrieve the current game status
    let game_status = GameStatus::get(KeyId::NONE)
        .await
        .map_err(facade2appsync)?
        .ok_or_else(invalid_game_status)?;

    // If the game is not "Started", then we return an error
    if game_status != GameStatus::Started {
        return Err(invalid_game_status());
    }
    Ok(())
}

#[appsync_operation(mutation(clickRust))]
pub async fn click(player_id: ID, secret: String) -> Result<Player, AppsyncError> {
    assert_game_started().await?;
    // increment the click_counter of the player

    Player::update_by_id(KeyId::pk(player_id), Update::init_increment("clicks", 0, 1))
        .condition(Player::exists() & Condition::eq("secret", secret))
        .await
        .map_err(facade2appsync)
}

#[appsync_operation(mutation(reportLatencyRust))]
pub async fn report_latency(
    player_id: ID,
    report: LatencyReport,
    secret: String,
) -> Result<Player, AppsyncError> {
    // Kick off an async request to get the player data first, so it can run in parallel
    // with the game status check that follows
    let player_req = lambda_appsync::tokio::spawn(Player::get(KeyId::pk(player_id)).execute());

    // Verify the game is currently in progress
    assert_game_started().await?;

    // Wait for and retrieve the player data we requested earlier
    let player = player_req
        .await
        .unwrap()
        .map_err(facade2appsync)?
        .ok_or_else(player_not_found)?;

    // Extract the values from the latency report:
    // - clicks: how many clicks were made during this reporting period
    // - avg_latency: the average latency (in ms) measured for those clicks
    let LatencyReport {
        clicks,
        avg_latency,
    } = report;

    // Get the player's current statistics:
    // - old_avg_latency: their current average latency across all clicks
    // - old_avg_latency_clicks: how many clicks that average is based on
    let old_avg_latency = player.avg_latency;
    let old_avg_latency_clicks = player.avg_latency_clicks;

    // Calculate the total cumulative latency from all previous clicks.
    // If this is the player's first report (both values None), start at 0.
    // Otherwise multiply their current average by number of clicks to get total.
    let old_total_latency = match (old_avg_latency, old_avg_latency_clicks) {
        (Some(old_avg_latency), Some(old_avg_latency_clicks)) => {
            old_avg_latency * (old_avg_latency_clicks as f64)
        }
        (None, None) => 0f64,
        _ => unreachable!(
            "Functionnal error, old_avg_latency and old_avg_latency_clicks \
        can only be both None or both Some"
        ),
    };

    // Add the new latency total to the cumulative total:
    // new latency total = old latency total + this report's average * number of clicks in this report
    let new_total_latency = old_total_latency + avg_latency * (clicks as f64);

    // Update the total click count by adding new clicks to the old total (or to 0 if first report)
    let new_avg_latency_clicks = old_avg_latency_clicks.unwrap_or_default() + clicks;

    // Calculate the new overall average:
    // total latency across all clicks / total number of clicks
    let new_avg_latency = new_total_latency / (new_avg_latency_clicks as f64);

    // Only update the stats in the database if we got a valid new average latency
    // (protects against division by zero or other invalid calculations)
    if new_avg_latency.is_finite() {
        log::debug!(
            "report_latency - player_id={player_id} \
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

        Ok(Player::update_by_id(KeyId::pk(player_id), update)
            .condition(base_condition & latency_condition)
            .await
            .map_err(facade2appsync)?)
    } else {
        // If the calculation gave invalid results, return the player unchanged
        Ok(player)
    }
}
