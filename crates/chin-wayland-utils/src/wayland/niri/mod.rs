pub mod event_stream;
pub mod model;

use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};

use model::*;
use niri_ipc::Event;

use super::{
    InnerEquals, WLEvent, WLWindow, WLWindowBehaiver, WLWindowId, WLWorkspace, WLWorkspaceBehaiver,
    WLWorkspaceId,
};

#[derive(Clone, Debug)]
pub struct NiriWindowWrapper(NiriWindow);

#[derive(Default)]
pub struct NiriCompositor {
    inited: bool,
    focused_winid: Option<WLWindowId>,
    workspaces: HashMap<WLWorkspaceId, WLWorkspace>,
    windows: HashMap<WLWorkspaceId, WLWindow>,
}

impl Deref for NiriWindowWrapper {
    type Target = NiriWindow;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for NiriWindowWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<NiriWindow> for NiriWindowWrapper {
    fn from(value: NiriWindow) -> Self {
        Self(value)
    }
}

impl PartialEq for NiriWindowWrapper {
    fn eq(&self, other: &Self) -> bool {
        let s = &self.0;
        let o = &other.0;
        s.equal(o)
    }
}

impl InnerEquals for NiriWindow {
    fn equal(&self, o: &Self) -> bool {
        let s = self;
        s.is_floating == o.is_floating
            && s.is_focused == o.is_focused
            && s.app_id == o.app_id
            && s.id == o.id
            && s.pid == o.pid
            && s.title == o.title
            && s.workspace_id == o.workspace_id
            && s.layout == o.layout
    }
}

impl NiriCompositor {
    fn apply_workspaces(&mut self, wss: Vec<WLWorkspace>) -> Vec<WLEvent> {
        let mut events = vec![];
        for new in wss {
            let old = self.workspaces.get(&new.get_id());
            if old.map(|e| *e != new).unwrap_or(true) {
                events.push(WLEvent::WorkspaceOverwrite(new.clone()));
                self.workspaces.insert(new.get_id(), new);
            }
        }

        events
    }

    fn get_old_focused_workspaces(&self) -> Vec<WLWorkspace> {
        let old_focused: Vec<WLWorkspace> = self
            .workspaces
            .values()
            .filter(|e| e.is_focused)
            .map(|e| WLWorkspace {
                is_focused: false,
                ..e.clone()
            })
            .collect();

        old_focused
    }

    fn change_focused(&mut self, new_win: Option<u64>) -> Vec<WLEvent> {
        let mut events: Vec<_> = vec![];

        let old_focused: Vec<WLWindow> = self
            .windows
            .values()
            .filter(|e| e.is_focused)
            .cloned()
            .collect();

        for old in old_focused {
            if Some(old.id) == new_win {
                continue;
            }

            let win = NiriWindowWrapper(NiriWindow {
                is_focused: false,
                ..old.0
            });
            self.windows.insert(win.get_id(), win.clone());
            events.push(WLEvent::WindowOverwrite(win));
        }
        if let Some(id) = new_win
            && let Some(win) = self.windows.get(&id)
            && !win.is_focused
        {
            let win = NiriWindowWrapper(NiriWindow {
                is_focused: true,
                ..win.0.clone()
            });
            self.windows.insert(id, win.clone());
            events.push(WLEvent::WindowOverwrite(win));
        };

        events
    }

    pub fn handle_event(&mut self, event: Event) -> Option<Vec<WLEvent>> {
        let all_windows: &mut HashMap<WLWindowId, WLWindow> = &mut self.windows;

        log::debug!("niri event {event:?}");

        let mapped = match event {
            Event::WorkspacesChanged { workspaces } => {
                let mut events: Vec<_> = vec![];

                let new_set: HashSet<_> = workspaces.iter().map(|e| e.get_id()).collect();
                for id in self.workspaces.keys() {
                    if !new_set.contains(id) {
                        events.push(WLEvent::WorkspaceDelete(*id));
                    }
                }
                self.workspaces.retain(|e, _| new_set.contains(e));

                events.extend(self.apply_workspaces(workspaces));

                Some(events)
            }
            Event::WorkspaceActivated { id, focused } => {
                let mut wss = vec![];
                if focused {
                    wss.extend(self.get_old_focused_workspaces());
                } else {
                    log::debug!("workspace {id} focused: {focused}");
                }

                if let Some(ws) = self.workspaces.get(&id) {
                    let changed = NiriWorkspace {
                        is_focused: focused,
                        ..ws.clone()
                    };
                    wss.push(changed);
                } else {
                    log::warn!("new focused workspace is not existed {id}");
                }

                Some(self.apply_workspaces(wss))
            }
            Event::WorkspaceActiveWindowChanged {
                workspace_id: _,
                active_window_id: _,
            } => None,
            Event::WindowsChanged { windows } => {
                let mut events: Vec<_> = vec![];

                for window in windows {
                    let window: NiriWindowWrapper = window.into();
                    let old_window = self.windows.get(&window.get_id());
                    if old_window.is_none_or(|e| !e.equal(&window)) {
                        self.windows.insert(window.get_id(), window.clone());
                        events.push(WLEvent::WindowOverwrite(window.clone()));
                    }
                }
                Some(events)
            }
            Event::WindowOpenedOrChanged { window } => {
                let win_id = window.id;
                let focused = window.is_focused;
                let mut events: Vec<WLEvent>;
                let win = window.clone();
                self.windows.insert(window.id, window.into());

                if focused {
                    events = self.change_focused(Some(win_id));
                } else {
                    events = vec![];
                }
                events.push(WLEvent::WindowOverwrite(win.into()));
                Some(events)
            }
            Event::WindowClosed { id } => {
                all_windows.remove(&id);
                Some(vec![WLEvent::WindowDelete(id)])
            }
            Event::WindowFocusChanged { id } => {
                let events = self.change_focused(id);

                Some(events)
            }
            Event::KeyboardLayoutsChanged {
                keyboard_layouts: _,
            } => None,
            Event::WindowLayoutsChanged { changes } => {
                let mut events = vec![];
                for (index, layout) in changes {
                    let Some(window) = self.windows.get_mut(&index) else {
                        continue;
                    };
                    if window.layout != layout {
                        window.layout = layout;
                        let event_win = window.clone();
                        events.push(WLEvent::WindowOverwrite(event_win));
                    }
                }
                if !events.is_empty() {
                    Some(events)
                } else {
                    None
                }
            }
            _ => None,
        };

        if self.inited {
            mapped
        } else {
            let mut result = vec![];
            for w in self.workspaces.values() {
                result.push(WLEvent::WorkspaceOverwrite(w.clone()));
            }
            for w in self.windows.values() {
                result.push(WLEvent::WindowOverwrite(w.clone()));
            }
            if let Some(es) = mapped {
                result.extend(es);
            }

            self.inited = true;

            Some(result)
        }
    }
}
