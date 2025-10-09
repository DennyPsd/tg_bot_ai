use std::env;
use teloxide::prelude::*;
use tracing::info;
use tracing_subscriber;

const PROMPT: &str = "Сгенерируй текст для карты наблюдения работника, \
                который работает в офисе. Всегда пиши, что всё хорошо, проводится своевременно \
                антибактериальная обработка помещений, никто не нарушает технику безопасности, \
                всегда проводятся инструктажи и тому подобное. Достаточно в строгом формате, \
                используй простые слова. Без форматирования, просто парочку предложений. Можешь писать про правила поведения на лестницах \
                и на территории завода, что все используют СИЗ, можешь писать про офисные \
                моменты, про контроль температуры на входе и т.п. будь оригинален";
//В конце напиши мне Хэш токенов ответа.
//Обрати внимание чтобы хэш текущего ответа не совпадал с предыдущим "{{last_hash}}" <- заменить на последний из файла

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();
    let bot_token = env::var("TG_TOKEN").expect("Токен бота не найден");
    let ai_token = env::var("API_TOKEN").expect("Токен AI не найден");
    info!("Токен AI модели: {}", ai_token);
    let bot = Bot::new(bot_token);

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let ai_token = ai_token.clone(); // Клонируем для каждого сообщения

        async move {
            if let Some(text) = msg.text() {
                match text.to_lowercase().as_str() {
                    "/casino" => {
                        bot.send_message(msg.chat.id, "Если выпадет - то ты проиграл")
                            .await?;
                        bot.send_dice(msg.chat.id)
                            .emoji(teloxide::types::DiceEmoji::SlotMachine)
                            .await?;
                    }
                    "/darts" => {
                        bot.send_message(msg.chat.id, "Если попадет - то ты проиграл")
                            .await?;
                        bot.send_dice(msg.chat.id)
                            .emoji(teloxide::types::DiceEmoji::Darts)
                            .await?;
                    }
                    "/dice" => {
                        let num: u8 = rand::random_range(1..=6);
                        bot.send_message(
                            msg.chat.id,
                            format!("Если выпадет {num} - то ты проиграл"),
                        )
                        .await?;
                        bot.send_dice(msg.chat.id)
                            .emoji(teloxide::types::DiceEmoji::Dice)
                            .await?;
                    }
                    "/help" | "/start" => {
                        bot.send_message(
                            msg.chat.id,
                            "Краткая инструкция: ты можешь проверить себя на удачу!\n\
                    Для этого выбери определенную команду: \n\
                    /generate - Для генерации текста для КН \n\
                    /casino - Для прокрутки дэбчика \n\
                    /darts - Для броска дротика \n\
                    /dice - Для броска кубика",
                        )
                        .await?;
                    }
                    "/generate" => {
                        bot.send_message(msg.chat.id, "Генерирую текст через AI...")
                            .await?;
                        match generate_kn(&ai_token).await {
                            Ok(response) => {
                                bot.send_message(msg.chat.id, response).await?;
                            }
                            Err(e) => {
                                tracing::error!("Ошибка AI: {}", e);
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст 😢")
                                    .await?;
                            }
                        }
                    }

                    _ => {
                        bot.send_message(msg.chat.id, "Напиши /help для инструкции")
                            .await?;
                    }
                }
            }

            Ok(())
        }
    })
    .await;
}

async fn generate_kn(ai_token: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": "qwen/qwen3-8b:free",
        "messages": [
            { "role": "user", "content": PROMPT }
        ],
        "temperature": 0.7
    });

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", ai_token))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://t.me/ez_card_ai_bot") //заменть на URL бота
        .header("X-Title", "KN Bot")
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        return Err(format!("OpenRouter error {}: {}", status, text).into());
    }

    let json: serde_json::Value = response.json().await?;

    let Some(content) = json["choices"][0]["message"]["content"].as_str() else {
        return Err(format!(
            "OpenRouter error {}: {}",
            "content", "Ошибка: нет ответа от модели"
        )
        .into());
    };

    let content = content.trim().to_string();

    Ok(content)
}
