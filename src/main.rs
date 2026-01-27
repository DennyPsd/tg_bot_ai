// TODO: БД с id чатами пользователей с привязкой по почте
// Запуск бота с сопровождением сообщения о запуске всем пользователям. Возможно и такая же остановка.
// Возможность выбора модели для генерации КН
// Подписка и рассылка на генерацию КН раз в месяц (2-3 число)
use std::env;
use std::sync::{Arc, Mutex};
use teloxide::prelude::*;
use teloxide::types::{ChatId};

use tracing::info;
use tracing_subscriber;

use mail_send::SmtpClientBuilder;
use mail_send::mail_builder::MessageBuilder;

use rusqlite::{Connection, Result};

const VERSION: &str = "0.2.1";

//Два промта. Надо будет переместить их
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

// Инициализация типов хэшмапов для сохранения почты и последней КН
type UserEmails = Arc<Mutex<std::collections::HashMap<u64, String>>>;
type UserCards = Arc<Mutex<std::collections::HashMap<u64, String>>>;


#[tokio::main]
async fn main() -> Result<()> {
    //Инициализация либы логирования
    tracing_subscriber::fmt::init();
    rustls::crypto::ring::default_provider()
    .install_default()
    .expect("Failed to install ring crypto provider");

    // Открываем или создаем базу данных в текущей директории
    let conn = Connection::open("users_mail.db")?;
    let conn = Arc::new(Mutex::new(conn));
    conn.lock().unwrap().execute("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, email TEXT)", [])?;
    println!("БД поднята!");

    // Загружаем почты пользователей из БД в память
let mut loaded_emails = std::collections::HashMap::new();
{
    let conn_guard = conn.lock().unwrap();
    conn_guard.execute(
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, email TEXT NOT NULL)",
        [],
    )?;
    let mut stmt = conn_guard.prepare("SELECT id, email FROM users")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, email) = row?;
        // Telegram user IDs are positive, so casting i64 → u64 is safe
        loaded_emails.insert(id as u64, email);
    }
}
println!("Загружено {} пользователей", loaded_emails.len());

    //Либа для подтяжки данных из файла .env
    dotenvy::dotenv().ok();
    let bot_token = env::var("TG_TOKEN").expect("Токен бота не найден");
    let ai_token = env::var("API_TOKEN").expect("Токен AI не найден");
    info!("Токен AI модели загружен");
    let bot = Bot::new(bot_token);

    let user_emails: UserEmails = Arc::new(Mutex::new(loaded_emails));
    let user_cards: UserCards = Arc::new(Mutex::new(std::collections::HashMap::new()));


    //Инициализация бота и переменных
    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let ai_token = ai_token.clone();
        let user_emails = Arc::clone(&user_emails);
        let user_cards = Arc::clone(&user_cards);
        let conn = Arc::clone(&conn);

        async move {
            //Проверка на сообщения пользователя
            if let Some(text) = msg.text() {
                match text.to_lowercase().as_str() {
                    "/casino" => {
                        bot.send_message(msg.chat.id, "Казино вредит вашему кошельку")
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
                            "Краткая инструкция: \n\
                            Бот создан для генерации текста для Карт Наблюдения (КН) \n\
                            Для этого есть команды: \n\
                            /genoffice - Для генерации текста для КН (работа в офисе) \n\
                            /genzavod - Для генерации текста для КН (работа на заводе) \n\n\
                            Бот может отправить сгенерированную КН на вашу почту: \n\
                            /msg - Отправить КН на почту \n\
                            /setmail - Установить почту для отправки карты наблюдения \n\n\
                            Развлекательные функции: \n\
                            /casino - Для прокрутки казино \n\
                            /darts - Для броска дротика \n\
                            /dice - Для броска кубика \n\n\n\
                            *BETA* \n\
                            Также бота можно использовать для обращения к AI (но пока в рамках одного сообщения)\n\
                            Для этого просто напиши любой текст! \n\n\
                            Бот находится в стадии разработки, сильно его не ругайте",
                        )
                        .await?;
                    }
                    "/genoffice" => {
                        bot.send_message(msg.chat.id, "Генерирую текст для офисника через AI...").await?;
                        match generate_kn(&ai_token, PROMPT_OFFICE.to_string()).await {
                            Ok(response) => {
                                let user_id = msg.from.as_ref()
                                    .expect("Отсутствует информация о пользователе")
                                    .id.0;
                                
                                // Сохраняем сгенерированную карту
                                {
                                    let mut cards = user_cards.lock().unwrap();
                                    cards.insert(user_id, response.clone());
                                }

                                bot.send_message(msg.chat.id, response).await?;
                                //bot.send_message(msg.chat.id, "Отправить эту карту на почту \n/msg").await?;
                                
                                // Выводим почту пользователя, если она установлена
                                if let Some(email) = get_user_email(&user_emails, user_id) {
                                    bot.send_message(msg.chat.id, format!("Отправить эту КН на почту {} ? \n Введите /msg", email)).await?;
                                } else {
                                    bot.send_message(msg.chat.id, "Укажите почту для отправки этой КН \n/setmail").await?;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Ошибка AI: {}", e);
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст, слишком много запросов")
                                    .await?;
                            }
                        }
                    }
                    "/genzavod" => {
                        bot.send_message(msg.chat.id, "Генерирую текст для заводчанина через AI...")
                            .await?;
                        match generate_kn(&ai_token, PROMPT_ZAVOD.to_string()).await {
                            Ok(response) => {
                                let user_id = msg.from.as_ref()
                                    .expect("Отсутствует информация о пользователе")
                                    .id.0;
                                
                                // Сохраняем сгенерированную карту
                                {
                                    let mut cards = user_cards.lock().unwrap();
                                    cards.insert(user_id, response.clone());
                                }

                                bot.send_message(msg.chat.id, response).await?;
                               // Выводим почту пользователя, если она установлена
                                if let Some(email) = get_user_email(&user_emails, user_id) {
                                    bot.send_message(msg.chat.id, format!("Отправить эту КН на почту {} ? \n Введите /msg", email)).await?;
                                } else {
                                    bot.send_message(msg.chat.id, "Укажите почту для отправки этой КН \n/setmail").await?;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Ошибка AI: {}", e);
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст, слишком много запросов")
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

                    //Ожидание ввода почты после команды /setmail
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
                        
                        match conn.lock().unwrap().execute("INSERT OR REPLACE INTO users (id, email) values (?1, ?2)",&[&user_id.to_string(), &email]) {
                            Ok(_) => {
                                tracing::info!("Записана почта {} для чата {}", email, user_id);
                            }
                            Err(e) => {
                                tracing::error!("Ошибка БД: {}", e);
                            }
                        };

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
                    "/shutdown" => {
                        let user_id = msg.from.as_ref().unwrap().id.0;
                        if user_id == 465320725 {
                        bot.send_message(msg.chat.id, "Бот выключен!").await?;
                        use std::process::exit;
                        exit(0);
                        };
                    }
                    "/version" => {
                        bot.send_message(msg.chat.id, format!("Версия бота: {}", VERSION)).await?;
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
                                bot.send_message(msg.chat.id, "Не удалось сгенерировать текст, слишком много запросов")
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
Ok(())
}

//Функция обращения к AI по API
async fn generate_kn(
    ai_token: &str,
    user_msg: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let model = env::var("MODEL").expect("Модель AI не найдена");
    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": model,
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

//Функция отправки сообщения на почту пользователя
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

/// Возвращает почту пользователя, если она установлена и не является placeholder "waiting"
fn get_user_email(user_emails: &UserEmails, user_id: u64) -> Option<String> {
    let emails = user_emails.lock().unwrap();
    emails.get(&user_id).filter(|e| *e != "waiting").cloned()
}
