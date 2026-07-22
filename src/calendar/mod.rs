use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use argh::FromArgs;
use calcard::{
    Parser,
    icalendar::{ICalendarComponent, ICalendarComponentType, ICalendarProperty, ICalendarValue},
};
use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use rusqlite::{Connection, params};
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
#[derive(Clone, Default, Debug)]
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

const MIN_SYNC_INTERVAL_SECONDS: i64 = 5 * 60;

#[derive(Clone, Debug)]
pub(crate) enum CalendarChange {
    Created { title: String },
    Updated { title: String },
    Removed { title: String },
}

#[derive(Clone, Debug)]
pub(crate) enum SyncStatus {
    Syncing {
        calendar: String,
    },
    Synced {
        synced_at: DateTime<Utc>,
        next_sync_at: DateTime<Utc>,
        changes: Vec<CalendarChange>,
    },
    Failed {
        calendar: String,
        error: String,
        next_sync_at: DateTime<Utc>,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) enum CalendarType {
    ICS,
    Gmail,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ConfigProfile {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) color: Option<String>,
    #[serde(default)]
    pub(crate) calendar: Vec<ConfigCalendar>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ConfigCalendar {
    pub(crate) label: String,
    pub(crate) account: String,
    #[serde(rename = "type")]
    pub(crate) cal_type: CalendarType,
    pub(crate) url: String,
    #[serde(default)]
    pub(crate) color: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
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

    for (profile, calendar_cfg) in selected_calendars(&config, profile_name, calendar_name) {
        let profile_id = remote_profile_id(&profile.name);
        let remote_calendar_id = remote_calendar_id(&profile.name, &calendar_cfg.url);
        let calendar = Calendar {
            id: remote_calendar_id,
            uid: remote_calendar_id.to_string(),
            url: calendar_cfg.url.clone(),
            label: calendar_cfg.label.clone(),
            timezone: String::new(),
            publish_ttl: None,
        };

        let result = sync_calendar(&calendar, previous_year)?;
        store_sync_items(
            &profile_id,
            &calendar,
            &result.items,
            Utc::now().naive_utc(),
            result.published_ttl_seconds,
        )?;
        synced_calendars += 1;
        synced_items += result.items.len();

        println!(
            "synced profile={} calendar={} items={}",
            profile.name,
            calendar.label,
            result.items.len()
        );
    }

    println!("done calendars={} items={}", synced_calendars, synced_items);
    Ok(())
}

pub(crate) fn start_sync_worker() -> Receiver<SyncStatus> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        if let Err(err) = sync_worker(tx.clone()) {
            let next_sync_at = Utc::now() + chrono::Duration::seconds(MIN_SYNC_INTERVAL_SECONDS);
            let _ = tx.send(SyncStatus::Failed {
                calendar: "sync".to_owned(),
                error: err.to_string(),
                next_sync_at,
            });
        }
    });

    rx
}

fn sync_worker(tx: Sender<SyncStatus>) -> Result<(), Box<dyn Error>> {
    let config = read_config()?;
    let mut calendars = selected_calendars(&config, None, None)
        .into_iter()
        .map(|(profile, calendar)| WorkerCalendar {
            profile_name: profile.name.clone(),
            calendar: calendar.clone(),
            next_sync_at: Utc::now(),
        })
        .collect::<Vec<_>>();

    if calendars.is_empty() {
        return Ok(());
    }

    loop {
        let now = Utc::now();

        for index in 0..calendars.len() {
            if calendars[index].next_sync_at > now {
                continue;
            }

            let _ = tx.send(SyncStatus::Syncing {
                calendar: calendars[index].calendar.label.clone(),
            });

            let result =
                sync_one_calendar(&calendars[index].profile_name, &calendars[index].calendar);
            let status = match result {
                Ok(outcome) => {
                    calendars[index].next_sync_at = outcome.next_sync_at;
                    SyncStatus::Synced {
                        synced_at: outcome.synced_at,
                        next_sync_at: next_worker_sync_at(&calendars),
                        changes: outcome.changes,
                    }
                }
                Err(err) => {
                    calendars[index].next_sync_at =
                        Utc::now() + chrono::Duration::seconds(MIN_SYNC_INTERVAL_SECONDS);
                    SyncStatus::Failed {
                        calendar: calendars[index].calendar.label.clone(),
                        error: err.to_string(),
                        next_sync_at: next_worker_sync_at(&calendars),
                    }
                }
            };
            let _ = tx.send(status);
        }

        thread::sleep(Duration::from_secs(1));
    }
}

