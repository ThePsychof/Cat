use std::path::{Path, PathBuf};

use cat_core::{
    RepositoryRecord, discover_repositories, fetch_repository, get_commit_log, get_current_branch,
    get_file_status, list_branches, open_in_editor, pull_repository, push_repository,
};

pub fn app_name() -> &'static str {
    "Cat"
}

pub fn drive_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Default)]
struct CatWindow {
    drive_root: PathBuf,
    repositories: Vec<RepositoryRecord>,
    selected: usize,
}

impl CatWindow {
    fn new() -> Self {
        let mut window = Self {
            drive_root: drive_root(),
            ..Self::default()
        };
        window.refresh();
        window
    }

    fn refresh(&mut self) {
        let root = self.drive_root.join("repositories");
        self.repositories = discover_repositories(&root).unwrap_or_default();
        if self.selected >= self.repositories.len() {
            self.selected = 0;
        }
    }

    fn selected_repo(&self) -> Option<&RepositoryRecord> {
        self.repositories.get(self.selected)
    }

    fn snapshot(&self) -> String {
        let Some(repo) = self.selected_repo() else {
            return format!(
                "CAT\r\n\r\nDrive: {}\r\n\r\nNo repositories found.\r\nPlace Git repositories in the drive's repositories folder.",
                self.drive_root.display()
            );
        };
        let path = Path::new(&repo.local_path);
        let branch = get_current_branch(path).unwrap_or_else(|_| "unknown".into());
        let mut text = format!(
            "CAT  |  {}\r\n\r\nDrive: {}\r\nRepository: {}\r\nRemote: {}\r\nState: {:?}\r\nBranch: {}\r\n\r\nBranches\r\n",
            repo.name,
            self.drive_root.display(),
            repo.name,
            repo.remote_url,
            repo.sync_state,
            branch
        );
        for item in list_branches(path).unwrap_or_default() {
            text.push_str(&format!(
                "  {}{}\r\n",
                if item.is_current { "* " } else { "  " },
                item.name
            ));
        }
        text.push_str("\r\nRecent commits\r\n");
        for commit in get_commit_log(path, 8).unwrap_or_default() {
            text.push_str(&format!(
                "  {}  {}  {}\r\n",
                &commit.sha[..7.min(commit.sha.len())],
                commit.author,
                commit.message
            ));
        }
        text.push_str("\r\nChanged files\r\n");
        let changes = get_file_status(path).unwrap_or_default();
        if changes.is_empty() {
            text.push_str("  Working tree clean\r\n");
        }
        for change in changes {
            text.push_str(&format!("  {}  {}\r\n", change.status, change.path));
        }
        text
    }

    fn run_action(&mut self, action: Action) -> String {
        if matches!(action, Action::Refresh) {
            self.refresh();
            return "Refresh completed".into();
        }
        let Some(repo) = self.selected_repo().cloned() else {
            return "No repository selected".into();
        };
        let path = Path::new(&repo.local_path);
        let branch = get_current_branch(path).unwrap_or_else(|_| "main".into());
        let result = match action {
            Action::Fetch => fetch_repository(path, "origin"),
            Action::Pull => pull_repository(path, "origin", &branch),
            Action::Push => push_repository(path, "origin", &branch),
            Action::Sync => fetch_repository(path, "origin")
                .and_then(|_| pull_repository(path, "origin", &branch))
                .and_then(|_| push_repository(path, "origin", &branch)),
            Action::Open => open_in_editor(path, "code"),
            Action::Refresh => Ok(()),
        };
        match result {
            Ok(()) => format!("{} completed", action.label()),
            Err(error) => error,
        }
    }
}

enum Action {
    Refresh,
    Fetch,
    Pull,
    Push,
    Sync,
    Open,
}
impl Action {
    fn label(&self) -> &'static str {
        match self {
            Self::Refresh => "Refresh",
            Self::Fetch => "Fetch",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::Sync => "Sync",
            Self::Open => "Open",
        }
    }
}

#[cfg(windows)]
mod windows_gui {
    use super::{Action, CatWindow};
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    type Handle = *mut std::ffi::c_void;
    type Long = isize;
    type Word = u16;
    type Dword = u32;
    type Uint = u32;
    type Wparam = usize;
    type Lparam = isize;
    type Lresult = isize;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    struct Msg {
        hwnd: Handle,
        message: Uint,
        w_param: Wparam,
        l_param: Lparam,
        time: Dword,
        point: Point,
    }
    #[repr(C)]
    struct WndClass {
        style: Uint,
        wnd_proc: Option<unsafe extern "system" fn(Handle, Uint, Wparam, Lparam) -> Lresult>,
        cls_extra: i32,
        wnd_extra: i32,
        instance: Handle,
        icon: Handle,
        cursor: Handle,
        background: Handle,
        menu_name: *const Word,
        class_name: *const Word,
    }
    #[repr(C)]
    struct CreateStruct {
        create_params: *mut std::ffi::c_void,
        instance: Handle,
        menu: Handle,
        parent: Handle,
        cy: i32,
        cx: i32,
        y: i32,
        x: i32,
        style: Long,
        name: *const Word,
        class_name: *const Word,
        ex_style: Dword,
    }

    const WM_NCCREATE: Uint = 0x0081;
    const WM_COMMAND: Uint = 0x0111;
    const WM_DESTROY: Uint = 0x0002;
    const GWLP_USERDATA: i32 = -21;
    const WS_VISIBLE: Dword = 0x10000000;
    const WS_CHILD: Dword = 0x40000000;
    const WS_BORDER: Dword = 0x00800000;
    const ES_MULTILINE: Dword = 0x0004;
    const ES_READONLY: Dword = 0x0800;
    const IDC_TEXT: usize = 100;
    const IDC_REFRESH: usize = 101;
    const IDC_FETCH: usize = 102;
    const IDC_PULL: usize = 103;
    const IDC_PUSH: usize = 104;
    const IDC_SYNC: usize = 105;
    const IDC_OPEN: usize = 106;

