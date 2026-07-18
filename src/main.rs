mod calendar;
mod components;
mod icons;
mod layout;
mod node;
mod profile;
mod renderer;
mod table;
mod theme;

use argh::FromArgs;
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, NaiveDateTime, Weekday};
use rusqlite::{Connection, params};
use std::{collections::BTreeMap, error::Error};
use cosmic_text::Align;
use tiny_skia::Color;
use taffy::{AlignItems, FlexDirection, JustifyContent, NodeId};
use tracing::{debug, info, trace};

use crate::components::{div, icon, pill, text};
use crate::layout::AppLayout;
use crate::node::builder::BobTheBuilder;
use crate::theme::THEME;

#[derive(FromArgs)]
/// Rusty Calendar Pi
struct Cli {
    #[argh(subcommand)]
    command: Option<Command>,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Sync(SyncArgs),
    Profile(profile::ProfileArgs),
    Calendar(calendar::CalendarArgs),
}

#[derive(FromArgs)]
#[argh(subcommand, name = "sync")]
/// Sync configured calendars
struct SyncArgs {
    /// sync only this profile
    #[argh(option)]
    profile: Option<String>,

    /// sync only this calendar within the selected profile
    #[argh(option)]
    calendar: Option<String>,
}

fn build_layout(layout: &mut AppLayout) -> (NodeId, NodeId, NodeId) {
    let header = div()
        .border_color(THEME.border)
        .name(node::NodeName::Header)
        .height(64.0)
        .border_b(1.0)
        .build(layout);

    let content = div()
        .name(node::NodeName::Content)
        .width_full()
        .flex_dir_column()
        .layout(|l| {
            l.flex_grow = 1.0;
            l.flex_shrink = 1.0;
        })
        .build(layout);

    let footer = div()
        .name(node::NodeName::Footer)
        .width_full()
        .height(32.0)
        .border_color(THEME.border)
        .border_t(1.)
        .build(layout);

    (header, content, footer)
}

const CAL_COLS: usize = 7;
const CAL_ROWS: usize = 4;

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli: Cli = argh::from_env();

    match cli.command {
        Some(Command::Sync(args)) => {
            crate::calendar::sync(args.profile.as_deref(), args.calendar.as_deref())?
        }
        Some(Command::Profile(args)) => match args.command {
            Some(profile::ProfileCommand::Add(_)) => crate::profile::profile_add()?,
            None => crate::profile::list_profiles()?,
        },
        Some(Command::Calendar(args)) => match args.command {
            Some(calendar::CalendarCommand::Add(_)) => crate::calendar::calendar_add()?,
            None => crate::calendar::list_calendars()?,
        },
        None => launch_app(),
    }

    Ok(())
}

fn launch_app() {
    info!("launch app");
    let mut layout = AppLayout::new();

    let (_header, content, _footer) = build_layout(&mut layout);
    let items_by_date = seed_demo_calendar_items().expect("failed to seed demo calendar items");

    let today = Local::now();

    let headers = get_weekdays();
    components::grid("calendar_weekday", 7, None)
        .height_auto()
        .flex_no_grow()
        .border_color(THEME.border)
        .border_b(1.)
        .children(
            headers
                .iter()
                .map(|day| text(day).py(5.).border_color(THEME.border))
                .collect(),
        )
        .parent_node(content)
        .build(&mut layout);

    let mut dates = get_dates((CAL_COLS * CAL_ROWS) as u32).into_iter();
    components::grid("calendar", 7, Some(4))
        .border_color(THEME.border)
        .height_full()
        .parent_node(content)
        .foreach_children(|kid, _i| {
            trace!(cell = _i, "fill calendar cell");
            kid.set_layout(|l| {
                l.flex_direction = FlexDirection::Column;
            });

            let date = dates.next().expect("Cannot pop a date");

            if today.date_naive().eq(&date) {
                println!("TODAY IS THE DAY {}", date);
            }

            let label = format!("calendar-cell_{}", date).to_owned();
            kid.add_child(
                div()
                    .width_full()
                    .layout(|l| {
                        l.align_items = Some(AlignItems::Center);
                        l.justify_content = Some(JustifyContent::Center);
                    })
                    .child(if today.date_naive().eq(&date) {
                        pill(format!("{}", date.day())).background(THEME.warning)
                    } else {
                        text(format!("{}", date.day())).py(5.)
                    })
                    .name(node::NodeName::other(label)),
            );

            if let Some(items) = items_by_date.get(&date) {
                for item in items {
                    trace!(cell = _i, date = %date, item = %item.title, "render cell item");
                    kid.add_child(render_demo_item(item));
                }
            }
        })
        .build(&mut layout);

    debug!("layout built");

    renderer::run(layout);
}