struct WorkerCalendar {
    profile_name: String,
    calendar: ConfigCalendar,
    next_sync_at: DateTime<Utc>,
}

struct SyncOutcome {
    synced_at: DateTime<Utc>,
    next_sync_at: DateTime<Utc>,
    changes: Vec<CalendarChange>,
}

fn next_worker_sync_at(calendars: &[WorkerCalendar]) -> DateTime<Utc> {
    calendars
        .iter()
        .map(|calendar| calendar.next_sync_at)
        .min()
        .unwrap_or_else(Utc::now)
}

fn sync_one_calendar(
    profile_name: &str,
    calendar_cfg: &ConfigCalendar,
) -> Result<SyncOutcome, Box<dyn Error>> {
    let current_date = Utc::now();
    let previous_year = (current_date.year() - 1) as u32;
    let profile_id = remote_profile_id(profile_name);
    let remote_calendar_id = remote_calendar_id(profile_name, &calendar_cfg.url);
    let calendar = Calendar {
        id: remote_calendar_id,
        uid: remote_calendar_id.to_string(),
        url: calendar_cfg.url.clone(),
        label: calendar_cfg.label.clone(),
        timezone: String::new(),
        publish_ttl: None,
    };

    let result = sync_calendar(&calendar, previous_year)?;
    let synced_at = Utc::now();
    let changes = store_sync_items(
        &profile_id,
        &calendar,
        &result.items,
        synced_at.naive_utc(),
        result.published_ttl_seconds,
    )?;
    let ttl = result
        .published_ttl_seconds
        .unwrap_or(MIN_SYNC_INTERVAL_SECONDS)
        .max(MIN_SYNC_INTERVAL_SECONDS);

    Ok(SyncOutcome {
        synced_at,
        next_sync_at: synced_at + chrono::Duration::seconds(ttl),
        changes,
    })
}

fn selected_calendars<'a>(
    config: &'a Config,
    profile_name: Option<&str>,
    calendar_name: Option<&str>,
) -> Vec<(&'a ConfigProfile, &'a ConfigCalendar)> {
    let mut selected = Vec::new();

    for profile in &config.profile {
        if profile_name.is_some_and(|wanted| profile.name != wanted) {
            continue;
        }

        for calendar in &profile.calendar {
            if calendar_name.is_some_and(|wanted| calendar.label != wanted) {
                continue;
            }

            selected.push((profile, calendar));
        }
    }

    selected
}

fn store_sync_items(
    profile_id: &Uuid,
    calendar: &Calendar,
    items: &[CalendarItem],
    synced_at: NaiveDateTime,
    published_ttl_seconds: Option<i64>,
) -> Result<Vec<CalendarChange>, Box<dyn Error>> {
    let mut conn = Connection::open(db_path())?;
    persist_sync_items_with_ttl(
        &mut conn,
        profile_id,
        calendar,
        items,
        synced_at,
        published_ttl_seconds,
    )
}

#[cfg(test)]
fn persist_sync_items(
    conn: &mut Connection,
    profile_id: &Uuid,
    calendar: &Calendar,
    items: &[CalendarItem],
    synced_at: NaiveDateTime,
) -> Result<(), Box<dyn Error>> {
    persist_sync_items_with_ttl(conn, profile_id, calendar, items, synced_at, None).map(|_| ())
}

