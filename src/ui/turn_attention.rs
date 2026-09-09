//! Local presentation only. Never changes a turn or a server deadline.
use super::multiway_review::MultiwayReviewView;

#[derive(Default)]
pub struct TurnAttention {
    active: bool,
    #[cfg(windows)]
    window: Option<windows::RaisedWindow>,
}

impl TurnAttention {
    pub fn update(&mut self, _view: &MultiwayReviewView, can_act: bool, connected: bool) {
        self.set_active(connected && can_act);
    }

    fn set_active(&mut self, active: bool) {
        // Request focus once when a decision begins, not on every render frame.
        if active == self.active {
            return;
        }
        self.active = active;
        #[cfg(windows)]
        {
            self.window = if active {
                windows::RaisedWindow::acquire()
            } else {
                None // Drop restores the original topmost state.
            };
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use windows_sys::Win32::{
        Foundation::HWND,
        System::{
            Console::GetConsoleWindow,
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::{
            Input::KeyboardAndMouse::{SetActiveWindow, SetFocus},
            WindowsAndMessaging::*,
        },
    };

    static ACTIVATING: AtomicBool = AtomicBool::new(false);

    struct InputAttachment(u32, u32);
    impl InputAttachment {
        fn attach(from: u32, to: u32) -> Option<Self> {
            // SAFETY: IDs are queried from live windows/current thread.
            (from != to && to != 0 && unsafe { AttachThreadInput(from, to, 1) } != 0)
                .then_some(Self(from, to))
        }
    }
    impl Drop for InputAttachment {
        fn drop(&mut self) {
            // SAFETY: Detach only an attachment successfully made by this guard.
            unsafe {
                AttachThreadInput(self.0, self.1, 0);
            }
        }
    }

    fn focus_belongs_to(root: HWND, focus: HWND) -> bool {
        !focus.is_null() && (focus == root || unsafe { IsChild(root, focus) } != 0)
    }

    fn activate_keyboard(hwnd: HWND, process: u32, cancelled: &AtomicBool) -> bool {
        // SAFETY: Validate the host, create this worker's message queue, and
        // temporarily share input queues to set focus on the terminal control.
        // No keys/clicks are synthesized and no system preferences are changed.
        unsafe {
            let mut owner = 0;
            let target_thread = GetWindowThreadProcessId(hwnd, &mut owner);
            if target_thread == 0 || owner != process || cancelled.load(Ordering::Acquire) {
                return false;
            }
            let mut message = MSG::default();
            PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            let input = if GetGUIThreadInfo(target_thread, &mut info) != 0
                && focus_belongs_to(hwnd, info.hwndFocus)
            {
                info.hwndFocus
            } else {
                hwnd
            };
            let current = GetCurrentThreadId();
            let foreground_thread =
                GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let _foreground = InputAttachment::attach(current, foreground_thread);
            let _target = if target_thread != foreground_thread {
                InputAttachment::attach(current, target_thread)
            } else {
                None
            };
            if cancelled.load(Ordering::Acquire) {
                return false;
            }
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
            SetActiveWindow(hwnd);
            SetFocus(input);
            // Success means keyboard focus in this host, not merely topmost.
            GetForegroundWindow() == hwnd
                && GetGUIThreadInfo(target_thread, &mut info) != 0
                && focus_belongs_to(hwnd, info.hwndFocus)
        }
    }

    fn request_keyboard(hwnd: HWND, process: u32, cancelled: Arc<AtomicBool>) {
        // Input-queue attachment can block in another GUI's message handler.
        // Never block the game loop or accumulate workers on subsequent turns.
        if ACTIVATING.swap(true, Ordering::AcqRel) {
            return;
        }
        let handle = hwnd as usize;
        if std::thread::Builder::new()
            .name("turn-keyboard-focus".into())
            .spawn(move || {
                struct ReleaseBusy;
                impl Drop for ReleaseBusy {
                    fn drop(&mut self) {
                        ACTIVATING.store(false, Ordering::Release);
                    }
                }
                let _busy = ReleaseBusy;
                let hwnd = handle as HWND;
                // Allow an asynchronous minimized-window restore to complete.
                for _ in 0..10 {
                    if cancelled.load(Ordering::Acquire) {
                        return;
                    }
                    if unsafe { IsIconic(hwnd) } == 0 {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                if !activate_keyboard(hwnd, process, &cancelled)
                    && !cancelled.load(Ordering::Acquire)
                {
                    unsafe {
                        FlashWindowEx(&FLASHWINFO {
                            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                            hwnd,
                            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
                            uCount: 3,
                            dwTimeout: 0,
                        });
                    }
                }
            })
            .is_err()
        {
            ACTIVATING.store(false, Ordering::Release);
        }
    }

    pub(super) struct RaisedWindow {
        hwnd: HWND,
        process: u32,
        raised: bool,
        cancelled: Arc<AtomicBool>,
    }

    impl RaisedWindow {
        pub(super) fn acquire() -> Option<Self> {
            // SAFETY: Win32 supplies the handles. We only target the visible
            // root owner of this process's attached console, never an arbitrary
            // foreground window or a title-matched unrelated application.
            unsafe {
                let console = GetConsoleWindow();
                if console.is_null() {
                    return None;
                }
                let root = GetAncestor(console, GA_ROOTOWNER);
                let hwnd = if !root.is_null() && IsWindowVisible(root) != 0 {
                    root
                } else if IsWindowVisible(console) != 0 {
                    console
                } else {
                    // A headless/remote ConPTY has no local window to activate.
                    return None;
                };
                let mut process = 0;
                GetWindowThreadProcessId(hwnd, &mut process);
                let was_topmost =
                    GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST as isize != 0;
                if IsIconic(hwnd) != 0 {
                    ShowWindowAsync(hwnd, SW_RESTORE);
                }
                let raised = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                ) != 0
                    && !was_topmost;
                let cancelled = Arc::new(AtomicBool::new(false));
                request_keyboard(hwnd, process, cancelled.clone());
                Some(Self {
                    hwnd,
                    process,
                    raised,
                    cancelled,
                })
            }
        }
    }

    impl Drop for RaisedWindow {
        fn drop(&mut self) {
            self.cancelled.store(true, Ordering::Release);
            // SAFETY: Revalidate ownership in case the terminal closed. Restore
            // stacking without activating it or taking focus away from the user.
            unsafe {
                let mut process = 0;
                if GetWindowThreadProcessId(self.hwnd, &mut process) == 0 || process != self.process
                {
                    return;
                }
                if self.raised {
                    SetWindowPos(
                        self.hwnd,
                        HWND_NOTOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                    );
                }
                FlashWindowEx(&FLASHWINFO {
                    cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                    hwnd: self.hwnd,
                    dwFlags: FLASHW_STOP,
                    uCount: 0,
                    dwTimeout: 0,
                });
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        #[ignore = "explicit desktop check: briefly opens two native test windows"]
        fn desktop_activation_restores_keyboard_focus_instead_of_only_raising() {
            // Two independent GUI queues reproduce a console behind another
            // application. No mouse click or synthesized keystroke is used.
            unsafe {
                let target = CreateWindowExW(
                    0,
                    windows_sys::core::w!("STATIC"),
                    windows_sys::core::w!("Poker keyboard-focus verification"),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    80,
                    80,
                    380,
                    160,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                );
                assert!(!target.is_null());
                let input = CreateWindowExW(
                    0,
                    windows_sys::core::w!("EDIT"),
                    windows_sys::core::w!("Keyboard focus target"),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    10,
                    10,
                    320,
                    30,
                    target,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                );
                assert!(!input.is_null());
                SetForegroundWindow(target);
                SetFocus(input);
                let (send, receive) = std::sync::mpsc::channel();
                let distractor = std::thread::spawn(move || {
                    let hwnd = CreateWindowExW(
                        0,
                        windows_sys::core::w!("STATIC"),
                        windows_sys::core::w!("Temporary other-window focus"),
                        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                        480,
                        80,
                        340,
                        140,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null(),
                    );
                    SetForegroundWindow(hwnd);
                    SetFocus(hwnd);
                    send.send(hwnd as usize).unwrap();
                    let mut message = MSG::default();
                    while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                        if message.message == WM_APP {
                            break;
                        }
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    DestroyWindow(hwnd);
                });
                let other = receive.recv().unwrap() as HWND;
                let mut message = MSG::default();
                let pump = |message: &mut MSG| {
                    while PeekMessageW(message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(message);
                        DispatchMessageW(message);
                    }
                };
                let started = std::time::Instant::now();
                while GetForegroundWindow() != other && started.elapsed().as_secs() < 2 {
                    pump(&mut message);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                let established_other = GetForegroundWindow() == other;
                let mut process = 0;
                let target_thread = GetWindowThreadProcessId(target, &mut process);
                SetWindowPos(
                    target,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                );
                let cancelled = Arc::new(AtomicBool::new(false));
                request_keyboard(target, process, cancelled.clone());
                let started = std::time::Instant::now();
                let mut info = GUITHREADINFO {
                    cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                    ..Default::default()
                };
                let mut focused = false;
                while started.elapsed().as_secs() < 3 {
                    pump(&mut message);
                    focused = GetForegroundWindow() == target
                        && GetGUIThreadInfo(target_thread, &mut info) != 0
                        && info.hwndFocus == input
                        && !ACTIVATING.load(Ordering::Acquire);
                    if focused {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                cancelled.store(true, Ordering::Release);
                PostThreadMessageW(
                    GetWindowThreadProcessId(other, std::ptr::null_mut()),
                    WM_APP,
                    0,
                    0,
                );
                distractor.join().unwrap();
                DestroyWindow(target);
                assert!(
                    established_other,
                    "test must begin with a different window foreground"
                );
                assert!(
                    focused,
                    "host must own foreground AND its input control must own keyboard focus"
                );
            }
        }

        #[test]
        fn ending_attention_restores_only_topmost_state_owned_by_the_game() {
            // A private, hidden native window exercises actual Win32 cleanup
            // without stealing focus or touching the user's terminal.
            unsafe {
                for raised in [true, false] {
                    let hwnd = CreateWindowExW(
                        WS_EX_TOPMOST,
                        windows_sys::core::w!("STATIC"),
                        windows_sys::core::w!("turn-attention-test"),
                        WS_POPUP,
                        0,
                        0,
                        10,
                        10,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null(),
                    );
                    assert!(!hwnd.is_null());
                    let mut process = 0;
                    GetWindowThreadProcessId(hwnd, &mut process);
                    drop(RaisedWindow {
                        hwnd,
                        process,
                        raised,
                        cancelled: Arc::new(AtomicBool::new(false)),
                    });
                    let topmost =
                        GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST as isize != 0;
                    DestroyWindow(hwnd);
                    assert_eq!(topmost, !raised);
                }
            }
        }
    }
}
