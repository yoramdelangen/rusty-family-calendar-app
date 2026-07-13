use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use calcard::{
    Parser,
    icalendar::{ICalendarComponent, ICalendarComponentType, ICalendarProperty, ICalendarValue},
};
use chrono::{Datelike, NaiveDateTime, Utc};
use argh::FromArgs;
use serde::{Deserialize, Serialize};
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

#[derive(FromArgs)]
#[argh(subcommand, name = "calendar")]
/// Manage calendars
pub(crate) struct CalendarArgs {
    #[argh(subcommand)]
    pub(crate) command: Option<CalendarCommand>,
}

#[derive(FromArgs)]
#[argh(subcommand)]
pub(crate) enum CalendarCommand {
    Add(CalendarAddArgs),
}

#[derive(FromArgs)]
#[argh(subcommand, name = "add")]
/// Add a calendar
pub(crate) struct CalendarAddArgs {}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) enum CalendarType {
    ICS,
    Gmail,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct ConfigProfile {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) calendar: Vec<ConfigCalendar>,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct ConfigCalendar {
    pub(crate) label: String,
    pub(crate) account: String,
    #[serde(rename = "type")]
    pub(crate) cal_type: CalendarType,
    pub(crate) url: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Config {
    pub(crate) profile: Vec<ConfigProfile>,
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

pub(crate) fn list_calendars() -> Result<(), Box<dyn Error>> {
    let config = read_config()?;

    let rows = config
        .profile
        .iter()
        .flat_map(|profile| {
            profile.calendar.iter().map(move |calendar| {
                vec![
                    profile.name.clone(),
                    calendar.label.clone(),
                    format!("{:?}", calendar.cal_type),
                    calendar.account.clone(),
                ]
            })
        })
        .collect::<Vec<_>>();

    if rows.is_empty() {
        println!("no calendars configured");
        return Ok(());
    }

    crate::table::print(&["Profile", "Label", "Type", "Account"], &rows);

    Ok(())
}

pub(crate) fn calendar_add() -> Result<(), Box<dyn Error>> {
    let mut config = read_config()?;
    if config.profile.is_empty() {
        return Err(other_error("add a profile first"));
    }

    let profile_index = prompt_profile_index(&config.profile)?;
    let profile_name = config.profile[profile_index].name.clone();
    let label = prompt_unique_calendar_label(&config.profile[profile_index])?;
    let account = prompt_required("Account")?;
    let cal_type = prompt_calendar_type()?;
    let url = prompt_calendar_url()?;

    config.profile[profile_index].calendar.push(ConfigCalendar {
        label: label.clone(),
        account,
        cal_type,
        url: url.clone(),
    });
    save_config(&config)?;

    println!("added profile={profile_name} calendar={label}");
    Ok(())
}

pub(crate) fn read_config() -> Result<Config, Box<dyn Error>> {
    ensure_config_file()?;
    let contents = fs::read_to_string(config_path())?;
    Ok(toml::from_str(&contents)?)
}

pub(crate) fn save_config(config: &Config) -> Result<(), Box<dyn Error>> {
    ensure_config_file()?;
    fs::write(config_path(), toml::to_string_pretty(config)?)?;
    Ok(())
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
    storage_dir().join("config.toml")
}

fn data_dir() -> PathBuf {
    storage_dir()
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn storage_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    } else {
        let mut path = home_dir();
        path.push(".config/rusty-calendar-pi");
        path
    }
}

fn prompt_profile_index(profiles: &[ConfigProfile]) -> Result<usize, Box<dyn Error>> {
    println!("Choose a profile:");
    for (index, profile) in profiles.iter().enumerate() {
        println!("{}: {}", index + 1, profile.name);
    }

    loop {
        let value = prompt_required("Profile number")?;
        let Ok(choice) = value.parse::<usize>() else {
            println!("enter a number");
            continue;
        };

        if choice == 0 || choice > profiles.len() {
            println!("pick a number from 1 to {}", profiles.len());
            continue;
        }

        return Ok(choice - 1);
    }
}

fn prompt_unique_calendar_label(profile: &ConfigProfile) -> Result<String, Box<dyn Error>> {
    loop {
        let label = prompt_required("Calendar label")?;
        if profile.calendar.iter().any(|calendar| calendar.label == label) {
            println!("calendar already exists in this profile");
            continue;
        }

        return Ok(label);
    }
}

fn prompt_required(label: &str) -> Result<String, Box<dyn Error>> {
    loop {
        let value = prompt(label)?;
        if !value.is_empty() {
            return Ok(value);
        }

        println!("value is required");
    }
}

fn prompt_calendar_type() -> Result<CalendarType, Box<dyn Error>> {
    println!("Calendar type:");
    println!("1: ICS");
    println!("2: Gmail");

    loop {
        let value = prompt_required("Choice")?;
        if let Some(cal_type) = parse_calendar_type_choice(&value) {
            return Ok(cal_type);
        }

        println!("pick 1 or 2");
    }
}

fn prompt_calendar_url() -> Result<String, Box<dyn Error>> {
    loop {
        let value = prompt_required("Calendar URL")?;
        if is_valid_calendar_url(&value) {
            return Ok(value);
        }

        println!("url must start with http:// or https://");
    }
}

fn prompt(label: &str) -> Result<String, Box<dyn Error>> {
    print!("{}: ", label);
    io::stdout().flush()?;

    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 {
        return Err(other_error("unexpected end of input"));
    }
    Ok(input.trim().to_owned())
}

fn parse_calendar_type_choice(value: &str) -> Option<CalendarType> {
    match value.trim() {
        "1" => Some(CalendarType::ICS),
        "2" => Some(CalendarType::Gmail),
        _ => None,
    }
}

fn is_valid_calendar_url(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("http://") || value.starts_with("https://")
}

fn other_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::other(message.into()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calendar_type_choice() {
        assert!(matches!(parse_calendar_type_choice("1"), Some(CalendarType::ICS)));
        assert!(matches!(parse_calendar_type_choice("2"), Some(CalendarType::Gmail)));
        assert!(parse_calendar_type_choice("nope").is_none());
    }

    #[test]
    fn validates_calendar_url_prefix() {
        assert!(is_valid_calendar_url("https://example.com/feed.ics"));
        assert!(!is_valid_calendar_url("ftp://example.com/feed.ics"));
    }
}