fn get_weekdays() -> Vec<String> {
    vec!["Ma", "Di", "Wo", "Do", "Vr", "Za", "Zo"]
        .iter_mut()
        .map(|v| v.to_owned())
        .collect()
}

fn get_dates(how_many: u32) -> Vec<NaiveDate> {
    let local: DateTime<Local> = Local::now();
    let week = local.iso_week().week();
    let begin = NaiveDate::from_isoywd_opt(local.year(), week, Weekday::Mon).unwrap();

    let mut dates = Vec::with_capacity(how_many as usize);
    dates.push(begin);
    for i in 1..how_many {
        let next = begin.checked_add_days(Days::new(i.into())).unwrap();
        dates.push(next);
    }

    dates
}

fn seed_demo_calendar_items() -> Result<BTreeMap<NaiveDate, Vec<DemoItem>>, Box<dyn Error>> {
    debug!("seed demo calendar items");
    let conn = Connection::open_in_memory()?;
    conn.execute("CREATE TABLE profiles (name TEXT PRIMARY KEY NOT NULL)", [])?;
    conn.execute(
        "CREATE TABLE calendars (
            profile_name TEXT NOT NULL,
            name TEXT NOT NULL,
            PRIMARY KEY(profile_name, name)
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE items (
            profile_name TEXT NOT NULL,
            calendar_name TEXT NOT NULL,
            title TEXT NOT NULL,
            start_at TEXT NOT NULL,
            presentation TEXT NOT NULL,
            icon_name TEXT
        )",
        [],
    )?;

    let week_start = get_dates(1)
        .into_iter()
        .next()
        .expect("expected at least one date");

    let profiles = [
        ("Work", "Primary", "16/time.svg"),
        ("Home", "Shared", "16/user.svg"),
        ("Family", "Kids", "16/star.svg"),
        ("Side", "Project", "16/launch.svg"),
    ];

    for (profile_name, calendar_name, _) in profiles {
        conn.execute("INSERT INTO profiles (name) VALUES (?1)", params![profile_name])?;
        conn.execute(
            "INSERT INTO calendars (profile_name, name) VALUES (?1, ?2)",
            params![profile_name, calendar_name],
        )?;
    }

    let items = [
        ("Work", "Primary", 0, 8, 0, "Standup", DemoItemPresentation::Icon, Some("20/time.svg")),
        ("Work", "Primary", 0, 10, 30, "Planning", DemoItemPresentation::IconInPill, Some("20/checkmark.svg")),
        ("Work", "Primary", 0, 13, 0, "Review", DemoItemPresentation::Pill, None),
        ("Home", "Shared", 1, 9, 0, "School run", DemoItemPresentation::Icon, Some("16/user.svg")),
        ("Home", "Shared", 1, 12, 0, "Dinner prep", DemoItemPresentation::IconInPill, Some("16/notification.svg")),
        ("Home", "Shared", 1, 18, 0, "Quiet time", DemoItemPresentation::Pill, None),
        ("Family", "Kids", 2, 7, 45, "Breakfast", DemoItemPresentation::Icon, Some("16/star.svg")),
        ("Family", "Kids", 2, 11, 15, "Practice", DemoItemPresentation::IconInPill, Some("16/play.svg")),
        ("Family", "Kids", 2, 17, 0, "Game night", DemoItemPresentation::Pill, None),
        ("Side", "Project", 3, 11, 0, "Write", DemoItemPresentation::Icon, Some("16/launch.svg")),
        ("Side", "Project", 3, 16, 30, "Ship", DemoItemPresentation::IconInPill, Some("16/save.svg")),
        ("Side", "Project", 3, 19, 0, "Retro", DemoItemPresentation::Pill, None),
    ];

    for (profile_name, calendar_name, day_offset, hour, minute, title, presentation, icon_name) in items {
        let start_at = week_start
            .checked_add_days(Days::new(day_offset))
            .expect("invalid day offset")
            .and_hms_opt(hour, minute, 0)
            .expect("invalid time");

        conn.execute(
            "INSERT INTO items (profile_name, calendar_name, title, start_at, presentation, icon_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                profile_name,
                calendar_name,
                title,
                start_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                presentation.as_str(),
                icon_name,
            ],
        )?;
    }

    let mut stmt = conn.prepare(
        "SELECT profile_name, calendar_name, title, start_at, presentation, icon_name
         FROM items
         ORDER BY start_at",
    )?;
    let mut rows = stmt.query([])?;
    let mut items_by_date = BTreeMap::new();

    while let Some(row) = rows.next()? {
        let profile_name: String = row.get(0)?;
        let calendar_name: String = row.get(1)?;
        let title: String = row.get(2)?;
        let start_at = NaiveDateTime::parse_from_str(&row.get::<_, String>(3)?, "%Y-%m-%d %H:%M:%S")?;
        let presentation = DemoItemPresentation::from_db(row.get::<_, String>(4)?.as_str());
        let icon_name = row.get::<_, Option<String>>(5)?;

        items_by_date
            .entry(start_at.date())
            .or_insert_with(Vec::new)
            .push(DemoItem {
                profile_name,
                calendar_name,
                title,
                start_at,
                presentation,
                icon_name,
            });
    }

    Ok(items_by_date)
}

