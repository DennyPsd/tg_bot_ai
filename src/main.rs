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
use chrono::prelude::*;


const VERSION: &str = "0.3.3";



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

// Инициализация типов хэшмапов для сохранения почты, последней КН и текущей модели
type UserEmails = Arc<Mutex<std::collections::HashMap<u64, String>>>;
type UserCards = Arc<Mutex<std::collections::HashMap<u64, String>>>;
type CurrentModel = Arc<Mutex<String>>;


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
    
    // Создаем таблицы
    {
        let conn_guard = conn.lock().unwrap();
        conn_guard.execute("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, email TEXT)", [])?;
        conn_guard.execute(
            "CREATE TABLE IF NOT EXISTS subscriptions (
                user_id INTEGER PRIMARY KEY,
                email TEXT NOT NULL,
                subscribed_at TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1
            )",
            [],
        )?;
    }
   
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
    
    // Загружаем модель из env
    let initial_model = env::var("MODEL").expect("Модель AI не найдена");
    let current_model: CurrentModel = Arc::new(Mutex::new(initial_model));
    //info!("Модель AI: {}", current_model.lock().unwrap());

    // Проверяем, сегодня ли 2-е число — тогда отправляем рассылку
let now = Utc::now();

if now.day() == 2 {
    tracing::info!("Сегодня 2-е число — запускаем ежемесячную рассылку");
    let conn_for_newsletter = Arc::clone(&conn);
    let ai_token_for_newsletter = ai_token.clone();
    let user_emails_for_newsletter = Arc::clone(&user_emails);
    let model_for_newsletter = current_model.lock().unwrap().clone();

    // Запускаем рассылку (без spawn, чтобы не блокировать старт бота)
    if let Err(e) = send_monthly_newsletter(
        &conn_for_newsletter,
        &ai_token_for_newsletter,
        &user_emails_for_newsletter,
        &model_for_newsletter,
    ).await {
        tracing::error!("Ошибка рассылки: {}", e);
    } else {
        tracing::info!("Рассылка успешно отправлена");
    }
}

    //Инициализация бота и переменных
    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let ai_token = ai_token.clone();
        let user_emails = Arc::clone(&user_emails);
        let user_cards = Arc::clone(&user_cards);
        let conn = Arc::clone(&conn);
        let current_model = current_model.clone();

        async move {
            //Проверка на сообщения пользователя
            if let Some(text) = msg.text() {
                match text.to_lowercase().as_str() {
                    text => {
                        let user_id = msg.from.as_ref().unwrap().id.0;
                        
                        // Обработка /setmodel с аргументами
                        if text.to_lowercase().starts_with("/setmodel") {
                            if user_id == 465320725 {
                                let args: Vec<&str> = text.split_whitespace().collect();
                                if args.len() < 2 {
                                    let current = current_model.lock().unwrap().clone();
                                    bot.send_message(msg.chat.id, format!("Текущая модель: {}\nИспользование: /setmodel <название модели>\nНапример: /setmodel google/gemini-2.0-flash-thinking-exp-01-21", current)).await?;
                                } else {
                                    let new_model = args[1..].join(" ");
                                    {
                                        let mut model = current_model.lock().unwrap();
                                        *model = new_model.clone();
                                    }
                                    info!("Модель изменена на: {}", new_model);
                                    bot.send_message(msg.chat.id, format!("✅ Модель изменена на: {}", new_model)).await?;
                                }
                            }
                        } else {
                            // Обычная маршрутизация команд
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
                                        Ежемесячная рассылка: \n\
                                        /subscribe - Подписаться на рассылку КН (2-го числа каждого месяца) \n\
                                        /unsubscribe - Отписаться от рассылки \n\
                                        /subscription_status - Проверить статус подписки \n\n\
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
                                    let model = current_model.lock().unwrap().clone();
                                    bot.send_message(msg.chat.id, format!("Генерирую текст для офисника через AI...")).await?;
                                    match generate_kn(&ai_token, &model, PROMPT_OFFICE.to_string()).await {
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
                                    let model = current_model.lock().unwrap().clone();
                                    bot.send_message(msg.chat.id, format!("Генерирую текст для заводчанина через AI..."))
                                        .await?;
                                    match generate_kn(&ai_token, &model, PROMPT_ZAVOD.to_string()).await {
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
                                    let user_id = msg.from.as_ref().unwrap().id.0;
                                    let card_text = {
                                        let cards = user_cards.lock().unwrap();
                                        cards.get(&user_id).cloned()
                                    };
                                    
                                    if let Some(card_text) = card_text {
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
                                    bot.send_message(msg.chat.id, format!("Версия бота: {} \nВерсия модели: {:?}", VERSION, current_model.lock().unwrap())).await?;
                                }
                                "/subscribe" => {
                                    // Подписка на рассылку КН
                                    let user_id = msg.from.as_ref().unwrap().id.0;
                                    
                                    // Проверяем, установлена ли почта
                                    if let Some(email) = get_user_email(&user_emails, user_id) {
                                        match set_subscription(&conn, user_id, &email, true).await {
                                            Ok(true) => {
                                                bot.send_message(
                                                    msg.chat.id,
                                                    "✅ Вы успешно подписались на рассылку карт наблюдения!\n\
                                                     📅 Рассылка будет производиться 2-го числа каждого месяца.",
                                                ).await?;
                                            }
                                            Ok(false) => {
                                                bot.send_message(
                                                    msg.chat.id,
                                                    "Вы уже подписаны на рассылку.",
                                                ).await?;
                                            }
                                            Err(e) => {
                                                tracing::error!("Ошибка БД при подписке: {}", e);
                                                bot.send_message(
                                                    msg.chat.id,
                                                    "Ошибка при подписке. Попробуйте позже.",
                                                ).await?;
                                            }
                                        }
                                    } else {
                                        bot.send_message(
                                            msg.chat.id,
                                            "Сначала укажите почту командой /setmail",
                                        ).await?;
                                    }
                                }
                                "/unsubscribe" => {
                                    // Отписка от рассылки КН
                                    let user_id = msg.from.as_ref().unwrap().id.0;
                                    
                                    match set_subscription(&conn, user_id, "", false).await {
                                        Ok(true) => {
                                            bot.send_message(
                                                msg.chat.id,
                                                "Вы отписались от рассылки карт наблюдения.",
                                            ).await?;
                                        }
                                        Ok(false) => {
                                            bot.send_message(
                                                msg.chat.id,
                                                "Вы не были подписаны на рассылку.",
                                            ).await?;
                                        }
                                        Err(e) => {
                                            tracing::error!("Ошибка БД при отписке: {}", e);
                                            bot.send_message(
                                                msg.chat.id,
                                                "Ошибка при отписке.",
                                            ).await?;
                                        }
                                    }
                                }
                                "/subscription_status" => {
                                    // Проверка статуса подписки
                                    let user_id = msg.from.as_ref().unwrap().id.0;
                                    
                                    match get_subscription_status(&conn, user_id).await {
                                        Ok(Some((email, subscribed_at))) => {
                                            let formatted_date = subscribed_at.format("%d.%m.%Y %H:%M").to_string();
                                            bot.send_message(
                                                msg.chat.id,
                                                format!(
                                                    "📋 Статус подписки:\n\
                                                     ✅ Подписан на рассылку\n\
                                                     📧 Почта: {}\n\
                                                     📅 Подписан с: {}",
                                                    email,
                                                    formatted_date
                                                ),
                                            ).await?;
                                        }
                                        Ok(None) => {
                                            bot.send_message(
                                                msg.chat.id,
                                                "Вы не подписаны на рассылку.\n\
                                                 Для подписки используйте /subscribe",
                                            ).await?;
                                        }
                                        Err(e) => {
                                            tracing::error!("Ошибка БД при проверке статуса: {}", e);
                                            bot.send_message(
                                                msg.chat.id,
                                                "Ошибка при проверке статуса.",
                                            ).await?;
                                        }
                                    }
                                }
                                "/test" => {
                                    // Принудительная рассылка всем подписчикам
                                    let user_id = msg.from.as_ref().unwrap().id.0;
                                    
                                    // Проверяем, что это админ (id 465320725)
                                    if user_id == 465320725 {
                                        bot.send_message(msg.chat.id, "🚀 Запускаю тестовую рассылку в фоне...")
                                            .await?;
                                        
                                        // Получаем список подписчиков синхронно (до spawn)
                                        let subscribers: Vec<(u64, String)> = {
                                            let conn_guard = conn.lock().unwrap();
                                            let mut stmt = conn_guard.prepare(
                                                "SELECT user_id, email FROM subscriptions WHERE is_active = 1"
                                            ).unwrap();
                                            stmt.query_map([], |row| {
                                                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
                                            }).unwrap().filter_map(|row| row.ok()).collect()
                                        };
                                        
                                        if subscribers.is_empty() {
                                            bot.send_message(msg.chat.id, "Нет активных подписчиков для рассылки")
                                                .await?;
                                        } else {
                                            // Запускаем рассылку в фоне с готовыми данными
                                            let ai_token_clone = ai_token.clone();
                                            let chat_id = msg.chat.id;
                                            let bot_clone = bot.clone();
                                            let model_clone = current_model.lock().unwrap().clone();
                                            
                                            tokio::spawn(async move {
                                                send_newsletter_to_subscribers(subscribers, &ai_token_clone, &model_clone, &bot_clone, chat_id).await;
                                            });
                                        }
                                    } else {
                                        bot.send_message(
                                            msg.chat.id,
                                            "⛔ У вас нет доступа к этой команде.",
                                        ).await?;
                                    }
                                }

                                _ => {
                                    let model = current_model.lock().unwrap().clone();
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
                                    match generate_kn(&ai_token, &model, msg.text().unwrap().to_string()).await {
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
    model: &str,
    user_msg: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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

// Устанавливает статус подписки пользователя
// Возвращает Ok(true) если подписка была изменена, Ok(false) если статус уже был таким
async fn set_subscription(
    conn: &Arc<Mutex<Connection>>,
    user_id: u64,
    email: &str,
    is_active: bool,
) -> Result<bool, rusqlite::Error> {
    let conn_guard = conn.lock().unwrap();
    
    // Проверяем текущий статус
    let current_status: Option<i32> = conn_guard.query_row(
        "SELECT is_active FROM subscriptions WHERE user_id = ?1",
        [user_id as i64],
        |row| row.get(0),
    ).ok();
    
    // Если статус уже такой - ничего не делаем
    if current_status == Some(if is_active { 1 } else { 0 }) {
        return Ok(false);
    }
    
    // Обновляем или создаем запись
    conn_guard.execute(
        "INSERT OR REPLACE INTO subscriptions (user_id, email, subscribed_at, is_active)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            user_id as i64,
            email,
            chrono::Utc::now().to_rfc3339(),
            if is_active { 1 } else { 0 }
        ],
    )?;
    
    Ok(true)
}

/// Получает статус подписки пользователя
async fn get_subscription_status(
    conn: &Arc<Mutex<Connection>>,
    user_id: u64,
) -> Result<Option<(String, chrono::DateTime<chrono::Utc>)>, rusqlite::Error> {
    let conn_guard = conn.lock().unwrap();
    
    let result: Result<(String, String), rusqlite::Error> = conn_guard.query_row(
        "SELECT email, subscribed_at FROM subscriptions WHERE user_id = ?1 AND is_active = 1",
        [user_id as i64],
        |row| {
            Ok((row.get(0)?, row.get(1)?))
        },
    );
    
    match result {
        Ok((email, subscribed_at)) => {
            let parsed_date = chrono::DateTime::parse_from_rfc3339(&subscribed_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            Ok(Some((email, parsed_date)))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Функция для отправки рассылки (вызывается по расписанию 2-го числа каждого месяца)
async fn send_monthly_newsletter(
    conn: &Arc<Mutex<Connection>>,
    ai_token: &str,
    user_emails: &UserEmails,
    current_model: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn_guard = conn.lock().unwrap();
    
    // Получаем всех активных подписчиков
    let mut stmt = conn_guard.prepare(
        "SELECT user_id, email FROM subscriptions WHERE is_active = 1"
    )?;
    
    let subscribers: Vec<(u64, String)> = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
    })?.filter_map(|row| row.ok()).collect();
    
    drop(stmt);
    drop(conn_guard);
    
    // Генерируем и отправляем КН каждому подписчику
    for (user_id, email) in subscribers {
        // Генерируем КН (чередуем между офисом и заводом)
        let prompt = if user_id % 2 == 0 {
            PROMPT_OFFICE.to_string()
        } else {
            PROMPT_ZAVOD.to_string()
        };
        
        match generate_kn(ai_token, &current_model, PROMPT_OFFICE.to_string()).await {
            Ok(kn_text) => {
                // Отправляем на почту
                let pass_gm = env::var("PASS_GM")?;
                let message = MessageBuilder::new()
                    .from(("Bot AI", "cardaibot@gmail.com"))
                    .to(vec![("User", email.as_str())])
                    .subject("Карта наблюдения - ежемесячная рассылка")
                    .text_body(format!("{} \n\n\nBy tgbot: @ez_card_ai_bot", kn_text));
                
                SmtpClientBuilder::new("smtp.gmail.com", 587)
                    .implicit_tls(false)
                    .credentials(("cardaibot@gmail.com", pass_gm.as_str()))
                    .connect()
                    .await?
                    .send(message)
                    .await?;
                
                tracing::info!("Рассылка отправлена пользователю {} на почту {}", user_id, email);
            }
            Err(e) => {
                tracing::error!("Ошибка генерации КН для пользователя {}: {}", user_id, e);
            }
        }
        
        // Небольшая пауза между отправками, чтобы не спамить SMTP
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
    
    Ok(())
}

/// Функция для рассылки с готовым списком подписчиков (используется для /test)
async fn send_newsletter_to_subscribers(
    subscribers: Vec<(u64, String)>,
    ai_token: &str,
    current_model: &str,
    bot: &Bot,
    chat_id: ChatId,
) {
    let mut sent_count = 0;
    
    for (user_id, email) in &subscribers {
        // Генерируем КН (чередуем между офисом и заводом)
        let prompt = if user_id % 2 == 0 {
            PROMPT_OFFICE.to_string()
        } else {
            PROMPT_ZAVOD.to_string()
        };
        
        match generate_kn(ai_token, &current_model, prompt).await {
            Ok(kn_text) => {
                // Отправляем на почту
                let pass_gm = match env::var("PASS_GM") {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("Ошибка получения пароля: {}", e);
                        let _ = bot.send_message(chat_id, "Ошибка конфигурации SMTP").await;
                        return;
                    }
                };
                
                let message = MessageBuilder::new()
                    .from(("Bot AI", "cardaibot@gmail.com"))
                    .to(vec![("User", email.as_str())])
                    .subject("Карта наблюдения - тестовая рассылка")
                    .text_body(format!("{} \n\n\nBy tgbot: @ez_card_ai_bot", kn_text));
                
                if let Err(e) = SmtpClientBuilder::new("smtp.gmail.com", 587)
                    .implicit_tls(false)
                    .credentials(("cardaibot@gmail.com", pass_gm.as_str()))
                    .connect()
                    .await
                {
                    tracing::error!("Ошибка подключения SMTP: {}", e);
                    continue;
                }
                
                if let Err(e) = SmtpClientBuilder::new("smtp.gmail.com", 587)
                    .implicit_tls(false)
                    .credentials(("cardaibot@gmail.com", pass_gm.as_str()))
                    .connect()
                    .await
                    .unwrap()
                    .send(message)
                    .await
                {
                    tracing::error!("Ошибка отправки письма: {}", e);
                    continue;
                }
                
                tracing::info!("Тестовая рассылка отправлена пользователю {} на почту {}", user_id, email);
                sent_count += 1;
            }
            Err(e) => {
                tracing::error!("Ошибка генерации КН для пользователя {}: {}", user_id, e);
            }
        }
        
        // Небольшая пауза между отправками, чтобы не спамить SMTP
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
    
    let _ = bot.send_message(chat_id, format!("✅ Тестовая рассылка завершена! Отправлено {} писем.", sent_count)).await;
}