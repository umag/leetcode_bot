use chrono::{NaiveTime, Local};
use rand::Rng;
use reqwest::Client;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, ParseMode};
use teloxide::Bot;
use tokio::sync::Mutex;
use tokio::time::{interval_at, sleep, Duration, Instant};
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
    println!("Sending request to LeetCode for daily question...");
    let response = client
        .post("https://leetcode.com/graphql/")
        .header("Content-type", "application/json")
        .header("Origin", "leetcode.com")
        .header("User-agent", "Mozilla/5.0")
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

// Fetch a LeetCode question based on difficulty
async fn fetch_leetcode_question(client: &Client, difficulty: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = format!(r#"
    {{
        "query": "query problemsetQuestionList($categorySlug: String, $filters: QuestionListFilterInput) {{ 
            problemsetQuestionList(categorySlug: $categorySlug, filters: $filters) {{ questions {{ titleSlug }} }} 
        }}",
        "variables": {{
            "categorySlug": "",
            "filters": {{
                "difficulty": "{}"
            }}
        }},
        "operationName": "problemsetQuestionList"
    }}
    "#, difficulty.to_uppercase());
    println!("Sending request to LeetCode for {} question...", difficulty);
    let response = client
        .post("https://leetcode.com/graphql/")
        .header("Content-type", "application/json")
        .header("Origin", "leetcode.com")
        .header("User-agent", "Mozilla/5.0")
        .body(query)
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

    println!("Response from LeetCode for {} question arrived.", difficulty);
    if let Some(data) = response.get("data") {
        if let Some(problemsetQuestionList) = data.get("problemsetQuestionList") {
            if let Some(questions) = problemsetQuestionList.get("questions") {
                if let Some(first_question) = questions.as_array().and_then(|arr| arr.get(0)) {
                    if let Some(title_slug) = first_question.get("titleSlug") {
                        if let Some(title_slug_str) = title_slug.as_str() {
                            println!("{} question found.", difficulty);
                            return Ok(Some(format!("https://leetcode.com/problems/{}", title_slug_str)));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

// Send the LeetCode challenges to all subscribed chats
async fn send_daily_challenge(bot: Bot, chat_ids: Arc<Mutex<HashSet<ChatId>>>, client: Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let daily_question = fetch_leetcode_daily_question(&client).await?;
    let easy_question = fetch_leetcode_question(&client, "EASY").await?;
    let hard_question = fetch_leetcode_question(&client, "HARD").await?;

    let message_text = format!(
        "Today's LeetCode Challenges:\n\nDaily: {}\n\nEasy: {}\n\nHard: {}",
        daily_question.unwrap_or_else(|| "Not available".to_string()),
        easy_question.unwrap_or_else(|| "Not available".to_string()),
        hard_question.unwrap_or_else(|| "Not available".to_string())
    );

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

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv().ok();
    println!("Loading environment variables...");
    let bot_token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN not set");
    let trigger_time_str = env::var("TRIGGER_TIME").expect("TRIGGER_TIME not set");
    let trigger_time = NaiveTime::parse_from_str(&trigger_time_str, "%H:%M:%S")
        .expect("TRIGGER_TIME should be in the format HH:MM:SS");

    // Initialize the bot and HTTP client
    println!("Initializing bot and client...");
    let bot = Bot::new(bot_token);
    let client = Client::new();
    let chat_ids = Arc::new(Mutex::new(HashSet::new()));

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
    teloxide::repl(bot.clone(), move |message: Message, bot: Bot| {
        let chat_id = message.chat.id;
        let text = message.text().unwrap_or_default().to_string();
        let chat_ids = Arc::clone(&chat_ids);

        async move {
            match text.as_str() {
                "/start" => {
                    println!("Chat {} started receiving challenges.", chat_id);
                    {
                        let mut chat_ids_guard = chat_ids.lock().await;
                        chat_ids_guard.insert(chat_id);
                    }
                    bot.send_message(chat_id, "Welcome to the LeetCode Challenge Bot! You will receive daily challenges.")
                        .parse_mode(ParseMode::Html)
                        .send()
                        .await?;
                },
                "/stop" => {
                    println!("Chat {} stopped receiving challenges.", chat_id);
                    {
                        let mut chat_ids_guard = chat_ids.lock().await;
                        chat_ids_guard.remove(&chat_id);
                    }
                    bot.send_message(chat_id, "You have been unsubscribed from daily challenges.")
                        .send()
                        .await?;
                },
                _ => {}
            }

            Ok(())
        }
    })
    .await;
}
