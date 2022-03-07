use redis::AsyncCommands;
use teloxide::{
    dispatching2::dialogue::{serializer::Json, RedisStorage, Storage},
    macros::DialogueState,
    payloads::SendMessageSetters,
    prelude2::*,
    RequestError,
};
use thiserror::Error;

use check_car_plates_telegram_bot::*;

type MyDialogue = Dialogue<State, RedisStorage<Json>>;
type StorageError = <RedisStorage<Json> as Storage<State>>::Error;

#[derive(Debug, Error)]
enum Error {
    #[error("error from Telegram: {0}")]
    TelegramError(#[from] RequestError),

    #[error("error from storage: {0}")]
    StorageError(#[from] StorageError),
}

#[derive(DialogueState, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[handler_out(anyhow::Result<()>)]
pub enum State {
    #[handler(handle_start)]
    Start,

    #[handler(handle_awaiting_requests)]
    AwaitingRequests { contact: teloxide::types::Contact },
}

impl Default for State {
    fn default() -> Self {
        Self::Start
    }
}

struct AppState {
    redis_connection: tokio::sync::Mutex<redis::aio::Connection>,
}

#[tokio::main]
async fn main() {
    teloxide::enable_logging!();
    log::info!("Starting dialogue_bot...");

    let bot = Bot::from_env().auto_send();
    // You can also choose serializer::JSON or serializer::CBOR
    // All serializers but JSON require enabling feature
    // "serializer-<name>", e. g. "serializer-cbor"
    // or "serializer-bincode"
    let storage = RedisStorage::open("redis://127.0.0.1:6379", Json)
        .await
        .unwrap();

    let app_state = AppState {
        redis_connection: tokio::sync::Mutex::new(
            redis::Client::open("redis://127.0.0.1:6379")
                .unwrap()
                .get_async_connection()
                .await
                .unwrap(),
        ),
    };

    let handler = Update::filter_message()
        .enter_dialogue::<Message, RedisStorage<Json>, State>()
        .dispatch_by::<State>();

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![std::sync::Arc::new(app_state), storage])
        .build()
        .setup_ctrlc_handler()
        .dispatch()
        .await;
}

fn request_phone_number_confirmation_keyboard() -> teloxide::types::KeyboardMarkup {
    teloxide::types::KeyboardMarkup::new(vec![vec![teloxide::types::KeyboardButton::new(
        "Підтвердити мій номер телефону",
    )
    .request(teloxide::types::ButtonRequest::Contact)]])
}

async fn handle_start(
    bot: AutoSend<Bot>,
    msg: Message,
    app_state: std::sync::Arc<AppState>,
    dialogue: MyDialogue,
) -> anyhow::Result<()> {
    log::info!("start: {:#?}", msg);
    if !msg.chat.is_private() {
        return Ok(());
    }
    match msg.contact() {
        Some(contact) => {
            if contact.user_id.map(i64::from) != Some(msg.chat.id) {
                bot.send_message(msg.chat.id, "Відправте свій контакт.")
                    .reply_markup(request_phone_number_confirmation_keyboard())
                    .await?;
                return Ok(());
            }
            if !app_state
                .redis_connection
                .lock()
                .await
                .exists(format!(
                    "USER:{phone_number}",
                    phone_number = contact.phone_number
                ))
                .await?
            {
                bot.send_message(msg.chat.id, "Нажаль вашого номеру телефона ще нема в списку дозволених. Зверніться до адміністратора, та відправте свій контакт знов.").reply_markup(request_phone_number_confirmation_keyboard()).await?;
                return Ok(());
            }
            dialogue
                .update(State::AwaitingRequests {
                    contact: contact.clone(),
                })
                .await?;
            bot.send_message(
                msg.chat.id,
                format!("Ваш номер {} підтверджено. Просто надсилайте текстове повідомлення з номерними знаками і бот відповість чи є такий запис в базі.", contact.phone_number),
            ).reply_markup(teloxide::types::KeyboardRemove::new())
            .await?;
        }
        _ => {
            bot.send_message(
                msg.chat.id,
                "Натисніть \"Підтвердити мій номер телефону\" щоб продовжити.",
            )
            .reply_markup(request_phone_number_confirmation_keyboard())
            .await?;
        }
    }

    Ok(())
}

async fn handle_awaiting_requests(
    bot: AutoSend<Bot>,
    msg: Message,
    app_state: std::sync::Arc<AppState>,
    (contact,): (teloxide::types::Contact,),
) -> anyhow::Result<()> {
    let msg_text = if let Some(msg_text) = msg.text() {
        msg_text
    } else {
        bot.send_message(
            msg.chat.id,
            "Просто надсилайте текстове повідомлення з номерними знаками і бот відповість чи є такий запис в базі."
        ).await?;
        return Ok(());
    };

    if msg_text.starts_with("/") {
        if app_state
            .redis_connection
            .lock()
            .await
            .exists(format!(
                "ADMIN:{phone_number}",
                phone_number = contact.phone_number
            ))
            .await?
        {
            if msg_text.starts_with("/adduser ") {
                let phone_number = &msg_text["/adduser ".len()..].trim().replace(|ch: char| !ch.is_ascii_digit(), "");
                app_state
                    .redis_connection
                    .lock()
                    .await
                    .set(
                        format!("USER:{phone_number}"),
                        ""
                    )
                    .await?;
                bot.send_message(
                        msg.chat.id,
                        format!("Користувача з номером телефону {phone_number} додано.")
                    ).await?;
            }

            if msg_text.starts_with("/deluser ") {
                let phone_number = &msg_text["/deluser ".len()..].trim().replace(|ch: char| !ch.is_ascii_digit(), "");
                app_state
                    .redis_connection
                    .lock()
                    .await
                    .del(format!("USER:{phone_number}"))
                    .await?;
                bot.send_message(
                        msg.chat.id,
                        format!("Користувача з номером телефону {phone_number} видалено.")
                    ).await?;
            }

            if msg_text.starts_with("/addadmin ") {
                let phone_number = &msg_text["/addadmin ".len()..].trim().replace(|ch: char| !ch.is_ascii_digit(), "");
                app_state
                    .redis_connection
                    .lock()
                    .await
                    .set(
                        format!("ADMIN:{phone_number}"),
                        ""
                    )
                    .await?;
                bot.send_message(
                        msg.chat.id,
                        format!("Адміна з номером телефону {phone_number} додано.")
                    ).await?;
            }

            if msg_text.starts_with("/deladmin ") {
                let phone_number = &msg_text["/deladmin ".len()..].trim().replace(|ch: char| !ch.is_ascii_digit(), "");
                app_state
                    .redis_connection
                    .lock()
                    .await
                    .del(format!("ADMIN:{phone_number}"))
                    .await?;
                bot.send_message(
                        msg.chat.id,
                        format!("Адміна з номером телефону {phone_number} видалено.")
                    ).await?;
            }

            return Ok(());
        }
    }

    if msg_text.len() < 20 {
        let car_license_plate = normalize_license_plate(msg_text);
        log::info!("Searching for \"{car_license_plate}\" (raw: \"{msg_text}\")");
        // app_state.redis_connection.lock().await.set::<_, _, _>(format!("CAR:{license_plate}", license_plate=normalize_license_plate("ВТ5527СМ")), serde_json::to_vec(&CarInfo { reported_in_city: Some("Херсон".to_owned()), car_brand: Some("Renault Trafic".to_owned()), car_color: Some("Белый".to_owned()), comment: Some("два з них символікою ССО (судової охорони) у них є тепер форма Служби Судової Охорони".to_owned()), number_of_people: None }).unwrap()).await?;
        // app_state.redis_connection.lock().await.set::<_, _, _>("partial-license-plates-car-info", serde_json::to_vec(&vec![PartialLicensePlatesCarInfo { partial_license_plate: "2333".to_owned(), car_info: CarInfo { reported_in_city: None, car_brand: Some("Нива".to_owned()), car_color: Some("Темносиня".to_owned()), comment: None, number_of_people: None }}]).unwrap()).await?;
        let car_info: Option<CarInfo> = {
            let raw_car_info = app_state
                .redis_connection
                .lock()
                .await
                .get::<_, Vec<u8>>(format!("CAR:{car_license_plate}"))
                .await?;
            if raw_car_info.is_empty() {
                None
            } else {
                Some(serde_json::from_slice(&raw_car_info)?)
            }
        };
        if let Some(car_info) = car_info {
            bot.send_message(
                msg.chat.id,
                format!("Є точний збіг!\n\nНомерний знак: {car_license_plate}\n{car_info}"),
            )
            .await?;
        } else {
            /*
            let partial_license_plates_car_info: Vec<PartialLicensePlatesCarInfo> = {
                let raw_partial_license_plates_car_info = app_state
                    .redis_connection
                    .lock()
                    .await
                    .get::<_, Vec<u8>>("partial-license-plates-car-info")
                    .await?;
                if raw_partial_license_plates_car_info.is_empty() {
                    vec![]
                } else {
                    serde_json::from_slice::<Vec<PartialLicensePlatesCarInfo>>(
                        &raw_partial_license_plates_car_info,
                    )?
                    .into_iter()
                    .filter(|partial_license_plates_car_info| {
                        partial_license_plates_car_info.matches(&car_license_plate)
                    })
                    .collect()
                }
            };
            if !partial_license_plates_car_info.is_empty() {
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Нема точного збігу за \"{car_license_plate}\", але знайдено {number_of_partial_match_records} запис(ів), які можуть збігатися:",
                        number_of_partial_match_records=partial_license_plates_car_info.len(),
                    ),
                )
                .await?;
                for partial_license_plate_car_info in partial_license_plates_car_info {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "Частково відомий номерний знак: {partial_license_plate}\n{car_info}",
                            partial_license_plate=partial_license_plate_car_info.partial_license_plate,
                            car_info=partial_license_plate_car_info.car_info,
                        ),
                    )
                    .await?;
                }
            } else {
            */

            bot.send_message(
                msg.chat.id,
                format!(
                    "Інформації за номерними знаками \"{}\" не знайдено. Просто надсилайте текстове повідомлення з номерними знаками і бот відповість чи є такий запис в базі.",
                    car_license_plate
                ),
            )
            .await?;
        }
    } else {
        if !app_state
            .redis_connection
            .lock()
            .await
            .exists(format!(
                "ADMIN:{phone_number}",
                phone_number = contact.phone_number
            ))
            .await?
        {
            bot.send_message(
            msg.chat.id,
            "Просто надсилайте текстове повідомлення з номерними знаками і бот відповість чи є такий запис в базі."
            ).await?;
        } else {
            let re = regex::Regex::new(
                r"Номерний знак: (?P<car_license_plate>[^\n]+)\s+Авто: (?P<car_brand>[^\n]+)\s+Колір авто: (?P<car_color>[^\n]+)\s+Особливості: (?P<comment>[^\n]+)\s+Чисельність ДРГ: (?P<number_of_people>\d+|\?)\s+Місто де вперше було зафіксовано: (?P<reported_in_city>[^\n]+)",
            )
            .unwrap();
            if let Some(car_info_match) = re.captures(&msg_text) {
                let car_license_plate =
                    normalize_license_plate(&car_info_match["car_license_plate"]);
                let car_brand = car_info_match["car_brand"].to_owned();
                let car_color = car_info_match["car_color"].to_owned();
                let comment = car_info_match["comment"].to_owned();
                let number_of_people = &car_info_match["number_of_people"];
                let reported_in_city = car_info_match["reported_in_city"].to_owned();

                let car_info = CarInfo {
                    car_brand: if car_brand == "?" {
                        None
                    } else {
                        Some(car_brand)
                    },
                    car_color: if car_color == "?" {
                        None
                    } else {
                        Some(car_color)
                    },
                    comment: if comment == "?" { None } else { Some(comment) },
                    number_of_people: if number_of_people == "?" {
                        None
                    } else {
                        Some(number_of_people.parse()?)
                    },
                    reported_in_city: if reported_in_city == "?" {
                        None
                    } else {
                        Some(reported_in_city)
                    },
                };

                app_state
                    .redis_connection
                    .lock()
                    .await
                    .set::<_, _, _>(
                        format!("CAR:{car_license_plate}"),
                        serde_json::to_vec(&car_info).unwrap(),
                    )
                    .await?;

                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Інформацію про авто з номерними знаками \"{car_license_plate}\" додано"
                    ),
                )
                .reply_to_message_id(msg.id)
                .await?;
            } else {
                bot.send_message(
                    msg.chat.id,
                    "Не вдалось розібрати запит на додавання інформації про авто. Перевірте форму (зайві пусті строки тощо не можуть бути оброблені автоматично)"
                )
                .await?;
            }
        }
    }

    Ok(())
}
