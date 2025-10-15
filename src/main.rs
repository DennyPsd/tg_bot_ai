use std::env;
use std::sync::{Arc, Mutex};
use teloxide::prelude::*;
use teloxide::types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup};

use tracing::info;
use tracing_subscriber;

use mail_send::SmtpClientBuilder;
use mail_send::mail_builder::MessageBuilder;

const PROMPT_OFFICE: &str = "Сгенерируй текст для карты наблюдения работника, \
                который работает в офисе. Всегда пиши, что всё хорошо, проводится своевременно \
                антибактериальная обработка помещений, никто не нарушает технику безопасности, \
                всегда проводятся инструктажи и тому подобное. Достаточно в строгом формате, \
                используй простые слова. Без форматирования, просто парочку предложений. Можешь писать про правила поведения на лестницах \
                и вблизи территории офиса, можешь писать про офисные \
                моменты, про контроль температуры на входе и т.п. будь оригинален и придумывай свои идеи для карты. Не пиши про личную \
                гигиену сотрудников. Напиши 2-3 небольших предложения.";

const PROMPT_ZAVOD: &str = "Сгенерируй текст для карты наблюдения работника, \
                который работает в офисе. Всегда пиши, что всё хорошо, проводится своевременно \
                антибактериальная обработка помещений, никто не нарушает технику безопасности, \
                всегда проводятся инструктажи и тому подобное. Достаточно в строгом формате, \
                используй простые слова. Без форматирования, просто парочку предложений. Можешь писать про правила поведения на лестницах \
                и на территории завода, что все используют СИЗ, можешь писать про офисные \
                моменты, про контроль температуры на входе и т.п. будь оригинален и придумывай свои идеи для карты. Не пиши про личную \
                гигиену сотрудников. Напиши 2-3 небольших предложения.";

type UserEmails = Arc<Mutex<std::collections::HashMap<u64, String>>>;
type UserCards = Arc<Mutex<std::collections::HashMap<u64, String>>>;


