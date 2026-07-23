use std::sync::mpsc::Receiver;

use chrono::{DateTime, Duration, Utc};
use taffy::{Dimension, Size};

use crate::{
    AppLayout,
    calendar::{CalendarChange, SyncStatus},
    event::AppEvent,
    layout::LayoutAction,
    node::NodeName,
    theme::THEME,
};

pub(crate) struct App {
    layout: AppLayout,
    sync_rx: Receiver<SyncStatus>,
    sync_footer: SyncFooterState,
    week_offset: i64,
    last_navigation_at: Option<DateTime<Utc>>,
}

impl App {
    pub(crate) fn new(sync_rx: Receiver<SyncStatus>) -> Self {
        Self {
            layout: crate::build_app_layout(0),
            sync_rx,
            sync_footer: SyncFooterState::new(),
            week_offset: 0,
            last_navigation_at: None,
        }
    }

    pub(crate) fn poll_sync(&mut self) -> bool {
        let mut dirty = false;

        while let Ok(status) = self.sync_rx.try_recv() {
            let rebuild = self.handle_sync_status(status);
            if rebuild {
                self.rebuild_layout();
            }
            dirty = true;
        }

        dirty
    }

    pub(crate) fn handle_event(&mut self, event: AppEvent) {
        self.layout.handle_event(event);
        self.handle_layout_action();
        if matches!(event, AppEvent::Tick) {
            self.reset_after_idle();
            self.apply_sync_footer();
        }
    }

    pub(crate) fn render_layout(&mut self, size: Size<Dimension>) {
        self.layout.render_layout(size);
    }

    pub(crate) fn draw(&mut self, buffer: &mut [u32], window_width: u32, window_height: u32) {
        self.layout.draw(buffer, window_width, window_height);
    }

    fn handle_sync_status(&mut self, status: SyncStatus) -> bool {
        let mut rebuild_calendar = false;

        match status {
            SyncStatus::Syncing { calendar } => {
                self.sync_footer.syncing = true;
                self.sync_footer.latest_changes = format!("syncing {calendar}");
            }
            SyncStatus::Synced {
                synced_at,
                next_sync_at,
                changes,
            } => {
                self.sync_footer.syncing = false;
                self.sync_footer.next_sync_at = Some(next_sync_at);
                if !changes.is_empty() {
                    rebuild_calendar = true;
                    self.sync_footer.latest_changes = format!(
                        "updated {}: {}",
                        synced_at.with_timezone(&chrono::Local).format("%H:%M"),
                        format_changes(&changes)
                    );
                } else {
                    self.sync_footer.latest_changes = format!(
                        "updated {}: no changes",
                        synced_at.with_timezone(&chrono::Local).format("%H:%M")
                    );
                }
            }
            SyncStatus::Failed {
                calendar,
                error,
                next_sync_at,
            } => {
                self.sync_footer.syncing = false;
                self.sync_footer.next_sync_at = Some(next_sync_at);
                self.sync_footer.latest_changes = format!("sync failed {calendar}: {error}");
            }
        }

        self.apply_sync_footer();
        rebuild_calendar
    }

    fn apply_sync_footer(&mut self) {
        let color = if self.sync_footer.syncing {
            THEME.success
        } else {
            THEME.text
        };
        let status = if self.sync_footer.syncing {
            "sync in progress".to_owned()
        } else if let Some(next_sync_at) = self.sync_footer.next_sync_at {
            format!("next sync in {}", format_countdown(next_sync_at))
        } else {
            "next sync pending".to_owned()
        };

        self.layout
            .set_text_color_by_name(NodeName::other(crate::SYNC_ICON_NODE), color);
        self.layout.set_icon_by_name(
            NodeName::other(crate::SYNC_ICON_NODE),
            if self.sync_footer.syncing {
                "incomplete"
            } else {
                "circle-fill"
            },
        );
        self.layout
            .set_text_color_by_name(NodeName::other(crate::SYNC_STATUS_NODE), color);
        self.layout
            .set_text_by_name(NodeName::other(crate::SYNC_STATUS_NODE), status);
        self.layout.set_text_by_name(
            NodeName::other(crate::SYNC_CHANGES_NODE),
            self.sync_footer.latest_changes.clone(),
        );
    }

    fn rebuild_layout(&mut self) {
        self.layout = crate::build_app_layout(self.week_offset);
        self.apply_sync_footer();
    }

    fn handle_layout_action(&mut self) {
        let Some(action) = self.layout.take_action() else {
            return;
        };

        match action {
            LayoutAction::ShiftWeeks(delta) => {
                self.week_offset += delta;
                self.last_navigation_at = Some(Utc::now());
                self.rebuild_layout();
            }
            LayoutAction::ResetWeeks => {
                if self.week_offset == 0 {
                    return;
                }

                self.week_offset = 0;
                self.last_navigation_at = None;
                self.rebuild_layout();
            }
        }
    }

    fn reset_after_idle(&mut self) {
        let Some(last_navigation_at) = self.last_navigation_at else {
            return;
        };

        if self.week_offset == 0 || Utc::now() - last_navigation_at < Duration::minutes(2) {
            return;
        }

        self.week_offset = 0;
        self.last_navigation_at = None;
        self.rebuild_layout();
    }
}

struct SyncFooterState {
    syncing: bool,
    next_sync_at: Option<DateTime<Utc>>,
    latest_changes: String,
}

impl SyncFooterState {
    fn new() -> Self {
        Self {
            syncing: false,
            next_sync_at: None,
            latest_changes: "no sync changes yet".to_owned(),
        }
    }
}

fn format_countdown(next_sync_at: DateTime<Utc>) -> String {
    let seconds = (next_sync_at - Utc::now()).num_seconds().max(0);
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn format_changes(changes: &[CalendarChange]) -> String {
    changes
        .iter()
        .take(3)
        .map(|change| match change {
            CalendarChange::Created { title } => format!("created \"{title}\""),
            CalendarChange::Updated { title } => format!("updated \"{title}\""),
            CalendarChange::Removed { title } => format!("removed \"{title}\""),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
