use chin_tools::AResult;
use chin_tools::EResult;
use chin_tools::aanyhow;
pub use niri_ipc::Output as NiriOutput;
pub use niri_ipc::Window as NiriWindow;
pub use niri_ipc::Workspace as NiriWorkspace;

use std::borrow::Cow;
use std::{collections::HashMap, fmt::Debug, process::Command};

use serde::de::DeserializeOwned;

use crate::wayland::WLCompositorBehavier;
use crate::wayland::WLMonitorId;
use crate::wayland::WLOutput;
use crate::wayland::WLOutputBehaiver;
use crate::wayland::WLWindowBehaiver;
use crate::wayland::WLWorkspaceBehaiver;
use crate::wayland::WLWorkspaceId;

use super::NiriCompositor;
use super::NiriWindowWrapper;

#[macro_export]
macro_rules! niri_msg {
    () => {
        Command::new("niri").arg("msg").arg("-j")
    };
}

#[macro_export]
macro_rules! niri_action {
    () => {
        niri_msg!().arg("action")
    };
}

fn json_output<T>(cmd: &mut Command) -> AResult<T>
where
    T: DeserializeOwned + Debug,
{
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(aanyhow!("command exited with {:?}", output.status.code()));
    }
    let stdout = String::from_utf8_lossy(output.stdout.as_slice());

    Ok(serde_json::from_str(&stdout)?)
}

impl WLWindowBehaiver for NiriWindow {
    fn focus(&self) -> EResult {
        json_output(niri_action!().args(["focus-window", "--id", &format!("{}", self.id)]))
    }

    fn get_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn get_app_id(&self) -> Option<&str> {
        self.app_id.as_deref()
    }

    fn get_id(&self) -> crate::wayland::WLWindowId {
        self.id
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }

    fn get_workspace_id(&self) -> Option<crate::wayland::WLWorkspaceId> {
        self.workspace_id
    }

    fn get_x(&self) -> i64 {
        self.layout
            .pos_in_scrolling_layout
            .as_ref()
            .map_or(999999, |v| v.0 as i64)
    }

    fn get_y(&self) -> i64 {
        self.layout
            .pos_in_scrolling_layout
            .as_ref()
            .map_or(999999, |v| v.1 as i64)
    }

    fn is_floating(&self) -> bool {
        self.is_floating
    }

    fn is_urgent(&self) -> bool {
        self.is_urgent
    }
}

impl WLWorkspaceBehaiver for NiriWorkspace {
    fn is_active(&self) -> bool {
        self.is_active
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }

    fn focus(&self) -> EResult {
        json_output(niri_action!().args(["focus-workspace", &format!("{}", self.idx)]))
    }

    fn get_id(&self) -> &WLWorkspaceId {
        &self.id
    }

    fn get_name<'a>(&'a self) -> Cow<'a, str> {
        match &self.name {
            Some(name) => Cow::Borrowed(name),
            None => self.idx.to_string().into(),
        }
    }

    fn get_monitor_id(&self) -> Option<&WLMonitorId> {
        self.output.as_ref()
    }
}

impl WLOutputBehaiver for NiriOutput {
    fn focus(&self) -> EResult {
        Ok(())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

impl WLCompositorBehavier for NiriCompositor {
    fn fetch_all_windows(&self) -> AResult<Vec<NiriWindowWrapper>> {
        json_output(Command::new("niri").arg("msg").arg("--json").arg("windows"))
            .map(|v: Vec<NiriWindow>| v.into_iter().map(|e| e.into()).collect())
    }

    fn fetch_focused_window(&self) -> AResult<NiriWindowWrapper> {
        json_output(niri_msg!().arg("focused-window")).map(|v: NiriWindow| v.into())
    }

    fn fetch_all_workspaces(&self) -> AResult<Vec<NiriWorkspace>> {
        json_output(niri_msg!().arg("workspaces"))
            .map(|e: Vec<NiriWorkspace>| e.into_iter().collect())
    }

    fn fetch_focused_workspace(&self) -> AResult<NiriWorkspace> {
        self.fetch_all_workspaces().and_then(|e| {
            e.into_iter()
                .find(|e| e.is_focused())
                .ok_or(aanyhow!("no focused workspace???"))
        })
    }

    fn fetch_all_outputs(&self) -> AResult<Vec<WLOutput>> {
        let ret: HashMap<String, WLOutput> = json_output(niri_msg!().arg("outputs"))?;
        Ok(ret.into_values().collect())
    }

    fn new() -> AResult<Self>
    where
        Self: Sized,
    {
        let _ = std::env::var("NIRI_SOCKET")?;
        let mut instance = NiriCompositor::default();
        let windows = instance.fetch_all_windows()?;
        let awin = instance.fetch_focused_window().ok();
        let workspaces = instance.fetch_all_workspaces()?;

        instance.windows = windows.into_iter().map(|e| (e.get_id(), e)).collect();
        instance.workspaces = workspaces.into_iter().map(|e| (*e.get_id(), e)).collect();
        instance.focused_winid = awin.map(|e| e.get_id());

        Ok(instance)
    }
}

#[cfg(test)]
mod test {}
