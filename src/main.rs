use chrono::{NaiveTime, Local};
use rand::Rng;
use reqwest::Client;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, ParseMode};
use teloxide::Bot;
use tokio::sync::Mutex;
use tokio::time::{interval_at, sleep, Duration, Instant};
use dotenv::dotenv;
use std::env;
use tokio::fs as async_fs;
use tokio::io::AsyncWriteExt;

// Fetch the daily LeetCode question
async fn fetch_leetcode_daily_question(client: &Client) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = r#"
    {
        "query": "query questionOfToday {activeDailyCodingChallengeQuestion {date link question {difficulty}}}",
        "variables": {},
        "operationName": "questionOfToday"
    }
    "#;
    println!("Sending request to LeetCode for daily question...");
    let response = client
        .post("https://leetcode.com/graphql/")
        .header("Content-type", "application/json")
        .header("Origin", "leetcode.com")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3")
        .body(query)
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

    println!("Response from LeetCode arrived for daily question.");
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


// Send the LeetCode challenges to all subscribed chats
async fn send_daily_challenge(bot: Bot, chat_ids: Arc<Mutex<HashSet<ChatId>>>, client: Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let daily_question = fetch_leetcode_daily_question(&client).await?;

    let message_text = format!(
        "Today's LeetCode Challenge:\n\nDaily: {}",
        daily_question.unwrap_or_else(|| "Not available".to_string()),

    );
    println!("Sending message to all chats...");
    let chat_ids_guard = chat_ids.lock().await;
    for &chat_id in chat_ids_guard.iter() {
        let delay = rand::thread_rng().gen_range(0..600); // Random delay up to 10 minutes
        println!("Sending message to chat {} with a delay of {} seconds...", chat_id, delay);
        sleep(Duration::from_secs(delay)).await;
        let message = bot.send_message(chat_id, message_text.clone())
            .parse_mode(ParseMode::Html)
            .disable_web_page_preview(true)
            .send()
            .await?;
        bot.pin_chat_message(chat_id, message.id)
            .disable_notification(true)
            .send()
            .await?;
        println!("Message sent to chat {}.", chat_id);
    }

    Ok(())
}

// Calculate the duration until the next trigger time
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

// Load chat IDs from the file
async fn load_chat_ids(file_path: &str) -> HashSet<ChatId> {
    println!("Loading chat IDs from file...");
    if let Ok(data) = fs::read_to_string(file_path) {
        println!("Chat IDs file found.");
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        println!("Chat IDs file not found, creating a new one.");
        HashSet::new()
    }

}

// Save chat IDs to the file
async fn save_chat_ids(file_path: &str, chat_ids: &HashSet<ChatId>) {
    println!("Saving chat IDs to file...");
    if let Ok(data) = serde_json::to_string(chat_ids) {
        // Use tokio::fs::File for async file handling
        if let Ok(mut file) = async_fs::File::create(file_path).await {
            if file.write_all(data.as_bytes()).await.is_ok() {
                if file.sync_all().await.is_ok() {
                    println!("Chat IDs saved.");
                } else {
                    println!("Failed to sync data to disk.");
                }
            } else {
                println!("Failed to write data to file.");
            }
        } else {
            println!("Failed to create file.");
        }
    } else {
        println!("Failed to serialize chat IDs.");
    }
}

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv().ok();
    println!("Loading environment variables...");
    let bot_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN not set");
    let trigger_time_str = env::var("TRIGGER_TIME").expect("TRIGGER_TIME not set");
    let trigger_time = NaiveTime::parse_from_str(&trigger_time_str, "%H:%M:%S")
        .expect("TRIGGER_TIME should be in the format HH:MM:SS");
    let chat_ids_file_path = env::var("CHAT_IDS_FILE_PATH").expect("CHAT_IDS_FILE_PATH not set");

    // Initialize the bot and HTTP client
    println!("Initializing bot and client...");
    let bot = Bot::new(bot_token);
    let client = Client::new();

    // Load chat IDs from the file
    println!("Loading chat IDs from file...");
    let chat_ids = Arc::new(Mutex::new(load_chat_ids(&chat_ids_file_path).await));
    println!("Chat IDs loaded.");
    // Calculate the duration until the next trigger time
    let duration = duration_until_next_trigger(trigger_time);
    let start = Instant::now() + duration;

    // Clone necessary references for the spawned task
    let bot_clone = bot.clone();
    let client_clone = client.clone();
    let chat_ids_clone = Arc::clone(&chat_ids);

    // Spawn a task to send the daily challenges at the trigger time
    println!("Spawning task to send daily challenges...");
    tokio::spawn(async move {
        let mut interval = interval_at(start, Duration::from_secs(60 * 60 * 24));
        loop {
            println!("Waiting for next trigger...");
            interval.tick().await;
            println!("Triggered.");
            if let Err(err) = send_daily_challenge(bot_clone.clone(), Arc::clone(&chat_ids_clone), client_clone.clone()).await {
                eprintln!("Error sending daily challenge: {:?}", err);
            }
        }
    });

    // Handle incoming messages
    println!("Starting message handler...");
    let handler = Update::filter_message().branch(dptree::entry().endpoint({
        let chat_ids = Arc::clone(&chat_ids);
        let client_clone = client.clone();
        let bot_clone = bot.clone();
        let chat_ids_file_path = chat_ids_file_path.clone();
        move |message: Message, bot: Bot| {
            let chat_id = message.chat.id;
            let text = message.text().unwrap_or_default().to_string();
            let chat_ids = Arc::clone(&chat_ids);
            let client_clone = client_clone.clone();
            let bot_clone = bot_clone.clone();
            let chat_ids_file_path = chat_ids_file_path.clone();
            async move {
                match text.as_str() {
                    "/start" => {
                        println!("Chat {} started receiving challenges.", chat_id);
                        {
                            let mut chat_ids_guard = chat_ids.lock().await;
                            chat_ids_guard.insert(chat_id);
                            save_chat_ids(&chat_ids_file_path, &chat_ids_guard).await;
                        }
                        bot.send_message(chat_id, "You will start receiving daily challenges.")
                            .send()
                            .await?;

                        // Send the first set of challenges immediately
                        if let Err(err) = send_daily_challenge(bot_clone, Arc::clone(&chat_ids), client_clone).await {
                            eprintln!("Error sending initial challenges: {:?}", err);
                        }
                    }
                    "/stop" => {
                        println!("Chat {} stopped receiving challenges.", chat_id);
                        {
                            let mut chat_ids_guard = chat_ids.lock().await;
                            chat_ids_guard.remove(&chat_id);
                            save_chat_ids(&chat_ids_file_path, &chat_ids_guard).await;
                        }
                        bot.send_message(chat_id, "You have stopped receiving daily challenges.")
                            .send()
                            .await?;
                    }
                    _ => {
                        // do nothing
                    }
                }
                respond(())
            }
        }
    }));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