fn persist_sync_items_with_ttl(
    conn: &mut Connection,
    profile_id: &Uuid,
    calendar: &Calendar,
    items: &[CalendarItem],
    synced_at: NaiveDateTime,
    published_ttl_seconds: Option<i64>,
) -> Result<Vec<CalendarChange>, Box<dyn Error>> {
    ensure_sync_schema(conn)?;
    let old_items = load_stored_items(conn, &calendar.id)?;
    let changes = calendar_changes(&old_items, items);

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO sync_calendars (calendar_id, profile_id, last_synced_at, published_ttl_seconds)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(calendar_id) DO UPDATE SET
             profile_id = excluded.profile_id,
             last_synced_at = excluded.last_synced_at,
             published_ttl_seconds = excluded.published_ttl_seconds",
        params![
            calendar.id.to_string(),
            profile_id.to_string(),
            format_naive_datetime(&synced_at),
            published_ttl_seconds,
        ],
    )?;
    tx.execute(
        "DELETE FROM sync_items WHERE calendar_id = ?1",
        params![calendar.id.to_string()],
    )?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO sync_items (
                calendar_id,
                item_uid,
                item_label,
                description,
                start_at,
                end_at,
                created_at,
                last_modified
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;

        for item in items {
            stmt.execute(params![
                calendar.id.to_string(),
                item.uid.as_str(),
                item.label.as_str(),
                item.description.as_str(),
                format_naive_datetime(&item.start_at),
                item.end_at.map(|value| format_naive_datetime(&value)),
                item.created_at.map(|value| format_naive_datetime(&value)),
                item.last_modified
                    .map(|value| format_naive_datetime(&value)),
            ])?;
        }
    }

    tx.commit()?;
    Ok(changes)
}

fn load_stored_items(
    conn: &Connection,
    calendar_id: &Uuid,
) -> Result<Vec<CalendarItem>, Box<dyn Error>> {
    ensure_sync_schema(conn)?;
    let mut stmt = conn.prepare(
        "SELECT item_uid, item_label, description, start_at, end_at, created_at, last_modified
         FROM sync_items
         WHERE calendar_id = ?1",
    )?;
    let mut rows = stmt.query(params![calendar_id.to_string()])?;
    let mut items = Vec::new();

    while let Some(row) = rows.next()? {
        let start_at: String = row.get(3)?;
        let end_at: Option<String> = row.get(4)?;
        let created_at: Option<String> = row.get(5)?;
        let last_modified: Option<String> = row.get(6)?;

        items.push(CalendarItem {
            uid: row.get(0)?,
            label: row.get(1)?,
            description: row.get(2)?,
            start_at: parse_stored_datetime(&start_at)?,
            end_at: end_at.as_deref().map(parse_stored_datetime).transpose()?,
            created_at: created_at
                .as_deref()
                .map(parse_stored_datetime)
                .transpose()?,
            last_modified: last_modified
                .as_deref()
                .map(parse_stored_datetime)
                .transpose()?,
        });
    }

    Ok(items)
}

fn parse_stored_datetime(value: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S"))
}

