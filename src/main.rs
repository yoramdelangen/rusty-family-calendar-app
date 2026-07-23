mod app;
mod calendar;
mod components;
mod event;
mod icons;
mod layout;
mod node;
mod profile;
mod renderer;
mod table;
mod theme;

use argh::FromArgs;
use chrono::{Datelike, Days, Duration, Local, NaiveDate, NaiveDateTime};
use cosmic_text::Align;
use rusqlite::Connection;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
};
use taffy::{AlignItems, FlexDirection, JustifyContent, NodeId, prelude::length};
use tiny_skia::Color;
use tracing::{debug, error, info, trace};
use tracing_subscriber::EnvFilter;

use crate::layout::AppLayout;
use crate::layout::LayoutAction;
use crate::node::builder::BobTheBuilder;
use crate::theme::{THEME, font::FONT};
use crate::{
    components::{ButtonContent, button, div, grid, icon, pill, text},
    node::builder::Builder,
    node::{EventCaps, NodeKind},
};

pub(crate) const SYNC_ICON_NODE: &str = "sync-icon";
pub(crate) const SYNC_STATUS_NODE: &str = "sync-status";
pub(crate) const SYNC_CHANGES_NODE: &str = "sync-changes";
pub(crate) const VERSION_NODE: &str = "version";

#[cfg(debug_assertions)]
const APP_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-dev");

#[cfg(not(debug_assertions))]
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "macos")]
const FOOTER_PX: f32 = 16.0;

#[cfg(not(target_os = "macos"))]
const FOOTER_PX: f32 = 10.0;

#[cfg(target_os = "macos")]
const HEADER_LEFT_INSET: f32 = 146.0;

#[cfg(not(target_os = "macos"))]
const HEADER_LEFT_INSET: f32 = 0.0;

#[cfg(target_os = "macos")]
const HEADER_CONTENT_TOP_OFFSET: f32 = 6.0;

#[cfg(not(target_os = "macos"))]
const HEADER_CONTENT_TOP_OFFSET: f32 = 0.0;

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

fn build_layout(layout: &mut AppLayout, visible_start: NaiveDate, week_offset: i64) -> (NodeId, NodeId, NodeId) {
    let mut header_controls = Builder::new(NodeKind::Container, None)
        .width_auto()
        .events(EventCaps::empty())
        .layout(|l| {
            l.flex_direction = FlexDirection::Row;
            l.align_items = Some(AlignItems::Center);
            l.margin.left = length(HEADER_LEFT_INSET);
            l.margin.top = length(HEADER_CONTENT_TOP_OFFSET);
        })
        .child(
            text(header_month_label(visible_start))
                .width(160.)
                .font_size(FONT.xl.clone())
                .bold()
                .text_align(Align::Left),
        )
        .child(
            button(ButtonContent::Icon("16/chevron--left"))
                .width(36.)
                .height(36.)
                .rounded(18.)
                .on_click(|layout, _| layout.queue_action(LayoutAction::ShiftWeeks(-2)))
                .layout(|l| {
                    l.margin.left = length(12.);
                }),
        )
        .child(
            button(ButtonContent::Icon("16/chevron--right"))
                .width(36.)
                .height(36.)
                .rounded(18.)
                .on_click(|layout, _| layout.queue_action(LayoutAction::ShiftWeeks(2)))
                .layout(|l| {
                    l.margin.left = length(8.);
                }),
        );

    if week_offset != 0 {
        header_controls = header_controls.child(
            button(ButtonContent::Text("Today".to_owned()))
                .on_click(|layout, _| layout.queue_action(LayoutAction::ResetWeeks))
                .layout(|l| {
                    l.margin.left = length(8.);
                }),
        );
    }

    let header = div()
        .border_color(THEME.border)
        .name(node::NodeName::Header)
        .height(64.0)
        .px(16.)
        .border_b(1.0)
        .layout(|l| {
            l.align_items = Some(AlignItems::Center);
            l.justify_content = Some(JustifyContent::SpaceBetween);
        })
        .child(header_controls)
        .child(
            text(Local::now().format("%H:%M:%S").to_string())
                .name(node::NodeName::Clock)
                .width(96.)
                .text_align(Align::Center)
                .px(16.)
                .layout(|l| {
                    l.margin.top = length(HEADER_CONTENT_TOP_OFFSET);
                }),
        )
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
        .height(48.0)
        .px(FOOTER_PX)
        .border_color(THEME.border)
        .border_t(1.)
        .layout(|l| {
            l.flex_direction = FlexDirection::Row;
            l.align_items = Some(AlignItems::Center);
        })
        .child(
            icon("circle-fill")
                .name(node::NodeName::other(SYNC_ICON_NODE))
                .width(18.)
                .height(18.)
                .text_color(THEME.text)
                .px(5.),
        )
        .child(
            text("next sync pending")
                .name(node::NodeName::other(SYNC_STATUS_NODE))
                .width(180.)
                .text_color(THEME.text),
        )
        .child(
            text("no sync changes yet")
                .name(node::NodeName::other(SYNC_CHANGES_NODE))
                .width(0.)
                .text_color(THEME.text)
                .ellipsis()
                .layout(|l| {
                    l.flex_grow = 1.0;
                    l.flex_shrink = 1.0;
                }),
        )
        .child(
            text(footer_version_text())
                .name(node::NodeName::other(VERSION_NODE))
                .width_auto()
                .text_color(THEME.text)
                .text_align(Align::Right),
        )
        .build(layout);

    (header, content, footer)
}

