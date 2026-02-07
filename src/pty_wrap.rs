use std::ffi::CString;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};

use crate::state;

// ---------------------------------------------------------------------------
// OscScanner — state machine to detect OSC 0/2 title sequences
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    Normal,
    Escape,      // saw ESC (\x1b)
    OscStart,    // saw ESC ] — captures single-digit param only (sufficient for OSC 0/2)
    OscSemiWait, // got valid param, waiting for ';'
    OscData,     // collecting title bytes until BEL or ST
    StEscape,    // saw ESC inside OscData (possible ST = ESC \)
}

pub struct OscScanner {
    state: ScanState,
    param: u8,
    buf: Vec<u8>,
}

impl OscScanner {
    pub fn new() -> Self {
        Self {
            state: ScanState::Normal,
            param: 0,
            buf: Vec::with_capacity(256),
        }
    }

    /// Feed a single byte. Returns Some(title) when a complete OSC 0/2 is detected.
    pub fn feed(&mut self, byte: u8) -> Option<String> {
        match self.state {
            ScanState::Normal => {
                if byte == 0x1b {
                    self.state = ScanState::Escape;
                }
                None
            }
            ScanState::Escape => {
                if byte == b']' {
                    self.state = ScanState::OscStart;
                } else {
                    self.state = ScanState::Normal;
                }
                None
            }
            ScanState::OscStart => {
                if byte.is_ascii_digit() {
                    self.param = byte - b'0';
                    self.state = ScanState::OscSemiWait;
                } else {
                    self.state = ScanState::Normal;
                }
                None
            }
            ScanState::OscSemiWait => {
                if byte == b';' {
                    if self.param == 0 || self.param == 2 {
                        self.buf.clear();
                        self.state = ScanState::OscData;
                    } else {
                        self.state = ScanState::Normal;
                    }
                } else {
                    self.state = ScanState::Normal;
                }
                None
            }
            ScanState::OscData => {
                if byte == 0x07 {
                    // BEL terminator
                    let title = String::from_utf8_lossy(&self.buf).to_string();
                    self.state = ScanState::Normal;
                    Some(title)
                } else if byte == 0x1b {
                    // Possible start of ST (ESC \)
                    self.state = ScanState::StEscape;
                    None
                } else {
                    if self.buf.len() < 4096 {
                        self.buf.push(byte);
                    }
                    None
                }
            }
            ScanState::StEscape => {
                if byte == b'\\' {
                    // ST terminator (ESC \)
                    let title = String::from_utf8_lossy(&self.buf).to_string();
                    self.state = ScanState::Normal;
                    Some(title)
                } else {
                    // Not ST, treat ESC as data and continue
                    if self.buf.len() < 4096 {
                        self.buf.push(0x1b);
                    }
                    // Re-process current byte in OscData state
                    self.state = ScanState::OscData;
                    self.feed(byte)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TerminalGuard — RAII guard to restore terminal settings
// ---------------------------------------------------------------------------

struct TerminalGuard {
    fd: RawFd,
    original: libc::termios,
}

impl TerminalGuard {
    fn new(fd: RawFd) -> io::Result<Self> {
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { fd, original })
    }

    fn set_raw(&self) -> io::Result<()> {
        let mut raw = self.original;
        unsafe { libc::cfmakeraw(&mut raw) };
        if unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
        }
    }
}

// ---------------------------------------------------------------------------
// PTY helpers
// ---------------------------------------------------------------------------

fn openpty() -> io::Result<(RawFd, RawFd)> {
    let mut master: RawFd = 0;
    let mut slave: RawFd = 0;
    let ret = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    // Set close-on-exec for both fds
    unsafe {
        libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(slave, libc::F_SETFD, libc::FD_CLOEXEC);
    }
    Ok((master, slave))
}

fn get_winsize(fd: RawFd) -> io::Result<libc::winsize> {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(ws)
}

fn set_winsize(fd: RawFd, ws: &libc::winsize) -> io::Result<()> {
    if unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, ws) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Self-pipe for SIGWINCH (using AtomicI32 for signal safety)
// ---------------------------------------------------------------------------

static SIGWINCH_WRITE_FD: AtomicI32 = AtomicI32::new(-1);

extern "C" fn sigwinch_handler(_sig: libc::c_int) {
    let fd = SIGWINCH_WRITE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        unsafe {
            let _ = libc::write(fd, &1u8 as *const u8 as *const libc::c_void, 1);
        }
    }
}

fn setup_sigwinch_pipe() -> io::Result<RawFd> {
    let mut fds: [RawFd; 2] = [0; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }

    // Set non-blocking and close-on-exec for both ends
    for &fd in &fds {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    }

    SIGWINCH_WRITE_FD.store(fds[1], Ordering::Relaxed);

    // Install handler
    let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
    sa.sa_sigaction = sigwinch_handler as libc::sighandler_t;
    unsafe { libc::sigemptyset(&mut sa.sa_mask) };
    sa.sa_flags = libc::SA_RESTART;
    if unsafe { libc::sigaction(libc::SIGWINCH, &sa, std::ptr::null_mut()) } != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(fds[0]) // read end
}

// ---------------------------------------------------------------------------
// State update with debounce
// ---------------------------------------------------------------------------

fn update_claude_status(
    session_name: &str,
    title: &str,
    last_title: &mut String,
    last_update: &mut Instant,
) {
    if title == last_title.as_str() {
        return;
    }
    if last_update.elapsed().as_millis() < 100 {
        return;
    }
    *last_title = title.to_string();
    *last_update = Instant::now();
    let session_name = session_name.to_string();
    let title = title.to_string();
    let _ = state::update(|state| {
        if let Some(s) = state.sessions.iter_mut().find(|s| s.name == session_name) {
            s.claude_status = Some(title.clone());
        }
        Ok(())
    });
}

// ---------------------------------------------------------------------------
// Shell quoting helper
// ---------------------------------------------------------------------------

/// Join command arguments into a single shell-safe string.
/// Each argument is single-quoted to prevent shell interpretation.
fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.is_empty() {
                "''".to_string()
            } else if a.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || b == b'-'
                    || b == b'_'
                    || b == b'.'
                    || b == b'/'
                    || b == b':'
                    || b == b'='
            }) {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// run_wrap — main entry point
// ---------------------------------------------------------------------------

pub fn run_wrap(session_name: &str, command: &[String]) -> Result<i32> {
    if command.is_empty() {
        anyhow::bail!("no command specified");
    }

    let stdin_fd = io::stdin().as_raw_fd();
    let is_tty = unsafe { libc::isatty(stdin_fd) } == 1;

    // Open PTY pair
    let (master, slave) = openpty().context("failed to openpty")?;

    // Copy terminal size to PTY if we're a TTY
    if is_tty {
        if let Ok(ws) = get_winsize(stdin_fd) {
            let _ = set_winsize(master, &ws);
        }
    }

    // Prepare CStrings before fork (allocation is not async-signal-safe).
    // Execute through the user's shell with -ic so that aliases and shell
    // functions (e.g. `claude-dev`) are properly resolved.
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let cmd_str = shell_join(command);
    let c_shell = CString::new(shell.as_str()).context("SHELL contains null byte")?;
    let c_flag = CString::new("-ic").unwrap();
    let c_cmd = CString::new(cmd_str.as_str()).context("command contains null byte")?;
    let c_argv: Vec<*const libc::c_char> = vec![
        c_shell.as_ptr(),
        c_flag.as_ptr(),
        c_cmd.as_ptr(),
        std::ptr::null(),
    ];

    // Fork
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error()).context("fork failed");
    }

    if pid == 0 {
        // === Child process (only async-signal-safe operations) ===
        unsafe {
            libc::setsid();
            libc::close(master);

            // Clear close-on-exec on slave so it survives exec
            libc::fcntl(slave, libc::F_SETFD, 0);

            libc::ioctl(slave, libc::TIOCSCTTY as libc::c_ulong, 0);
            libc::dup2(slave, 0);
            libc::dup2(slave, 1);
            libc::dup2(slave, 2);
            if slave > 2 {
                libc::close(slave);
            }

            libc::execvp(c_shell.as_ptr(), c_argv.as_ptr());

            // exec failed — use only async-signal-safe write
            let msg = b"exec failed\n";
            libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
            libc::_exit(127);
        }
    }

    // === Parent process ===
    unsafe { libc::close(slave) };

    // Set up terminal raw mode
    let _guard = if is_tty {
        let guard = TerminalGuard::new(stdin_fd).context("failed to get terminal attrs")?;
        guard.set_raw().context("failed to set raw mode")?;
        Some(guard)
    } else {
        None
    };

    // Set up SIGWINCH self-pipe
    let sigwinch_read = if is_tty {
        setup_sigwinch_pipe().ok()
    } else {
        None
    };

    // State for debounce
    let mut last_title = String::new();
    let mut last_update = Instant::now();
    let mut scanner = OscScanner::new();

    // Stdin forwarding thread — use dup'd fd to avoid double-ownership of master
    let master_dup = unsafe { libc::dup(master) };
    if master_dup < 0 {
        return Err(io::Error::last_os_error()).context("dup master for stdin thread failed");
    }
    let _stdin_thread = std::thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; 4096];
        // File owns master_dup; will close it on drop
        let mut master_file = unsafe { std::fs::File::from_raw_fd(master_dup) };
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if master_file.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        // master_file drops here, closing master_dup (separate fd from master)
    });

    // Main loop: read from master, scan for OSC, write to stdout
    let mut read_buf = [0u8; 4096];
    let mut stdout = io::stdout();

    loop {
        // Use poll() to multiplex master fd and sigwinch pipe (no fd number limit)
        let mut pollfds: Vec<libc::pollfd> = vec![libc::pollfd {
            fd: master,
            events: libc::POLLIN,
            revents: 0,
        }];
        if let Some(sw_fd) = sigwinch_read {
            pollfds.push(libc::pollfd {
                fd: sw_fd,
                events: libc::POLLIN,
                revents: 0,
            });
        }

        let ret =
            unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, -1) };

        if ret < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }

        // Handle SIGWINCH
        if pollfds.len() > 1 && (pollfds[1].revents & libc::POLLIN) != 0 {
            // Drain the pipe
            let mut drain = [0u8; 64];
            unsafe {
                libc::read(
                    pollfds[1].fd,
                    drain.as_mut_ptr() as *mut libc::c_void,
                    drain.len(),
                );
            }
            // Propagate window size
            if let Ok(ws) = get_winsize(stdin_fd) {
                let _ = set_winsize(master, &ws);
            }
        }

        // Read from master
        if (pollfds[0].revents & (libc::POLLIN | libc::POLLHUP)) != 0 {
            let n = unsafe {
                libc::read(
                    master,
                    read_buf.as_mut_ptr() as *mut libc::c_void,
                    read_buf.len(),
                )
            };
            if n <= 0 {
                break;
            }
            let n = n as usize;

            // Scan for OSC sequences
            for &byte in &read_buf[..n] {
                if let Some(title) = scanner.feed(byte) {
                    update_claude_status(session_name, &title, &mut last_title, &mut last_update);
                }
            }

            // Pass through to stdout
            if stdout.write_all(&read_buf[..n]).is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    }

    // Wait for child
    let mut status: libc::c_int = 0;
    unsafe {
        libc::waitpid(pid, &mut status, 0);
    }

    // Close master fd
    unsafe { libc::close(master) };

    // Clean up sigwinch pipe
    if let Some(sw_fd) = sigwinch_read {
        let write_fd = SIGWINCH_WRITE_FD.swap(-1, Ordering::Relaxed);
        unsafe {
            libc::close(sw_fd);
            if write_fd >= 0 {
                libc::close(write_fd);
            }
        }
    }

    // stdin thread will exit when master_dup is closed or stdin EOF.
    // We call process::exit() after returning, which terminates all threads.

    // Extract exit code
    let exit_code = if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        1
    };

    Ok(exit_code)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osc0_bel_terminator() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b]0;test title\x07";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, Some("test title".to_string()));
    }

    #[test]
    fn test_osc0_st_terminator() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b]0;title\x1b\x5c";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, Some("title".to_string()));
    }

    #[test]
    fn test_osc2_detected() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b]2;window title\x07";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, Some("window title".to_string()));
    }

    #[test]
    fn test_osc3_ignored() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b]3;ignored\x07";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, None);
    }

    #[test]
    fn test_osc1_ignored() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b]1;icon name\x07";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, None);
    }

    #[test]
    fn test_incomplete_sequence() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b]0;incomplete";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, None);
    }

    #[test]
    fn test_normal_text_passthrough() {
        let mut scanner = OscScanner::new();
        let input = b"hello world";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, None);
    }

    #[test]
    fn test_multiple_sequences() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b]0;first\x07some text\x1b]0;second\x07";
        let mut results = Vec::new();
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                results.push(title);
            }
        }
        assert_eq!(results, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn test_osc_with_mixed_content() {
        let mut scanner = OscScanner::new();
        let input = b"output\x1b]0;Claude Code: thinking\x07more output";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, Some("Claude Code: thinking".to_string()));
    }

    #[test]
    fn test_broken_escape_sequence() {
        let mut scanner = OscScanner::new();
        let input = b"\x1b[A\x1b]0;valid\x07";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        assert_eq!(result, Some("valid".to_string()));
    }

    #[test]
    fn test_long_title_truncated() {
        let mut scanner = OscScanner::new();
        let mut input = Vec::new();
        input.extend_from_slice(b"\x1b]0;");
        input.extend(std::iter::repeat(b'A').take(5000));
        input.push(0x07);
        let mut result = None;
        for &byte in &input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        let title = result.unwrap();
        assert_eq!(title.len(), 4096);
    }

    #[test]
    fn test_double_escape_in_osc_data() {
        let mut scanner = OscScanner::new();
        // ESC followed by non-backslash inside OscData should be treated as data
        let input = b"\x1b]0;hello\x1b[world\x07";
        let mut result = None;
        for &byte in input {
            if let Some(title) = scanner.feed(byte) {
                result = Some(title);
            }
        }
        // ESC is pushed as data, then '[' resets to OscData and pushes '[',
        // but actually '[' is re-fed to OscData, which pushes it
        // ESC is pushed, then OscData gets '[' which is a regular byte
        // Note: after StEscape sees '[', it pushes 0x1b into buf, then re-feeds '['
        // which in OscData state pushes '['. Then 'w','o','r','l','d' are pushed too.
        assert_eq!(result, Some("hello\x1b[world".to_string()));
    }
}
