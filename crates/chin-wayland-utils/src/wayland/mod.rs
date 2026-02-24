use std::borrow::Cow;

use niri::NiriCompositor;
use niri::NiriWindowWrapper;
use niri_ipc::Output as NiriOutput;
use niri_ipc::Workspace as NiriWorkspace;

use chin_tools::AResult;
use chin_tools::EResult;

pub mod niri;

pub type WLWorkspaceId = u64;
pub type WLWindowId = u64;
pub type WLMonitorId = String;

pub type WLCompositor = NiriCompositor;
pub type WLOutput = NiriOutput;
pub type WLWindow = NiriWindowWrapper;
pub type WLWorkspace = NiriWorkspace;

pub trait WLCompositorBehavier {
    fn new() -> AResult<Self>
    where
        Self: Sized;

    fn fetch_all_windows(&self) -> AResult<Vec<WLWindow>>;
    fn fetch_focused_window(&self) -> AResult<WLWindow>;

    fn fetch_all_workspaces(&self) -> AResult<Vec<WLWorkspace>>;
    fn fetch_focused_workspace(&self) -> AResult<WLWorkspace>;

    fn fetch_all_outputs(&self) -> AResult<Vec<WLOutput>>;
}

pub trait WLWindowBehaiver {
    fn focus(&self) -> EResult;
    fn get_title(&self) -> Option<&str>;
    fn get_app_id(&self) -> Option<&str>;
    fn get_id(&self) -> WLWindowId;
    fn is_focused(&self) -> bool;
    fn is_floating(&self) -> bool;
    fn is_urgent(&self) -> bool;
    fn get_workspace_id(&self) -> Option<WLWorkspaceId>;
    fn get_x(&self) -> i64;
    fn get_y(&self) -> i64;
}

pub trait WLWorkspaceBehaiver {
    fn is_active(&self) -> bool;
    fn is_focused(&self) -> bool;
    fn focus(&self) -> EResult;
    fn get_id(&self) -> &WLWorkspaceId;
    fn get_name<'a>(&'a self) -> Cow<'a, str>;
    fn get_monitor_id(&self) -> Option<&WLMonitorId>;
}

pub trait WLOutputBehaiver {
    fn focus(&self) -> EResult;
    fn get_name(&self) -> &str;
}

#[derive(Clone, Debug)]
pub enum WLEvent {
    WorkspaceDelete(WLWorkspaceId),
    WorkspaceOverwrite(WLWorkspace),
    WindowDelete(WLWindowId),
    WindowOverwrite(WLWindow),
    MonitorDelete(WLMonitorId),
    MonitorOverwrite(WLOutput),
}

trait InnerEquals {
    fn equal(&self, o: &Self) -> bool;
}