const CAL_COLS: usize = 7;
const CAL_ROWS: usize = 4;

fn main() {
    let cli: Cli = argh::from_env();
    init_logging();

    if let Err(err) = run(cli) {
        error!(error = %err, "application failed");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
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
    let sync_rx = crate::calendar::start_sync_worker();
    renderer::run(app::App::new(sync_rx));
}

pub(crate) fn build_app_layout(week_offset: i64) -> AppLayout {
    let mut layout = AppLayout::new();
    let visible_start = visible_start_for_offset(week_offset);

    let (_header, content, _footer) = build_layout(&mut layout, visible_start, week_offset);
    let config = crate::calendar::read_config().expect("failed to load config");
    let items_by_date = load_calendar_items(&config).expect("failed to load calendar items");

    let today = Local::now().date_naive();

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
                .map(|day| text(day).py(5.).font_size(FONT.sm.clone()).border_color(THEME.border))
                .collect(),
        )
        .parent_node(content)
        .build(&mut layout);

    let mut weeknumbers: Vec<Builder> = get_weeknumbers(visible_start, CAL_ROWS)
        .iter()
        .map(|num| text(num.to_string()).font_size(FONT.sm.clone()))
        .collect();

    trace!(weeks = weeknumbers.len(), "week numbers prepared");

    let mut dates = get_dates(visible_start, (CAL_COLS * CAL_ROWS) as u32).into_iter();
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
            let is_today = today.eq(&date);

            if is_today {
                kid.style.background_color = Some(subtle_today_background());
            }

            let label = format!("calendar-cell_{}", date).to_owned();
            kid.add_child(
                div()
                    .width_full()
                    .layout(|l| {
                        l.align_items = Some(AlignItems::Center);
                        l.justify_content = Some(JustifyContent::Center);
                    })
                    .child(text(format!("{}", date.day())).py(5.).font_size(FONT.sm.clone()))
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
    layout
}

fn get_weekdays() -> Vec<String> {
    vec!["", "Ma", "Di", "Wo", "Do", "Vr", "Za", "Zo"]
        .iter_mut()
        .map(|v| v.to_owned())
        .collect()
}

/// Get week numbers for a num amount of weeks and return them in a vec.
fn get_weeknumbers(begin: NaiveDate, num_of_weeks: usize) -> Vec<u32> {
    let mut weeks = Vec::with_capacity(num_of_weeks);
    weeks.push(begin.iso_week().week());
    for i in 1..num_of_weeks {
        if let Some(d) = begin.checked_add_days(Days::new((i * 7) as u64)) {
            weeks.push(d.iso_week().week());
        }
    }

    weeks
}

fn get_dates(begin: NaiveDate, how_many: u32) -> Vec<NaiveDate> {
    let mut dates = Vec::with_capacity(how_many as usize);
    dates.push(begin);
    for i in 1..how_many {
        let next = begin.checked_add_days(Days::new(i.into())).unwrap();
        dates.push(next);
    }

    dates
}

fn visible_start_for_offset(week_offset: i64) -> NaiveDate {
    let today = Local::now().date_naive();
    let start = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    start
        .checked_add_signed(Duration::days(week_offset * 7))
        .expect("invalid visible start")
}

fn header_month_label(visible_start: NaiveDate) -> String {
    let month = dutch_month_name(visible_start.month());
    let mut chars = month.chars();
    let month = match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => month.to_owned(),
    };

    format!("{month} {}", visible_start.year())
}

