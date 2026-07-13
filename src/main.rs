mod components;
mod calendar;
mod icons;
mod layout;
mod node;
mod renderer;
mod theme;

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, Weekday};
use argh::FromArgs;
use std::error::Error;
use taffy::{FlexDirection, NodeId};

use crate::layout::AppLayout;
use crate::components::{div, pill, text};
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
        // .block()
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
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli: Cli = argh::from_env();

    match cli.command {
        Some(Command::Sync(args)) => crate::calendar::sync(args.profile.as_deref(), args.calendar.as_deref())?,
        None => launch_app(),
    }

    Ok(())
}

fn launch_app() {
    let mut layout = AppLayout::new();

    let (_header, content, _footer) = build_layout(&mut layout);

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
            kid.set_layout(|l| {
                l.flex_direction = FlexDirection::Column;
            });

            let date = dates.next().expect("Cannot pop a date");

            if today.date_naive().eq(&date) {
                println!("TODAY IS THE DAY {}", date);
            }

            let label = format!("calendar-cell_{}", date).to_owned();
            kid.add_child(if today.date_naive().eq(&date) {
                pill(format!("{}", date.day()))
                    .background(THEME.warning)
                    .name(node::NodeName::other(label))
            } else {
                text(format!("{}", date.day()))
                    .py(5.)
                    .name(node::NodeName::other(label))
            });
        })
        .build(&mut layout);

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
