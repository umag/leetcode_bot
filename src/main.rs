use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use teloxide::prelude::*;
use teloxide::types::{ChatId, ParseMode};
use teloxide::Bot;
use tokio::time::{interval, Duration};
use dotenv::dotenv;
use std::env;

// Fetch the daily LeetCode question
// Returns the URL of the daily question if found
// Returns None if no daily question is found
// Returns an error if an HTTP request error occurs
// The function is asynchronous because it makes an HTTP request
async fn fetch_leetcode_daily_question(client: &Client) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let query = r#"
    {
        "query": "query questionOfToday {activeDailyCodingChallengeQuestion {date link question {difficulty}}}",
        "variables": {},
        "operationName": "questionOfToday"
    }
    "#;

    let response = client
        .post("https://leetcode.com/graphql/")
        .header("Content-type", "application/json")
        .header("Origin", "leetcode.com")
        .header("User-agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .body(query)
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

    if let Some(data) = response.get("data") {
        if let Some(active_daily_coding_challenge_question) = data.get("activeDailyCodingChallengeQuestion") {
            if let Some(link) = active_daily_coding_challenge_question.get("link") {
                if let Some(link_str) = link.as_str() {
                    return Ok(Some(format!("https://leetcode.com{}", link_str)));
                }
            }
        }
    }

    Ok(None)
}


// Send the daily LeetCode challenge to the chat
async fn send_daily_challenge(bot: Bot, chat_id: ChatId, client: Client) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(url) = fetch_leetcode_daily_question(&client).await? {
        bot.send_message(chat_id, format!("Today's LeetCode Challenge: {}", url))
            .parse_mode(ParseMode::Html)
            .disable_web_page_preview(true)
            .send()
            .await?;
    } else {
        bot.send_message(chat_id, "No daily question found.")
            .send()
            .await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    // Load the Telegram bot token and chat ID from environment variables
    dotenv().ok();
    let bot_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN not set");
    let chat_id: i64 = env::var("CHAT_ID")
        .expect("CHAT_ID not set")
        .parse()
        .expect("CHAT_ID should be an integer");

    // Initialize the bot and HTTP client
    let bot = Bot::new(bot_token);
    let client = Client::new();
    let chat_id = ChatId(chat_id); // Convert chat_id to ChatId type

    // Set up a daily interval
    let mut interval = interval(Duration::from_secs(60 * 60 * 24));

    loop {
        interval.tick().await;
        if let Err(err) = send_daily_challenge(bot.clone(), chat_id, client.clone()).await {
            eprintln!("Error sending daily challenge: {:?}", err);
        }
    }
}