//В конце напиши мне Хэш токенов ответа.
//Обрати внимание чтобы хэш текущего ответа не совпадал с предыдущим "{{last_hash}}" <- заменить на последний из файла

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();
    let bot_token = env::var("TG_TOKEN").expect("Токен бота не найден");
    let ai_token = env::var("API_TOKEN").expect("Токен AI не найден");
    info!("Токен AI модели загружен");
    let bot = Bot::new(bot_token);

    let stickers = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let mut stickers = stickers.lock().unwrap();
        stickers.push(
            "CAACAgIAAxkBAAE8ch5o7fs9-aicqE8g3laBhd-LbSmXzQACXXgAAojD8Uhh82UePK7UITYE".to_string(),
        );
        stickers.push(
            "CAACAgIAAxkBAAE8cjFo7f0FLdiniZp3oPpOrIPsyvKY2QACaH8AAsIQkUgoZozF4ua9yTYE".to_string(),
        );
        stickers.push(
            "CAACAgIAAxkBAAE8cjZo7f0gckT1uosS4f1Q8dUU1baw1wAClncAAvy-kEhE7Nf1NE4HtjYE".to_string(),
        );
        stickers.push(
            "CAACAgIAAxkBAAE8cjxo7f1RjGXNQ0EgeyeMZsR7AtyI3QAC8HEAAv4ikEgGN5siLIeNaTYE".to_string(),
        );
        stickers.push(
            "CAACAgIAAxkBAAE8cj5o7f2OcHucJKBF1xAAAYzK4fDZLgMAAkCFAALFhZFI4xH8yB0MQFg2BA"
                .to_string(),
        );
    }
    let user_emails: UserEmails = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let user_cards: UserCards = Arc::new(Mutex::new(std::collections::HashMap::new()));



    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let ai_token = ai_token.clone();
        let stickers = Arc::clone(&stickers);
        let user_emails = Arc::clone(&user_emails);
        let user_cards = Arc::clone(&user_cards);

        async move {
            if let Some(sticker) = msg.sticker() {
                let file_id = sticker.file.id.to_string();
                {
                    let mut stickers_list = stickers.lock().unwrap();
                    stickers_list.push(file_id);
                    //info!("{:?}", stickers_list);
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
                            Бота можно использовать для обращения к AI (но пока в рамках одного сообщения)\n\
                            Для этого просто напиши любой текст! \n\
                            \n\
                            Также есть определенные команды: \n\
                            /genoffice - Для генерации текста для КН (работа в офисе) \n\
                            /genzavod - Для генерации текста для КН (работа на заводе) \n\
                            /msg - Отправить карту наблюдения на почту \n\
                            /setmail - Установить почту для отправки карты наблюдения \n\n\
                            Развлекательные функции: \n\
                            /casino - Для прокрутки казино \n\
                            /darts - Для броска дротика \n\
                            /dice - Для броска кубика \n\
                            /sticker - Для отправки рандомного стикера \n\
                            Или можешь прислать свой стикер и я его запомню",
                        )
                        .await?;
                    }
                    "/genoffice" => {
                        bot.send_message(msg.chat.id, "Генерирую текст для офисника через AI...").await?;
                        match generate_kn(&ai_token, PROMPT_OFFICE.to_string()).await {
                            Ok(response) => {

                                let user_id = msg.from.as_ref().unwrap().id.0;
                                {
                                    let mut cards = user_cards.lock().unwrap();
                                    cards.insert(user_id, response.clone());
                                }

                                //let button = InlineKeyboardMarkup::new([[InlineKeyboardButton::callback("Отправить на почту", "send_now"),]]);
                                //bot.send_message(msg.chat.id, response).reply_markup(button).await?;
                                bot.send_message(msg.chat.id, response).await?;
                                bot.send_message(msg.chat.id, "Отправить эту карту на почту \n/msg").await?;

                            }
                            Err(e) => {
                                tracing::error!("Ошибка AI: {}", e);
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст")
                                    .await?;
                            }
                        }
                    }
                    "/genzavod" => {
                        bot.send_message(msg.chat.id, "Генерирую текст для заводчанина через AI...")
                            .await?;
                        match generate_kn(&ai_token, PROMPT_ZAVOD.to_string()).await {
                            Ok(response) => {
                                 let user_id = msg.from.as_ref().unwrap().id.0;
                                {
                                    let mut cards = user_cards.lock().unwrap();
                                    cards.insert(user_id, response.clone());
                                }

                                //let button = InlineKeyboardMarkup::new([[InlineKeyboardButton::callback("Отправить на почту", "send_now"),]]);
                                //bot.send_message(msg.chat.id, response).reply_markup(button).await?;
                                bot.send_message(msg.chat.id, response).await?;
                                bot.send_message(msg.chat.id, "Отправить эту карту на почту \n/msg").await?;
                            }
                            Err(e) => {
                                tracing::error!("Ошибка AI: {}", e);
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст")
                                    .await?;
                            }
                        }
                    }
                    "/msg" => {
                        //send_mail().await;
                        //send_mail(bot.clone(), Arc::clone(&user_emails), msg.clone()).await;
                        let user_id = msg.from.as_ref().unwrap().id.0;
                        let card_text = {
                            let cards = user_cards.lock().unwrap();
                            cards.get(&user_id).cloned()
                        };
                        
                        if let Some(card_text) = card_text {
                            // Отправляем сохраненную карту на почту
                            if let Err(e) = send_mail(bot.clone(), Arc::clone(&user_emails), msg.clone(), card_text).await {
                                bot.send_message(msg.chat.id, format!("Ошибка отправки: {}", e)).await?;
                            }
                        } else {
                            bot.send_message(msg.chat.id, "Сначала сгенерируйте карту наблюдения").await?;
                        }
                    }

                    text if {
                        let user_id = msg.from.as_ref().unwrap().id.0;
                        let emails = user_emails.lock().unwrap();
                        emails.contains_key(&user_id) && emails[&user_id] == "waiting"
                    } => {
                        let user_id = msg.from.as_ref().unwrap().id.0;
                        let email = text.trim().to_string();
                        
                        {
                            let mut emails = user_emails.lock().unwrap();
                            emails.insert(user_id, email.clone());
                        }
                        
                        bot.send_message(msg.chat.id, format!("Почта {} успешно сохранена!", email)).await?;
                    }

                    "/setmail" => {
                        let user_id = msg.from.as_ref().unwrap().id.0;
                        {
                            let mut emails = user_emails.lock().unwrap();
                            emails.insert(user_id, "waiting".to_string());
                        }
                        bot.send_message(msg.chat.id, "Напиши почту:").await?;
                    }

                    _ => {
                        bot.send_message(
                            ChatId(465320725),
                            format!(
                                "{}: {}",
                                msg.from.as_ref().unwrap().first_name,
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

async fn send_mail(bot: Bot, user_emails: UserEmails, msg: Message, card_text: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().unwrap().id.0;
    
    let email = {
        let emails = user_emails.lock().unwrap();
        emails.get(&user_id).cloned()
    };
    
    if let Some(email) = email {
        if email == "waiting" {
            bot.send_message(msg.chat.id, "Сначала установи почту командой \n/setmail").await?;
            return Ok(());
        }
        
        // остальной код отправки почты
        let pass_gm = env::var("PASS_GM").expect("Токен почты не найден");
        let message = MessageBuilder::new()
            .from(("Bot AI", "cardaibot@gmail.com"))
            .to(vec![("User", email.as_str())])
            .subject("Карта наблюдения")
            .text_body(format!("{} \n\n\nBy tgbot: @ez_card_ai_bot", card_text));
            
        SmtpClientBuilder::new("smtp.gmail.com", 587)
            .implicit_tls(false)
            .credentials(("cardaibot@gmail.com", pass_gm.as_str()))
            .connect()
            .await?
            .send(message)
            .await?;
            
        bot.send_message(msg.chat.id, format!("Сообщение отправлено на {}", email)).await?;
    } else {
        bot.send_message(msg.chat.id, "Сначала установи почту командой \n/setmail").await?;
    }
    
    Ok(())
}