    fn wide(value: &str) -> Vec<Word> {
        OsStr::new(value).encode_wide().chain(once(0)).collect()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe extern "system" fn window_proc(
        hwnd: Handle,
        message: Uint,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult {
        if message == WM_NCCREATE {
            let create = &*(l_param as *const CreateStruct);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.create_params as Long);
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut CatWindow;
        if !state.is_null() && message == WM_COMMAND {
            let action = match w_param & 0xffff {
                IDC_REFRESH => Some(Action::Refresh),
                IDC_FETCH => Some(Action::Fetch),
                IDC_PULL => Some(Action::Pull),
                IDC_PUSH => Some(Action::Push),
                IDC_SYNC => Some(Action::Sync),
                IDC_OPEN => Some(Action::Open),
                _ => None,
            };
            if let Some(action) = action {
                let result = (*state).run_action(action);
                let text = wide(&(*state).snapshot());
                SetWindowTextW(GetDlgItem(hwnd, IDC_TEXT as i32), text.as_ptr());
                let message = wide(&result);
                MessageBoxW(hwnd, message.as_ptr(), wide("Cat").as_ptr(), 0);
            }
        }
        if message == WM_DESTROY {
            PostQuitMessage(0);
        }
        DefWindowProcW(hwnd, message, w_param, l_param)
    }

    pub fn run() -> Result<(), String> {
        unsafe {
            let state = Box::into_raw(Box::new(CatWindow::new()));
            let instance = GetModuleHandleW(null_mut());
            let class = wide("CatNativeWindow");
            let window_class = WndClass {
                style: 0,
                wnd_proc: Some(window_proc),
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: null_mut(),
                cursor: null_mut(),
                background: 6 as Handle,
                menu_name: null_mut(),
                class_name: class.as_ptr(),
            };
            if RegisterClassW(&window_class) == 0 {
                return Err("Could not register Cat window".into());
            }
            let title = wide("Cat - portable repository vault");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_VISIBLE,
                100,
                100,
                1040,
                720,
                null_mut(),
                null_mut(),
                instance,
                state as *mut _,
            );
            if hwnd.is_null() {
                return Err("Could not create Cat window".into());
            }
            let text = wide(&(*state).snapshot());
            CreateWindowExW(
                0,
                wide("EDIT").as_ptr(),
                text.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | ES_MULTILINE | ES_READONLY,
                12,
                52,
                1000,
                580,
                hwnd,
                IDC_TEXT as Handle,
                instance,
                null_mut(),
            );
            for (id, label, x) in [
                (IDC_REFRESH, "Refresh", 12),
                (IDC_FETCH, "Fetch", 105),
                (IDC_PULL, "Pull", 198),
                (IDC_PUSH, "Push", 291),
                (IDC_SYNC, "Sync", 384),
                (IDC_OPEN, "Open in VS Code", 477),
            ] {
                CreateWindowExW(
                    0,
                    wide("BUTTON").as_ptr(),
                    wide(label).as_ptr(),
                    WS_CHILD | WS_VISIBLE,
                    x,
                    12,
                    90,
                    28,
                    hwnd,
                    id as Handle,
                    instance,
                    null_mut(),
                );
            }
            let mut message = Msg {
                hwnd: null_mut(),
                message: 0,
                w_param: 0,
                l_param: 0,
                time: 0,
                point: Point { x: 0, y: 0 },
            };
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            drop(Box::from_raw(state));
            Ok(())
        }
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(class: *const WndClass) -> u16;
        fn CreateWindowExW(
            ex: Dword,
            class: *const Word,
            name: *const Word,
            style: Dword,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: Handle,
            menu: Handle,
            instance: Handle,
            params: *mut std::ffi::c_void,
        ) -> Handle;
        fn DefWindowProcW(hwnd: Handle, message: Uint, w: Wparam, l_param: Lparam) -> Lresult;
        fn GetMessageW(message: *mut Msg, hwnd: Handle, min: Uint, max: Uint) -> i32;
        fn TranslateMessage(message: *const Msg) -> i32;
        fn DispatchMessageW(message: *const Msg) -> Lresult;
        fn PostQuitMessage(code: i32);
        fn SetWindowLongPtrW(hwnd: Handle, index: i32, value: Long) -> Long;
        fn GetWindowLongPtrW(hwnd: Handle, index: i32) -> Long;
        fn GetDlgItem(hwnd: Handle, id: i32) -> Handle;
        fn SetWindowTextW(hwnd: Handle, text: *const Word) -> i32;
        fn MessageBoxW(hwnd: Handle, text: *const Word, caption: *const Word, kind: Uint) -> i32;
        fn GetModuleHandleW(name: *const Word) -> Handle;
    }
}

#[cfg(windows)]
pub fn run() -> Result<(), String> {
    windows_gui::run()
}
#[cfg(not(windows))]
pub fn run() -> Result<(), String> {
    Err("Cat's native GUI is currently implemented for Windows".into())
}

#[cfg(test)]
mod tests {
    use super::{app_name, drive_root};
    #[test]
    fn identity_is_cat() {
        assert_eq!(app_name(), "Cat");
    }
    #[test]
    fn has_drive_root() {
        assert!(!drive_root().as_os_str().is_empty());
    }
}
