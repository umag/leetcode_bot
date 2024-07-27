use chrono::{NaiveTime, Local};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use teloxide::prelude::*;
use teloxide::types::{ChatId, ParseMode};
use teloxide::Bot;
use tokio::time::{interval_at, Duration, Instant};
use dotenv::dotenv;
use std::env;

// Fetch the daily LeetCode question
async fn fetch_leetcode_daily_question(client: &Client) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = r#"
    {
        "query": "query questionOfToday {activeDailyCodingChallengeQuestion {date link question {difficulty}}}",
        "variables": {},
        "operationName": "questionOfToday"
    }
    "#;
    println!("Sending request to leetcode...");
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

    println!("Response from leetcode arrived.");
    if let Some(data) = response.get("data") {
        if let Some(active_daily_coding_challenge_question) = data.get("activeDailyCodingChallengeQuestion") {
            if let Some(link) = active_daily_coding_challenge_question.get("link") {
                if let Some(link_str) = link.as_str() {
                    println!("Daily question found.");
                    return Ok(Some(format!("https://leetcode.com{}", link_str)));
                }
            }
        }
    }

    Ok(None)
}

// Fetch a LeetCode question based on difficulty
async fn fetch_leetcode_question(client: &Client, difficulty: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = format!(r#"
    {{
        "query": "query randomQuestion($categorySlug: String, $filters: QuestionListFilterInput) {{ randomQuestion(categorySlug: $categorySlug, filters: $filters) {{ titleSlug }} }}",
        "variables": {{
            "categorySlug": "",
            "filters": {{
                "difficulty": "{}"
            }}
        }},
        "operationName": "randomQuestion"
    }}
    "#, difficulty);
    println!("Sending request to leetcode for {} question...", difficulty);
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

    println!("Response from leetcode for {} question arrived.", difficulty);
    if let Some(data) = response.get("data") {
        if let Some(random_question) = data.get("randomQuestion") {
            if let Some(title_slug) = random_question.get("titleSlug") {
                if let Some(title_slug_str) = title_slug.as_str() {
                    println!("{} question found.", difficulty);
                    return Ok(Some(format!("https://leetcode.com/problems/{}", title_slug_str)));
                }
            }
        }
    }

    Ok(None)
}

// Send the LeetCode challenge to the chat
async fn send_leetcode_challenge(bot: Bot, chat_id: ChatId, client: Client, difficulty: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = match difficulty {
        "daily" => fetch_leetcode_daily_question(&client).await?,
        _ => fetch_leetcode_question(&client, difficulty).await?,
    };
    println!("Sending message to chat...");
    if let Some(url) = url {
        let message = bot.send_message(chat_id, format!("Today's LeetCode Challenge ({}) : {}", difficulty, url))
            .parse_mode(ParseMode::Html)
            .disable_web_page_preview(true)
            .send()
            .await?;
        bot.pin_chat_message(chat_id, message.id)
        .disable_notification(true)
        .send().await?;
    } else {
        bot.send_message(chat_id, format!("No {} question found.", difficulty))
            .send()
            .await?;
    }
    Ok(())
}

fn duration_until_next_trigger(trigger_time: NaiveTime) -> Duration {
    let now = Local::now().naive_local();
    let target_datetime = now.date().and_time(trigger_time);

    let next_trigger = if now.time() < trigger_time {
        target_datetime
    } else {
        target_datetime + chrono::Duration::days(1)
    };

    let duration = next_trigger - now;
    println!("Duration until next trigger: {}", duration);
    Duration::from_secs(duration.num_seconds() as u64)
}

#[tokio::main]
async fn main() {
    // Load the Telegram bot token and chat ID from environment variables
    dotenv().ok();
    println!("Loading environment variables...");
    let bot_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN not set");
    let chat_id: i64 = env::var("CHAT_ID")
        .expect("CHAT_ID not set")
        .parse()
        .expect("CHAT_ID should be an integer");

    // Load the trigger time from environment variables
    let trigger_time_str = env::var("TRIGGER_TIME").expect("TRIGGER_TIME not set");
    let trigger_time = NaiveTime::parse_from_str(&trigger_time_str, "%H:%M:%S")
        .expect("TRIGGER_TIME should be in the format HH:MM:SS");
    println!("Trigger time: {}", trigger_time);
    println!("Variables loaded.");
    println!("Starting bot...");
    // Initialize the bot and HTTP client
    let bot = Bot::new(bot_token);
    let client = Client::new();
    let chat_id = ChatId(chat_id); // Convert chat_id to ChatId type

    // Calculate the duration until the next trigger time
    let duration = duration_until_next_trigger(trigger_time);
    let start = Instant::now() + duration;
    println!("Sending messages immediately...");
    // Send the messages immediately
    if let Err(err) = send_leetcode_challenge(bot.clone(), chat_id, client.clone(), "daily").await {
        eprintln!("Error sending daily challenge: {:?}", err);
    }
    if let Err(err) = send_leetcode_challenge(bot.clone(), chat_id, client.clone(), "EASY").await {
        eprintln!("Error sending easy challenge: {:?}", err);
    }
    if let Err(err) = send_leetcode_challenge(bot.clone(), chat_id, client.clone(), "HARD").await {
        eprintln!("Error sending hard challenge: {:?}", err);
    }

    // Spawn a task to send the daily challenge at the trigger time
    let bot_clone = bot.clone();
    let client_clone = client.clone();
    println!("Spawning task to send daily challenge...");
    tokio::spawn(async move {
        let mut interval = interval_at(start, Duration::from_secs(60 * 60 * 24));
        loop {
            println!("Waiting for next trigger...");
            interval.tick().await;
            println!("Triggered.");
            if let Err(err) = send_leetcode_challenge(bot_clone.clone(), chat_id, client_clone.clone(), "daily").await {
                eprintln!("Error sending daily challenge: {:?}", err);
            }
        }
    });

    // Spawn a task to send an easy problem at the trigger time
    let bot_clone_easy = bot.clone();
    let client_clone_easy = client.clone();
    println!("Spawning task to send easy challenge...");
    tokio::spawn(async move {
        let mut interval = interval_at(start, Duration::from_secs(60 * 60 * 24));
        loop {
            println!("Waiting for next trigger...");
            interval.tick().await;
            println!("Triggered.");
            if let Err(err) = send_leetcode_challenge(bot_clone_easy.clone(), chat_id, client_clone_easy.clone(), "Easy").await {
                eprintln!("Error sending easy challenge: {:?}", err);
            }
        }
    });

    // Spawn a task to send a hard problem at the trigger time
    let bot_clone_hard = bot.clone();
    let client_clone_hard = client.clone();
    println!("Spawning task to send hard challenge...");
    tokio::spawn(async move {
        let mut interval = interval_at(start, Duration::from_secs(60 * 60 * 24));
        loop {
            println!("Waiting for next trigger...");
            interval.tick().await;
            println!("Triggered.");
            if let Err(err) = send_leetcode_challenge(bot_clone_hard.clone(), chat_id, client_clone_hard.clone(), "Hard").await {
                eprintln!("Error sending hard challenge: {:?}", err);
            }
        }
    });

    // Keep the bot running
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}
