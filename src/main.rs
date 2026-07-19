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
use cosmic_text::Align;
use rusqlite::Connection;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
};
use taffy::{AlignItems, FlexDirection, JustifyContent, NodeId, prelude::length};
use tiny_skia::Color;
use tracing::{debug, info, trace};

use crate::layout::AppLayout;
use crate::node::builder::BobTheBuilder;
use crate::theme::THEME;
use crate::{
    components::{div, grid, pill, text},
    node::builder::Builder,
};

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
    let config = crate::calendar::read_config().expect("failed to load config");
    let items_by_date = load_calendar_items(&config).expect("failed to load calendar items");

    let today = Local::now();

    let headers = get_weekdays();
    grid("calendar_weekday", CAL_COLS + 1, None)
        .height_auto()
        .flex_no_grow()
        .border_color(THEME.border)
        .border_b(1.)
        .layout(|l| {
            l.grid_template_columns[0] = length(33.);
        })
        .children(
            headers
                .iter()
                .map(|day| text(day).py(5.).border_color(THEME.border))
                .collect(),
        )
        .parent_node(content)
        .build(&mut layout);

    let mut weeknumbers: Vec<Builder> = get_weeknumbers(CAL_ROWS)
        .iter()
        .map(|num| text(num.to_string()))
        .collect();

    println!("{:?}", weeknumbers.len());

    let mut dates = get_dates((CAL_COLS * CAL_ROWS) as u32).into_iter();
    grid("calendar", CAL_COLS + 1, Some(CAL_ROWS))
        .border_color(THEME.border)
        .height_full()
        .parent_node(content)
        .layout(|l| {
            l.grid_template_columns[0] = length(33.);
        })
        .foreach_children(|kid, i| {
            trace!(cell = i, "fill calendar cell");
            kid.set_layout(|l| {
                l.flex_direction = FlexDirection::Column;
            });

            // when first col of the row
            if i % (CAL_COLS + 1) == 0 {
                kid.add_child(weeknumbers.pop().expect("Dont have any weeks left"));
                return;
            }

            let date = dates.next().expect("Cannot pop a date");

            // if today.date_naive().eq(&date) {
            //     println!("TODAY IS THE DAY {}", date);
            // }

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
                    trace!(cell = i, date = %date, item = %item.title, "render cell item");
                    kid.add_child(render_calendar_item(item));
                }
            }
        })
        .build(&mut layout);

    debug!("layout built");

    renderer::run(layout);
}

fn get_weekdays() -> Vec<String> {
    vec!["", "Ma", "Di", "Wo", "Do", "Vr", "Za", "Zo"]
        .iter_mut()
        .map(|v| v.to_owned())
        .collect()
}

/// Get week numbers for a num amount of weeks and return them in a vec.
fn get_weeknumbers(num_of_weeks: usize) -> Vec<u32> {
    let local = Local::now().naive_local();

    let mut weeks = Vec::with_capacity(num_of_weeks);
    weeks.push(local.iso_week().week());
    for i in 1..num_of_weeks {
        if let Some(d) = local.checked_add_days(Days::new((i * 7) as u64)) {
            weeks.push(d.iso_week().week());
        }
    }

    weeks
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

fn load_calendar_items(
    config: &crate::calendar::Config,
) -> Result<BTreeMap<NaiveDate, Vec<CalendarItem>>, Box<dyn Error>> {
    let mut items_by_date = BTreeMap::new();
    let db_path = crate::calendar::db_path();
    let profile_colors: HashMap<String, Color> = config
        .profile
        .iter()
        .map(|profile| {
            let profile_id = crate::calendar::profile_id_from_name(&profile.name).to_string();
            let color = profile
                .color
                .as_deref()
                .and_then(theme::parse_hex_color)
                .unwrap_or(THEME.primary);
            (profile_id, color)
        })
        .collect();

    if !db_path.exists() {
        return Ok(items_by_date);
    }

    let conn = Connection::open(db_path)?;
    let mut stmt = match conn.prepare(
        "SELECT sync_calendars.profile_id, sync_items.item_label, sync_items.start_at
         FROM sync_items
         JOIN sync_calendars ON sync_calendars.calendar_id = sync_items.calendar_id
         ORDER BY sync_items.start_at",
    ) {
        Ok(stmt) => stmt,
        Err(err) => {
            debug!(?err, "no synced calendar items yet");
            return Ok(items_by_date);
        }
    };

    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let profile_id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let start_at = parse_calendar_datetime(&row.get::<_, String>(2)?)?;
        let accent = profile_colors
            .get(&profile_id)
            .copied()
            .unwrap_or(THEME.primary);

        items_by_date
            .entry(start_at.date())
            .or_insert_with(Vec::new)
            .push(CalendarItem {
                title,
                start_at,
                accent,
            });
    }

    Ok(items_by_date)
}

fn render_calendar_item(item: &CalendarItem) -> crate::node::builder::Builder {
    let accent = item.accent;
    let on_accent = readable_on(accent);
    let time = item.start_at.format("%H:%M").to_string();

    div()
        .width_full()
        .py(2.)
        .px(4.)
        .rounded_xl()
        .background(accent)
        .layout(|l| {
            l.flex_direction = FlexDirection::Row;
            l.align_items = Some(AlignItems::Center);
            l.margin.top = taffy::prelude::length(4.);
        })
        .child(
            text(time)
                .width(56.)
                .text_color(on_accent)
                .text_align(Align::Left),
        )
        .child(
            text(item.title.clone())
                .width_auto()
                .layout(|l| {
                    l.flex_grow = 1.0;
                    l.flex_shrink = 1.0;
                })
                .text_color(on_accent)
                .text_align(Align::Left),
        )
}

fn parse_calendar_datetime(value: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S"))
}

fn readable_on(color: Color) -> Color {
    let c = color.to_color_u8();
    let luminance =
        u32::from(c.red()) * 299 + u32::from(c.green()) * 587 + u32::from(c.blue()) * 114;

    if luminance < 128_000 {
        Color::from_rgba8(255, 255, 255, 255)
    } else {
        Color::from_rgba8(0, 0, 0, 255)
    }
}

#[derive(Clone, Debug)]
struct CalendarItem {
    title: String,
    start_at: NaiveDateTime,
    accent: Color,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_color() {
        assert_eq!(theme::parse_hex_color("#1e66f5"), Some(THEME.primary));
    }
}