fn calendar_changes(old_items: &[CalendarItem], new_items: &[CalendarItem]) -> Vec<CalendarChange> {
    let old_by_uid = old_items
        .iter()
        .map(|item| (item.uid.as_str(), item))
        .collect::<HashMap<_, _>>();
    let new_by_uid = new_items
        .iter()
        .map(|item| (item.uid.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut changes = Vec::new();

    for item in new_items {
        match old_by_uid.get(item.uid.as_str()) {
            None => changes.push(CalendarChange::Created {
                title: item.label.clone(),
            }),
            Some(old) if calendar_item_changed(old, item) => {
                changes.push(CalendarChange::Updated {
                    title: item.label.clone(),
                })
            }
            _ => {}
        }
    }

    for item in old_items {
        if !new_by_uid.contains_key(item.uid.as_str()) {
            changes.push(CalendarChange::Removed {
                title: item.label.clone(),
            });
        }
    }

    changes
}

fn calendar_item_changed(old: &CalendarItem, new: &CalendarItem) -> bool {
    old.label != new.label
        || old.description != new.description
        || old.start_at != new.start_at
        || old.end_at != new.end_at
        || old.last_modified != new.last_modified
}

fn ensure_sync_schema(conn: &Connection) -> Result<(), Box<dyn Error>> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_calendars (
            calendar_id TEXT PRIMARY KEY NOT NULL,
            profile_id TEXT NOT NULL,
            last_synced_at TEXT,
            published_ttl_seconds INTEGER
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_items (
            calendar_id TEXT NOT NULL,
            item_uid TEXT NOT NULL,
            item_label TEXT NOT NULL,
            description TEXT NOT NULL,
            start_at TEXT NOT NULL,
            end_at TEXT,
            created_at TEXT,
            last_modified TEXT,
            FOREIGN KEY(calendar_id) REFERENCES sync_calendars(calendar_id)
        )",
        [],
    )?;

    if !has_column(conn, "sync_calendars", "last_synced_at")? {
        conn.execute(
            "ALTER TABLE sync_calendars ADD COLUMN last_synced_at TEXT",
            [],
        )?;
    }

    if !has_column(conn, "sync_calendars", "published_ttl_seconds")? {
        conn.execute(
            "ALTER TABLE sync_calendars ADD COLUMN published_ttl_seconds INTEGER",
            [],
        )?;
    }

    if !has_column(conn, "sync_items", "calendar_id")? {
        conn.execute("ALTER TABLE sync_items ADD COLUMN calendar_id TEXT", [])?;
    }

    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, Box<dyn Error>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }

    Ok(false)
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
        color: None,
    });
    save_config(&config)?;

    println!("added profile={profile_name} calendar={label}");
    Ok(())
}

pub(crate) fn read_config() -> Result<Config, Box<dyn Error>> {
    ensure_config_file()?;
    let contents = fs::read_to_string(config_path())?;
    let mut config: Config = toml::from_str(&contents)?;
    if ensure_profile_colors(&mut config) {
        save_config(&config)?;
    }

    Ok(config)
}

pub(crate) fn save_config(config: &Config) -> Result<(), Box<dyn Error>> {
    ensure_config_file()?;
    fs::write(config_path(), toml::to_string_pretty(config)?)?;
    Ok(())
}