fn render_demo_item(item: &DemoItem) -> crate::node::builder::Builder {
    let label = item.label();
    let time = item.start_at.format("%H:%M").to_string();
    let accent = profile_color(&item.profile_name);
    let on_accent = readable_on(accent);
    let icon_slot = |icon_name: Option<&str>, color: Color| match icon_name {
        Some(name) => icon(name).width(16.).height(16.).text_color(color),
        None => div().width(16.).height(16.),
    };
    let time_slot = |color: Color| {
        text(time.clone())
        .width(56.)
        .text_color(color)
        .text_align(Align::Left)
    };
    let label_slot = |color: Color| {
        text(label.clone())
            .width_auto()
            .layout(|l| {
                l.flex_grow = 1.0;
                l.flex_shrink = 1.0;
            })
            .text_color(color)
            .text_align(Align::Left)
    };
    let item_row = div().width_full().layout(|l| {
        l.flex_direction = FlexDirection::Row;
        l.align_items = Some(AlignItems::Center);
        l.margin.top = taffy::prelude::length(4.);
    });
    let name = node::NodeName::other(format!(
        "calendar-item_{}_{}_{}",
        item.start_at,
        item.profile_name,
        item.title
    ));

    debug!(
        start_at = %item.start_at,
        presentation = ?item.presentation,
        icon = ?item.icon_name,
        "render demo item"
    );

    match item.presentation {
        DemoItemPresentation::Icon => item_row.clone()
            .px(4.)
            .child(icon_slot(item.icon_name.as_deref(), accent))
            .child(time_slot(THEME.text_muted).px(4.))
            .child(label_slot(THEME.text).px(6.))
            .name(name),
        DemoItemPresentation::IconInPill => item_row
            .clone()
            .py(2.)
            .px(4.)
            .rounded_xl()
            .background(accent)
            .child(icon_slot(item.icon_name.as_deref(), on_accent))
            .child(time_slot(on_accent).px(4.))
            .child(label_slot(on_accent).px(6.))
            .name(name),
        DemoItemPresentation::Pill => item_row
            .clone()
            .py(2.)
            .px(4.)
            .rounded_xl()
            .background(accent)
            .child(icon_slot(item.icon_name.as_deref(), on_accent))
            .child(time_slot(on_accent).px(4.))
            .child(label_slot(on_accent).px(6.))
            .name(name),
    }
}

fn readable_on(color: Color) -> Color {
    let c = color.to_color_u8();
    let luminance = u32::from(c.red()) * 299 + u32::from(c.green()) * 587 + u32::from(c.blue()) * 114;

    if luminance < 128_000 {
        Color::from_rgba8(255, 255, 255, 255)
    } else {
        Color::from_rgba8(0, 0, 0, 255)
    }
}

fn profile_color(profile_name: &str) -> Color {
    match profile_name {
        "Work" => THEME.primary,
        "Home" => THEME.success,
        "Family" => THEME.warning,
        _ => THEME.danger,
    }
}

#[derive(Clone, Debug)]
struct DemoItem {
    profile_name: String,
    calendar_name: String,
    title: String,
    start_at: NaiveDateTime,
    presentation: DemoItemPresentation,
    icon_name: Option<String>,
}

impl DemoItem {
    fn label(&self) -> String {
        self.title.clone()
    }
}

#[derive(Clone, Copy, Debug)]
enum DemoItemPresentation {
    Icon,
    IconInPill,
    Pill,
}

impl DemoItemPresentation {
    fn from_db(value: &str) -> Self {
        match value {
            "icon" => Self::Icon,
            "icon-pill" => Self::IconInPill,
            _ => Self::Pill,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Icon => "icon",
            Self::IconInPill => "icon-pill",
            Self::Pill => "pill",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_demo_items_in_time_order() {
        let items_by_date = seed_demo_calendar_items().unwrap();
        let monday = items_by_date.values().next().unwrap();

        assert_eq!(items_by_date.len(), 4);
        assert_eq!(monday.len(), 3);
        assert!(matches!(monday[0].presentation, DemoItemPresentation::Icon));
        assert!(matches!(monday[1].presentation, DemoItemPresentation::IconInPill));
        assert!(matches!(monday[2].presentation, DemoItemPresentation::Pill));
    }
}