fn dutch_month_name(month: u32) -> &'static str {
    match month {
        1 => "januari",
        2 => "februari",
        3 => "maart",
        4 => "april",
        5 => "mei",
        6 => "juni",
        7 => "juli",
        8 => "augustus",
        9 => "september",
        10 => "oktober",
        11 => "november",
        12 => "december",
        _ => unreachable!("invalid month"),
    }
}

fn load_calendar_items(
    config: &crate::calendar::Config,
) -> Result<BTreeMap<NaiveDate, Vec<CalendarItem>>, Box<dyn Error>> {
    let mut items_by_date = BTreeMap::new();
    let db_path = crate::calendar::db_path();
    let calendar_style: HashMap<String, CalendarItemStyle> = config
        .profile
        .iter()
        .flat_map(|profile| {
            let profile_color = profile
                .color
                .as_deref()
                .and_then(theme::parse_hex_color)
                .unwrap_or(THEME.primary);
            profile.calendar.iter().map(move |calendar| {
                let color = calendar
                    .color
                    .as_deref()
                    .and_then(theme::parse_hex_color)
                    .unwrap_or(profile_color);
                let pill = calendar.pill.unwrap_or(profile.pill);
                (
                    crate::calendar::calendar_id_from(&profile.name, &calendar.url).to_string(),
                    CalendarItemStyle { color, pill },
                )
            })
        })
        .collect();

    if !db_path.exists() {
        return Ok(items_by_date);
    }

    let conn = Connection::open(db_path)?;
    let mut stmt = match conn.prepare(
        "SELECT sync_items.calendar_id, sync_items.item_label, sync_items.start_at
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
        let calendar_id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let start_at = parse_calendar_datetime(&row.get::<_, String>(2)?)?;
        let style = calendar_style
            .get(&calendar_id)
            .copied()
            .unwrap_or(CalendarItemStyle {
                color: THEME.primary,
                pill: false,
            });

        items_by_date
            .entry(start_at.date())
            .or_insert_with(Vec::new)
            .push(CalendarItem {
                title,
                start_at,
                accent: style.color,
                pill: style.pill,
            });
    }

    Ok(items_by_date)
}

fn render_calendar_item(item: &CalendarItem) -> crate::node::builder::Builder {
    let accent = item.accent;
    let on_accent = readable_on(accent);
    let time = item.start_at.format("%H:%M").to_string();

    trace!(title = %item.title, start_at = %item.start_at, "render calendar item");

    if item.pill {
        return pill(format!("{time}  {}", item.title))
            .width_full()
            .font_size(FONT.sm.clone())
            .text_color(on_accent)
            .background(accent)
            .ellipsis()
            .layout(|l| {
                l.margin.top = length(4.);
            });
    }

    div().width_full().layout(|l| {
        l.flex_direction = FlexDirection::Row;
        l.align_items = Some(AlignItems::Center);
        l.margin.top = length(4.);
    })
    .child(
        text(time)
            .width(50.)
            .font_size(FONT.sm.clone())
            .text_color(accent)
            .text_align(Align::Left),
    )
    .child(
        text(item.title.clone())
            .width(0.)
            .font_size(FONT.sm.clone())
            .layout(|l| {
                l.flex_grow = 1.0;
                l.flex_shrink = 1.0;
            })
            .ellipsis()
            .text_color(THEME.text)
            .text_align(Align::Left),
    )
}

fn init_logging() {
    let filter = EnvFilter::try_new(crate::calendar::configured_log_level())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn footer_version_text() -> String {
    format!("{APP_VERSION} {}", crate::calendar::configured_log_level())
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

fn subtle_today_background() -> Color {
    let c = THEME.primary.to_color_u8();
    Color::from_rgba8(c.red(), c.green(), c.blue(), 24)
}

#[derive(Clone, Debug)]
struct CalendarItem {
    title: String,
    start_at: NaiveDateTime,
    accent: Color,
    pill: bool,
}

#[derive(Clone, Copy, Debug)]
struct CalendarItemStyle {
    color: Color,
    pill: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_color() {
        assert_eq!(theme::parse_hex_color("#1e66f5"), Some(THEME.primary));
    }

    #[test]
    fn visible_start_moves_by_whole_weeks() {
        let today = Local::now().date_naive();
        let start = visible_start_for_offset(0);

        assert_eq!(start.weekday().num_days_from_monday(), 0);
        assert!(start <= today);
        assert_eq!(visible_start_for_offset(2) - start, Duration::days(14));
        assert_eq!(start - visible_start_for_offset(-2), Duration::days(14));
    }

    #[test]
    fn header_month_label_uses_dutch_name() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();

        assert_eq!(header_month_label(date), "Juli 2026");
    }
}
