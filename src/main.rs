use std::env;
use std::sync::{Arc, Mutex};
use teloxide::prelude::*;
use teloxide::types::ChatId;

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

    let stickers = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let mut stickers = stickers.lock().unwrap();
        stickers.push(
            "CAACAgIAAxkBAAE8Pkpo57Jl9ZyDCcpAsctvnyZMUIzQewACKoMAAoMIeEmxqZaLd0ZwFDYE".to_string(),
        );
        stickers.push(
            "CAACAgIAAxkBAAE8Pjxo57F6ZPf1mw8YUT0D0EzGmEvs9QACt3YAAiDfeEkfyzgh-xukJzYE".to_string(),
        );
        stickers.push(
            "CAACAgIAAxkBAAE8PjVo57EKBtaS6gom3vpyEQ_UQz1oNgACGT8AAhwAAXFLdxvIwCabAAG-NgQ"
                .to_string(),
        );
        //info!("Добавлены хардкодные стикеры: {:?}", stickers_list);
    }

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let ai_token = ai_token.clone(); // Клонируем для каждого сообщения
        //Разобраться как сделать ввод в массив. А то он пустой в /stickers
        let stickers = Arc::clone(&stickers);
        async move {
            if let Some(sticker) = msg.sticker() {
                let file_id = sticker.file.id.to_string();
                {
                    let mut stickers_list = stickers.lock().unwrap();
                    stickers_list.push(file_id);
                    info!("{:?}", stickers_list);
                }
                bot.send_message(msg.chat.id, "✅ Стикер добавлен!").await?;
            }

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
                    "/sticker" => {
                        let sticker_to_send = {
                            let list = stickers.lock().unwrap();
                            if list.is_empty() {
                                None
                            } else {
                                let idx = rand::random_range(0..list.len());
                                Some(list[idx].clone())
                            }
                        };

                        if let Some(sticker_id) = sticker_to_send {
                            bot.send_sticker(
                                msg.chat.id,
                                teloxide::types::InputFile::file_id(sticker_id.clone().into()),
                            )
                            .await?;
                        }
                    }
                    "/help" | "/start" => {
                        bot.send_message(
                            msg.chat.id,
                            "Краткая инструкция: \n\
                            Бота можно для обращения к AI (но пока в рамках одного сообщения)\n\
                            Для этого просто напиши сообщение! \n\
                            \n\
                            Также есть определенные команду: \n\
                            /generate - Для генерации текста для КН \n\
                            /casino - Для прокрутки казино \n\
                            /darts - Для броска дротика \n\
                            /dice - Для броска кубика \n\
                            /sticker - Для отправки рандомного стикера \n\
                            Или можешь прислать свой стикер и я его запомню",
                        )
                        .await?;
                    }
                    "/generate" => {
                        bot.send_message(msg.chat.id, "Генерирую текст через AI...")
                            .await?;
                        match generate_kn(&ai_token, PROMPT.to_string()).await {
                            Ok(response) => {
                                bot.send_message(msg.chat.id, response).await?;
                            }
                            Err(e) => {
                                tracing::error!("Ошибка AI: {}", e);
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст")
                                    .await?;
                            }
                        }
                    }

                    _ => {
                        //bot.send_message(msg.chat.id, "Напиши /help для инструкции").await?;
                        //info!("{}", msg.chat.id);
                        bot.send_message(
                            ChatId(465320725),
                            format!(
                                "{}: {}",
                                msg.from().unwrap().first_name,
                                msg.text().unwrap().to_string()
                            ),
                        )
                        .await?;

                        bot.send_message(msg.chat.id, "Делаю запрос в AI...")
                            .await?;
                        match generate_kn(&ai_token, msg.text().unwrap().to_string()).await {
                            Ok(response) => {
                                bot.send_message(msg.chat.id, response).await?;
                            }
                            Err(e) => {
                                tracing::error!("Ошибка AI: {}", e);
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст")
                                    .await?;
                            }
                        }
                    }
                }
            }

            Ok(())
        }
    })
    .await;
}

async fn generate_kn(
    ai_token: &str,
    user_msg: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": "qwen/qwen3-8b:free",
        "messages": [
            { "role": "user", "content": user_msg }
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