pub(crate) fn profile_color_for_index(index: usize) -> String {
    let hue = (index as f32 * 137.508) % 360.0;
    let (r, g, b) = hsl_to_rgb(hue, 0.82, 0.48);
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = lightness - chroma / 2.0;

    let (r, g, b) = match hue {
        h if h < 60.0 => (chroma, x, 0.0),
        h if h < 120.0 => (x, chroma, 0.0),
        h if h < 180.0 => (0.0, chroma, x),
        h if h < 240.0 => (0.0, x, chroma),
        h if h < 300.0 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };

    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

pub(crate) fn calendar_id_from(profile_name: &str, url: &str) -> Uuid {
    remote_calendar_id(profile_name, url)
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

fn ensure_profile_colors(config: &mut Config) -> bool {
    let mut changed = false;

    for (index, profile) in config.profile.iter_mut().enumerate() {
        if profile.color.is_none() {
            profile.color = Some(profile_color_for_index(index));
            changed = true;
        }
    }

    changed
}

const DEFAULT_CONFIG: &str = r#"# Rusty Calendar Pi
# Profiles own the color.
# Calendars live under each profile and point at sync URLs.

profile = []
"#;

pub(crate) fn db_path() -> PathBuf {
    let mut path = data_dir();
    path.push("calendar.sqlite");
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
        if profile
            .calendar
            .iter()
            .any(|calendar| calendar.label == label)
        {
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

fn sync_calendar(
    calendar: &Calendar,
    previous_year: u32,
) -> Result<SyncCalendarResult, Box<dyn Error>> {
    let input = download_ical(&calendar.url);
    parse_calendar_items(&input, previous_year)
}

struct SyncCalendarResult {
    items: Vec<CalendarItem>,
    published_ttl_seconds: Option<i64>,
}

fn parse_calendar_items(
    input: &str,
    previous_year: u32,
) -> Result<SyncCalendarResult, Box<dyn Error>> {
    let mut parser = Parser::new(&input);
    let mut items = Vec::new();
    let mut published_ttl_seconds = None;

    loop {
        match parser.entry() {
            calcard::Entry::ICalendar(ical) => {
                for component in ical.components {
                    if matches!(component.component_type, ICalendarComponentType::VCalendar) {
                        published_ttl_seconds = get_optional_property_value_by_name(
                            &component,
                            ICalendarProperty::Other("X-PUBLISHED-TTL".to_owned()),
                        )
                        .as_deref()
                        .and_then(parse_ical_duration_seconds);
                        continue;
                    }

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

    Ok(SyncCalendarResult {
        items,
        published_ttl_seconds,
    })
}

fn parse_ical_duration_seconds(value: &str) -> Option<i64> {
    let mut chars = value.strip_prefix('P')?.chars().peekable();
    let mut in_time = false;
    let mut number = String::new();
    let mut seconds = 0_i64;

    while let Some(ch) = chars.next() {
        if ch == 'T' {
            in_time = true;
            continue;
        }

        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }

        let amount = number.parse::<i64>().ok()?;
        number.clear();
        seconds += match (in_time, ch) {
            (false, 'D') => amount * 24 * 60 * 60,
            (true, 'H') => amount * 60 * 60,
            (true, 'M') => amount * 60,
            (true, 'S') => amount,
            _ => return None,
        };
    }

    (seconds > 0 && number.is_empty()).then_some(seconds)
}

fn format_naive_datetime(value: &NaiveDateTime) -> String {
    value.format("%Y-%m-%d %H:%M:%S%.f").to_string()
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
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("{profile_name}:{url}").as_bytes(),
    )
}

fn remote_profile_id(profile_name: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, profile_name.as_bytes())
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

fn get_property_value_by_name(item: &ICalendarComponent, name: ICalendarProperty) -> String {
    if let Some(value) = get_optional_property_value_by_name(item, name.clone()) {
        return value;
    }

    match name {
        ICalendarProperty::Description | ICalendarProperty::Summary => "".to_owned(),
        _ => {
            if name == ICalendarProperty::Other("X-WR-TIMEZONE".to_owned()) {
                return "".to_owned();
            }

            dbg!(item);
            todo!("Entry item \"{:?}\" by name not found!", name)
        }
    }
}

fn get_optional_property_value_by_name(
    item: &ICalendarComponent,
    name: ICalendarProperty,
) -> Option<String> {
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
        None => return None,
    }
    .into()
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
                // dbg!(item);
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
        assert!(matches!(
            parse_calendar_type_choice("1"),
            Some(CalendarType::ICS)
        ));
        assert!(matches!(
            parse_calendar_type_choice("2"),
            Some(CalendarType::Gmail)
        ));
        assert!(parse_calendar_type_choice("nope").is_none());
    }

    #[test]
    fn validates_calendar_url_prefix() {
        assert!(is_valid_calendar_url("https://example.com/feed.ics"));
        assert!(!is_valid_calendar_url("ftp://example.com/feed.ics"));
    }

    #[test]
    fn parses_snapshot_items_without_delta_filtering() {
        let input = "BEGIN:VCALENDAR\n\
VERSION:2.0\n\
BEGIN:VEVENT\n\
UID:google-item\n\
SUMMARY:Google item\n\
DESCRIPTION:from gmail\n\
DTSTART:20260102T090000Z\n\
CREATED:20200101T090000Z\n\
LAST-MODIFIED:20200101T090000Z\n\
END:VEVENT\n\
END:VCALENDAR";

        let items = parse_calendar_items(input, 2025).unwrap();

        assert_eq!(items.items.len(), 1);
        assert_eq!(items.items[0].label, "Google item");
    }

    #[test]
    fn parses_published_ttl_duration() {
        assert_eq!(parse_ical_duration_seconds("PT30M"), Some(30 * 60));
        assert_eq!(parse_ical_duration_seconds("PT1H"), Some(60 * 60));
        assert_eq!(parse_ical_duration_seconds("P1D"), Some(24 * 60 * 60));
        assert_eq!(parse_ical_duration_seconds("nope"), None);
    }

    #[test]
    fn detects_calendar_item_changes() {
        let kept = test_item("kept", "Kept", 9);
        let old_updated = test_item("updated", "Old", 10);
        let removed = test_item("removed", "Removed", 11);
        let new_updated = test_item("updated", "New", 10);
        let created = test_item("created", "Created", 12);

        let changes = calendar_changes(
            &[kept.clone(), old_updated, removed],
            &[kept, new_updated, created],
        );

        assert!(matches!(
            changes[0],
            CalendarChange::Updated { ref title } if title == "New"
        ));
        assert!(matches!(
            changes[1],
            CalendarChange::Created { ref title } if title == "Created"
        ));
        assert!(matches!(
            changes[2],
            CalendarChange::Removed { ref title } if title == "Removed"
        ));
    }

    #[test]
    fn selects_all_calendars_by_default() {
        let config = Config {
            profile: vec![
                ConfigProfile {
                    name: "work".to_owned(),
                    color: None,
                    calendar: vec![ConfigCalendar {
                        label: "primary".to_owned(),
                        account: "a".to_owned(),
                        cal_type: CalendarType::ICS,
                        url: "https://a".to_owned(),
                        color: None,
                    }],
                },
                ConfigProfile {
                    name: "home".to_owned(),
                    color: None,
                    calendar: vec![ConfigCalendar {
                        label: "shared".to_owned(),
                        account: "b".to_owned(),
                        cal_type: CalendarType::Gmail,
                        url: "https://b".to_owned(),
                        color: None,
                    }],
                },
            ],
        };

        let selected = selected_calendars(&config, None, None);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].0.name, "work");
        assert_eq!(selected[1].0.name, "home");
    }

    #[test]
    fn stores_synced_items_in_sqlite() {
        let mut conn = Connection::open_in_memory().unwrap();
        let profile_id = Uuid::nil();
        let calendar = Calendar {
            id: Uuid::nil(),
            uid: "uid-1".to_owned(),
            url: "https://example.com/feed.ics".to_owned(),
            label: "Work".to_owned(),
            timezone: String::new(),
            publish_ttl: None,
        };
        let items = vec![CalendarItem {
            uid: "item-1".to_owned(),
            label: "Standup".to_owned(),
            description: "Daily sync".to_owned(),
            start_at: chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap(),
            end_at: None,
            created_at: None,
            last_modified: None,
        }];

        let synced_at = chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();

        persist_sync_items(&mut conn, &profile_id, &calendar, &items, synced_at).unwrap();

        let calendar_row: (String, String) = conn
            .query_row(
                "SELECT calendar_id, profile_id FROM sync_calendars",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        let stored_synced_at: String = conn
            .query_row("SELECT last_synced_at FROM sync_calendars", [], |row| {
                row.get(0)
            })
            .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_items", [], |row| row.get(0))
            .unwrap();
        let calendar_id: String = conn
            .query_row("SELECT calendar_id FROM sync_items", [], |row| row.get(0))
            .unwrap();

        assert_eq!(calendar_row.0, calendar.id.to_string());
        assert_eq!(calendar_row.1, profile_id.to_string());
        assert_eq!(stored_synced_at, format_naive_datetime(&synced_at));
        assert_eq!(count, 1);
        assert_eq!(calendar_id, calendar.id.to_string());
    }

    #[test]
    fn profiles_get_generated_vibrant_colors() {
        let first = profile_color_for_index(0);
        let second = profile_color_for_index(1);

        assert_eq!(first.len(), 7);
        assert!(first.starts_with('#'));
        assert_ne!(first, second);
        assert!(crate::theme::parse_hex_color(&first).is_some());
        assert!(color_channel_spread(&first) > 120);
    }

    fn color_channel_spread(hex: &str) -> u8 {
        let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
        let channels = [
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as u8,
        ];

        channels.iter().max().unwrap() - channels.iter().min().unwrap()
    }

    fn test_item(uid: &str, label: &str, hour: u32) -> CalendarItem {
        CalendarItem {
            uid: uid.to_owned(),
            label: label.to_owned(),
            description: String::new(),
            start_at: chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
                .unwrap()
                .and_hms_opt(hour, 0, 0)
                .unwrap(),
            end_at: None,
            created_at: None,
            last_modified: None,
        }
    }
}
