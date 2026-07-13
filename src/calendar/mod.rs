use std::{error::Error, fs, path::PathBuf};

use calcard::{
    Parser,
    icalendar::{ICalendarComponent, ICalendarComponentType, ICalendarProperty, ICalendarValue},
};
use chrono::{Datelike, NaiveDateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[allow(unused)]
#[derive(Default, Debug)]
pub(crate) struct Calendar {
    id: Uuid,
    uid: String,
    url: String,
    label: String,
    timezone: String, //todo: is there a chrono_tz value possible?
    publish_ttl: Option<String>,
}

#[allow(unused)]
#[derive(Default, Debug)]
pub(crate) struct CalendarItem {
    uid: String,
    label: String,
    description: String,
    // summary: String,
    start_at: NaiveDateTime,
    end_at: Option<NaiveDateTime>,
    created_at: Option<NaiveDateTime>,
    last_modified: Option<NaiveDateTime>,
}

#[derive(Deserialize, Debug)]
pub(crate) enum CalendarType {
    ICS,
    Gmail,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ConfigProfile {
    name: String,
    #[serde(default)]
    calendar: Vec<ConfigCalendar>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ConfigCalendar {
    label: String,
    account: String,
    #[serde(rename(deserialize = "type"))]
    cal_type: CalendarType,
    url: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Config {
    profile: Vec<ConfigProfile>,
}

pub(crate) fn sync(
    profile_name: Option<&str>,
    calendar_name: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let config = read_config()?;
    let current_date = Utc::now();
    let previous_year = (current_date.year() - 1) as u32;

    let mut synced_calendars = 0usize;
    let mut synced_items = 0usize;

    for profile in config
        .profile
        .iter()
        .filter(|profile| profile_name.map_or(true, |wanted| profile.name == wanted))
    {
        for calendar_cfg in profile
            .calendar
            .iter()
            .filter(|calendar| calendar_name.map_or(true, |wanted| calendar.label == wanted))
        {
            let remote_calendar_id = remote_calendar_id(&profile.name, &calendar_cfg.url);
            let calendar = Calendar {
                id: remote_calendar_id,
                uid: remote_calendar_id.to_string(),
                url: calendar_cfg.url.clone(),
                label: calendar_cfg.label.clone(),
                timezone: String::new(),
                publish_ttl: None,
            };

            let items = sync_calendar(&calendar, previous_year)?;
            synced_calendars += 1;
            synced_items += items.len();

            println!(
                "synced profile={} calendar={} items={}",
                profile.name,
                calendar.label,
                items.len()
            );
        }
    }

    println!("done calendars={} items={}", synced_calendars, synced_items);
    Ok(())
}

fn read_config() -> Result<Config, Box<dyn Error>> {
    ensure_config_file()?;
    let contents = fs::read_to_string(config_path())?;
    Ok(toml::from_str(&contents)?)
}

fn ensure_config_file() -> Result<(), Box<dyn Error>> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    if path.exists() {
        return Ok(());
    }

    fs::write(path, DEFAULT_CONFIG)?;
    println!("created config.toml");
    Ok(())
}

const DEFAULT_CONFIG: &str = r#"# Rusty Calendar Pi
# Profiles own the color.
# Calendars live under each profile and point at sync URLs.

profile = []
"#;

pub(crate) fn db_path() -> PathBuf {
    let mut path = data_dir();
    path.push("calendar.duckdb");
    path
}

fn config_path() -> PathBuf {
    let mut path = config_dir();
    path.push("config.toml");
    path
}

fn config_dir() -> PathBuf {
    let mut path = home_dir();
    path.push(".config/rusty-calendar-pi");
    path
}

fn data_dir() -> PathBuf {
    let mut path = home_dir();
    path.push(".local/share/rusty-calendar-pi");
    path
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn sync_calendar(calendar: &Calendar, previous_year: u32) -> Result<Vec<CalendarItem>, Box<dyn Error>> {
    let input = download_ical(&calendar.url);
    let mut parser = Parser::new(&input);
    let mut items = Vec::new();

    loop {
        match parser.entry() {
            calcard::Entry::ICalendar(ical) => {
                for component in ical.components {
                    if !matches!(component.component_type, ICalendarComponentType::VEvent) {
                        continue;
                    }

                    let Some(item) = parse_item(&component) else {
                        continue;
                    };

                    let (_, year) = item.start_at.year_ce();
                    if year < previous_year {
                        continue;
                    }

                    items.push(item);
                }
            }
            calcard::Entry::VCard(_) => continue,
            calcard::Entry::InvalidLine(_) => continue,
            calcard::Entry::UnexpectedComponentEnd { .. } => continue,
            calcard::Entry::UnterminatedComponent(_) => continue,
            calcard::Entry::TooManyComponents => continue,
            calcard::Entry::Eof => break,
            _ => continue,
        }
    }

    Ok(items)
}

fn parse_item(item: &ICalendarComponent) -> Option<CalendarItem> {
    let start_at = get_datetime_by_name(item, ICalendarProperty::Dtstart)?;

    Some(CalendarItem {
        uid: get_property_value_by_name(item, ICalendarProperty::Uid),
        label: get_property_value_by_name(item, ICalendarProperty::Summary),
        description: get_property_value_by_name(item, ICalendarProperty::Description),
        start_at,
        end_at: get_datetime_by_name(item, ICalendarProperty::Dtend),
        created_at: get_datetime_by_name(item, ICalendarProperty::Created),
        last_modified: get_datetime_by_name(item, ICalendarProperty::LastModified),
    })
}

fn remote_calendar_id(profile_name: &str, url: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("{profile_name}:{url}").as_bytes())
}

// fn main() {
//     let config_contents = fs::read_to_string("config.toml").expect("Config.toml file not found");
//     let config = toml::from_str::<Config>(&config_contents);
//
//     println!("Config: {:#?}\n", config);
//
//     return;
//
//     let urls = vec![
//         "https://www.brandweerrooster.nl/api/users/397643/calendar/availability_calendar.ics?locale=nl&standby_duty_only=true&hmac=643c529ab1b68e324c74fe25b8eab344ec3d836f",
//         // "https://calendar.google.com/calendar/ical/yoramdelangen%40gmail.com/private-8b908669de3f1313974d35a765bb88fc/basic.ics",
//     ];
//
//     let db = duckdb::Connection::open_in_memory().unwrap();
//     let res = db.execute(
//         "CREATE TABLE IF NOT EXISTS calendars (
// 		id VARCHAR PRIMARY KEY NOT NULL,
// 		url VARCHAR NOT NULL,
// 		publish_ttl_minutes INTEGER,
// 		type VARCHAR,
// 		timezone VARCHAR,
// 		format VARCHAR
// 	)",
//         params![],
//     );
//
//     println!("Result: {:#?}", res);
//
//     let mut c = 0;
//
//     let current_date = chrono::Utc::now();
//     let previous_year = (current_date.year() - 1) as u32;
//
//     for url in urls {
//         println!("Downloading URL: {}", url);
//
//         let u = url.parse::<Uri>().unwrap();
//
//         let p = format!(
//             "{}://{}{}",
//             u.scheme().unwrap(),
//             u.host().unwrap(),
//             u.path()
//         );
//
//         let uid = id_from_str(p.as_str());
//         println!("UUID: {}", uid);
//
//         break;
//         let input = download_ical(url);
//         let mut parser = Parser::new(&input);
//
//         loop {
//             match parser.entry() {
//                 calcard::Entry::VCard(vcard) => {
//                     println!("VCard: {:#?}", vcard);
//                 }
//                 calcard::Entry::ICalendar(ical) => {
//                     for item in ical.components {
//                         match item.component_type {
//                             // ICalendarComponentType::Other(_) => todo!(),
//                             ICalendarComponentType::VCalendar => {
//                                 // lookup by URL?
//                                 let nc = Calendar {
//                                     uid: Uuid::new_v4().to_string(),
//                                     url: url.to_owned(),
//                                     label: get_property_value_by_name(
//                                         &item,
//                                         ICalendarProperty::Other("X-WR-CALNAME".to_owned()),
//                                     ),
//                                     timezone: get_property_value_by_name(
//                                         &item,
//                                         ICalendarProperty::Other("X-WR-TIMEZONE".to_owned()),
//                                     ),
//                                     publish_ttl: Some(get_property_value_by_name(
//                                         &item,
//                                         ICalendarProperty::Other("X-PUBLISHED-TTL".to_owned()),
//                                     )),
//                                 };
//
//                                 // Contains:
//                                 // - Calander name
//                                 // - Publish TTL (updated every X: PT30M) 30minutes
//                                 // - Timezone (Europe/Amsterdam)
//                                 println!(
//                                     "Calendar info: {:?}, item: {:#?}, index: {}",
//                                     &item, nc, c
//                                 );
//
//                                 c += 1;
//                             }
//                             ICalendarComponentType::VEvent => {
//                                 let new_item = CalendarItem {
//                                     uid: get_property_value_by_name(&item, ICalendarProperty::Uid),
//                                     label: get_property_value_by_name(
//                                         &item,
//                                         ICalendarProperty::Summary,
//                                     ),
//                                     description: get_property_value_by_name(
//                                         &item,
//                                         ICalendarProperty::Description,
//                                     ),
//                                     // summary: todo!(),
//                                     start_at: get_datetime_by_name(
//                                         &item,
//                                         ICalendarProperty::Dtstart,
//                                     )
//                                     .unwrap(),
//                                     end_at: get_datetime_by_name(&item, ICalendarProperty::Dtend),
//                                     created_at: get_datetime_by_name(
//                                         &item,
//                                         ICalendarProperty::Created,
//                                     ),
//                                     last_modified: get_datetime_by_name(
//                                         &item,
//                                         ICalendarProperty::LastModified,
//                                     ),
//                                 };
//
//                                 let (_, y) = new_item.start_at.year_ce();
//                                 if y < previous_year {
//                                     continue;
//                                 }
//
//                                 c += 1;
//                                 // dbg!(new_item);
//                             }
//                             // ICalendarComponentType::VTodo => todo!(),
//                             // ICalendarComponentType::VJournal => todo!(),
//                             // ICalendarComponentType::VFreebusy => todo!(),
//                             ICalendarComponentType::VTimezone => {
//                                 // println!("Timezone from the calendar: {:?}", &item);
//                             }
//                             ICalendarComponentType::VAlarm => {
//                                 // what is this? dunno
//                                 continue;
//                             }
//                             ICalendarComponentType::Standard => continue,
//                             ICalendarComponentType::Daylight => continue,
//                             // ICalendarComponentType::VAvailability => todo!(),
//                             // ICalendarComponentType::Available => todo!(),
//                             // ICalendarComponentType::Participant => todo!(),
//                             // ICalendarComponentType::VLocation => todo!(),
//                             // ICalendarComponentType::VResource => todo!(),
//                             // ICalendarComponentType::VStatus => todo!(),
//                             _ => {
//                                 println!("Dont know this item: {:#?}", &item);
//                             }
//                         }
//                         // println!("ICal Component: {:#?}", &item);
//                     }
//                 }
//                 calcard::Entry::InvalidLine(_) => todo!(),
//                 calcard::Entry::UnexpectedComponentEnd { expected, found } => todo!(),
//                 calcard::Entry::UnterminatedComponent(cow) => todo!(),
//                 calcard::Entry::TooManyComponents => todo!(),
//                 calcard::Entry::Eof => break,
//                 _ => todo!(),
//             }
//         }
//     }
//
//     println!("Hello, world! Total count: {}", c);
// }

fn download_ical(url: &str) -> String {
    ureq::get(url)
        .call()
        .unwrap()
        .body_mut()
        .read_to_string()
        .unwrap()
}

fn id_from_str(url: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, url.as_bytes())
}

fn get_property_value_by_name(item: &ICalendarComponent, name: ICalendarProperty) -> String {
    let en = item.entries.iter().find(|e| e.name == name);
    match en {
        Some(entry) => match entry.values.first().unwrap() {
            // ICalendarValue::Binary(items) => todo!(),
            // ICalendarValue::Boolean(_) => todo!(),
            // ICalendarValue::Uri(uri) => todo!(),
            ICalendarValue::PartialDateTime(dt) => match dt.to_rfc3339() {
                Some(x) => x,
                None => {
                    let native = dt.to_date_time().unwrap().date_time;
                    if !dt.has_time() {
                        native.to_string()
                    } else {
                        native.date().to_string()
                    }
                }
            },
            // ICalendarValue::Duration(icalendar_duration) => todo!(),
            // ICalendarValue::RecurrenceRule(icalendar_recurrence_rule) => todo!(),
            // ICalendarValue::Period(icalendar_period) => todo!(),
            // ICalendarValue::Float(_) => todo!(),
            // ICalendarValue::Integer(_) => todo!(),
            // ICalendarValue::Text(_) => todo!(),
            // ICalendarValue::CalendarScale(calendar_scale) => todo!(),
            // ICalendarValue::Method(icalendar_method) => todo!(),
            // ICalendarValue::Classification(icalendar_classification) => todo!(),
            // ICalendarValue::Status(icalendar_status) => todo!(),
            // ICalendarValue::Transparency(icalendar_transparency) => todo!(),
            // ICalendarValue::Action(icalendar_action) => todo!(),
            // ICalendarValue::BusyType(icalendar_free_busy_type) => todo!(),
            // ICalendarValue::ParticipantType(icalendar_participant_type) => todo!(),
            // ICalendarValue::ResourceType(icalendar_resource_type) => todo!(),
            // ICalendarValue::Proximity(icalendar_proximity_value) => todo!(),
            val => val.clone().as_text().unwrap_or("").to_owned(),
        },

        None => match name {
            ICalendarProperty::Description | ICalendarProperty::Summary => "".to_owned(),
            _ => {
                if name == ICalendarProperty::Other("X-WR-TIMEZONE".to_owned()) {
                    return "".to_owned();
                }

                dbg!(item);
                todo!("Entry item \"{:?}\" by name not found!", name)
            }
        },
    }
}

fn get_datetime_by_name(
    item: &ICalendarComponent,
    name: ICalendarProperty,
) -> Option<NaiveDateTime> {
    let en = item.entries.iter().find(|e| e.name == name);
    match en {
        Some(entry) => match entry.values.first().unwrap() {
            ICalendarValue::PartialDateTime(dt) => Some(dt.to_date_time().unwrap().date_time),
            // ICalendarValue::Duration(icalendar_duration) => todo!(),
            // ICalendarValue::RecurrenceRule(icalendar_recurrence_rule) => todo!(),
            // ICalendarValue::Period(icalendar_period) => todo!(),
            // ICalendarValue::Float(_) => todo!(),
            // ICalendarValue::Integer(_) => todo!(),
            // ICalendarValue::Text(_) => todo!(),
            _ => {
                todo!("Entry item not DateTime value: {:?}", name)
            }
        },

        None => match name {
            _ => {
                dbg!(item);
                None
            }
        },
    }
}
