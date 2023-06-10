use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Choice {
    finish_reason: String,
    message: Message,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAiResponse {
    id: String,
    object: String,
    created: i64,
    model: String,
    choices: Vec<Choice>,
}

pub async fn request_gpt_api(key: &str, query: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .body(
            json!({
                "model": "gpt-3.5-turbo",
                "max_tokens": 300,
                "temperature": 0.2,
                "messages": [
                    {
                        "role": "system",
                        "content": "Complete the ScyllaDB query that answers the question.

                        Schema:
                        CREATE TABLE IF NOT EXISTS ks.s (channel text, id text, sched text, date_at date, create_at timestamp, PRIMARY KEY (channel, date_at, id, create_at))
                        
                        channel: The name of the channel.
                        id: The user's name.
                        sched: What kind of schedule is registered.
                        date_at: The date at which the schedule will be registered. If there is no specific mention of the year, please specify the current year(2023).
                        create_at: The time the query is registered, you can use toTimestamp(now()).
                        
                        Just give me the string query that was created. You shouldn't output a description or anything else.
                        
                        Example answer:
                        INSERT INTO ks.s (channel, id, sched, date_at, create_at) VALUES (?, ?, ?, ?, ?)"
                    },
                    {
                        "role": "user",
                        "content": query
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .await
        .unwrap();

    println!("resp: {:?}", resp);

    if resp.status() != 200 {
        return Err(anyhow!("status code: {}", resp.status()));
    }

    let resp = resp.json::<OpenAiResponse>().await.unwrap();

    println!("resp: {:?}", resp);

    let query = extract_query(&resp);

    if query.starts_with("INSERT") {
        Ok(query)
    } else {
        Err(anyhow!("support only INSERT query"))
    }
}

fn extract_query(resp: &OpenAiResponse) -> String {
    let mut query = String::new();

    for choice in &resp.choices {
        if choice.finish_reason == "stop" {
            query = choice.message.content.to_owned();
            break;
        }
    }

    query
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_request_gpt_api() {
        let key = "open ai key";
        let query = "6월 30일에 봄소풍이라는 일정을 등록해주세요. (channel: home, id: 21kyu)";

        let _ = request_gpt_api(key, query).await;
    }
}
