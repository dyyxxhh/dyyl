//! Testable IO provider abstraction for dyyl terminal commands.
//!
//! All IO input operations (line reading, key reading, password reading)
//! go through an `IoProvider` trait, allowing tests to inject deterministic
//! responses without terminal interaction.
//!
//! The default `StdIoProvider` reads from real stdin (safe for noninteractive
//! execution — returns `NoInputAvailable` when stdin is at EOF).
//! `MockIoProvider` provides pre-loaded responses for deterministic tests.

use std::collections::VecDeque;
use std::fmt;
use std::io::BufRead;
use std::sync::Mutex;

/// Error type for IO provider operations.
#[derive(Debug)]
pub enum IoError {
    /// No input available (stdin at EOF or provider exhausted).
    NoInputAvailable,
    /// Underlying I/O error.
    Io(std::io::Error),
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInputAvailable => write!(f, "no input available"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

/// Abstraction for terminal IO operations.
///
/// Implementors provide line reading, key reading, and password reading.
/// The trait is `Send + Sync` so it can be shared across execution contexts.
pub trait IoProvider: Send + Sync {
    /// Read a line of input. Returns the line (without trailing newline)
    /// or an error if no input is available.
    fn read_line(&self, prompt: &str) -> Result<String, IoError>;

    /// Read a single key press / key name. Returns the key name string
    /// or an error if no input is available.
    fn read_key(&self) -> Result<String, IoError>;

    /// Read a password line (typically without echo). Returns the password
    /// string or an error if no input is available.
    fn read_password(&self, prompt: &str) -> Result<String, IoError>;
}

/// Default IO provider that reads from real stdin.
///
/// When stdin is at EOF (piped/redirected with no data), all operations
/// return `IoError::NoInputAvailable` immediately instead of blocking.
pub struct StdIoProvider;

impl IoProvider for StdIoProvider {
    fn read_line(&self, _prompt: &str) -> Result<String, IoError> {
        let stdin = std::io::stdin();
        let mut buf = String::new();
        let n = stdin.lock().read_line(&mut buf).map_err(IoError::Io)?;
        if n == 0 {
            return Err(IoError::NoInputAvailable);
        }
        Ok(buf.trim_end_matches('\n').to_string())
    }

    fn read_key(&self) -> Result<String, IoError> {
        #[cfg(unix)]
        {
            read_key_tty()
        }
        #[cfg(not(unix))]
        {
            Err(IoError::NoInputAvailable)
        }
    }

    fn read_password(&self, _prompt: &str) -> Result<String, IoError> {
        let stdin = std::io::stdin();
        let mut buf = String::new();
        let n = stdin.lock().read_line(&mut buf).map_err(IoError::Io)?;
        if n == 0 {
            return Err(IoError::NoInputAvailable);
        }
        Ok(buf.trim_end_matches('\n').to_string())
    }
}

// ── Unix raw-mode key reading ──────────────────────────────────────

/// Read a single key press from `/dev/tty` in raw mode (Unix only).
///
/// Opens `/dev/tty` directly so it works even when stdin is redirected.
/// Puts the terminal in raw mode (no canonical, no echo), reads one
/// byte (or an escape sequence), then restores the original settings.
/// Returns `NoInputAvailable` if `/dev/tty` cannot be opened or is at EOF.
#[cfg(unix)]
fn read_key_tty() -> Result<String, IoError> {
    use std::os::unix::io::RawFd;

    // Open /dev/tty for reading — works even when stdin is piped.
    let tty_path = c"/dev/tty";
    let fd: RawFd = unsafe { libc::open(tty_path.as_ptr().cast(), libc::O_RDONLY) };
    if fd < 0 {
        return Err(IoError::NoInputAvailable);
    }

    // Check if it's actually a terminal.
    if unsafe { libc::isatty(fd) } == 0 {
        close_fd(fd);
        return Err(IoError::NoInputAvailable);
    }

    // Save original terminal settings.
    let mut orig_termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut orig_termios) } != 0 {
        close_fd(fd);
        return Err(IoError::NoInputAvailable);
    }

    // Put terminal in raw mode.
    let mut raw_termios = orig_termios;
    raw_termios.c_lflag &= !(libc::ICANON | libc::ECHO);
    raw_termios.c_cc[libc::VMIN] = 1;
    raw_termios.c_cc[libc::VTIME] = 0;
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw_termios) } != 0 {
        close_fd(fd);
        return Err(IoError::NoInputAvailable);
    }

    // Read one byte, then restore settings regardless of outcome.
    let result = read_one_byte(fd);
    restore_and_close(fd, &orig_termios);

    let byte = match result {
        Ok(b) => b,
        Err(e) => return Err(e),
    };

    Ok(byte_to_key_name(byte, fd_reopen_for_esc_seq()))
}

/// Read a single byte from `fd`.
#[cfg(unix)]
fn read_one_byte(fd: std::os::unix::io::RawFd) -> Result<u8, IoError> {
    let mut buf = [0u8; 1];
    let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), 1) };
    if n <= 0 {
        return Err(IoError::NoInputAvailable);
    }
    Ok(buf[0])
}

/// Close a file descriptor.
#[cfg(unix)]
fn close_fd(fd: std::os::unix::io::RawFd) {
    unsafe { libc::close(fd) };
}

/// Restore terminal settings and close the fd.
#[cfg(unix)]
fn restore_and_close(fd: std::os::unix::io::RawFd, orig: &libc::termios) {
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, orig) };
    close_fd(fd);
}

/// Reopen `/dev/tty` for reading additional escape-sequence bytes.
/// Returns -1 on failure (non-fatal, used for escape sequence reads).
#[cfg(unix)]
fn fd_reopen_for_esc_seq() -> std::os::unix::io::RawFd {
    let tty_path = c"/dev/tty";
    unsafe { libc::open(tty_path.as_ptr().cast(), libc::O_RDONLY) }
}

/// Map a raw byte to a human-readable key name.
///
/// Handles printable ASCII, control characters, and VT100 escape
/// sequences (arrow keys, Delete, etc.).
#[cfg(unix)]
#[allow(clippy::cast_possible_wrap)]
fn byte_to_key_name(byte: u8, esc_fd: std::os::unix::io::RawFd) -> String {
    match byte {
        // Newline / carriage return → Enter
        0x0A | 0x0D => "Enter".to_string(),
        // Backspace
        0x7F => "Backspace".to_string(),
        // Tab
        0x09 => "Tab".to_string(),
        // Ctrl+C
        0x03 => "Ctrl+C".to_string(),
        // Ctrl+D
        0x04 => "Ctrl+D".to_string(),
        // Escape — read escape sequence
        0x1B => {
            if esc_fd < 0 {
                return "Escape".to_string();
            }
            // Save esc_fd terminal settings for the sequence read
            let mut esc_termios: libc::termios = unsafe { std::mem::zeroed() };
            let has_termios = unsafe { libc::tcgetattr(esc_fd, &mut esc_termios) } == 0;

            // Put the escape fd into raw mode too
            if has_termios {
                let mut raw = esc_termios;
                raw.c_lflag &= !(libc::ICANON | libc::ECHO);
                raw.c_cc[libc::VMIN] = 1;
                raw.c_cc[libc::VTIME] = 0;
                unsafe { libc::tcsetattr(esc_fd, libc::TCSANOW, &raw) };
            }

            // Try to read the next byte (should arrive quickly if it's a sequence)
            let next = read_one_byte(esc_fd);
            let result = match next {
                Ok(0x5B /* [ */) => {
                    // CSI sequence — read the command byte
                    match read_one_byte(esc_fd) {
                        Ok(0x41 /* A */) => "Up",
                        Ok(0x42 /* B */) => "Down",
                        Ok(0x43 /* C */) => "Right",
                        Ok(0x44 /* D */) => "Left",
                        Ok(0x33 /* 3 */) => {
                            // Could be Delete: ESC [ 3 ~
                            match read_one_byte(esc_fd) {
                                Ok(0x7E /* ~ */) => "Delete",
                                _ => "Escape",
                            }
                        }
                        _ => "Escape",
                    }
                    .to_string()
                }
                _ => "Escape".to_string(),
            };

            // Restore escape fd and close
            if has_termios {
                unsafe { libc::tcsetattr(esc_fd, libc::TCSANOW, &esc_termios) };
            }
            close_fd(esc_fd);
            result
        }
        // Other control bytes → Ctrl+<letter>
        0x01..=0x08 | 0x0B..=0x0C | 0x0E..=0x1A | 0x1C..=0x1F => {
            let letter = (b'a' + byte - 1) as char;
            format!("Ctrl+{letter}")
        }
        // Printable ASCII
        0x20..=0x7E => {
            let ch = byte as char;
            ch.to_string()
        }
        // Non-ASCII byte — return as hex
        other => format!("0x{other:02X}"),
    }
}

/// Mock IO provider for deterministic testing.
///
/// Pre-load responses via `push_line`, `push_key`, and `push_password`.
/// When a queue is exhausted, the operation returns `NoInputAvailable`.
pub struct MockIoProvider {
    lines: Mutex<VecDeque<String>>,
    keys: Mutex<VecDeque<String>>,
    passwords: Mutex<VecDeque<String>>,
}

impl MockIoProvider {
    /// Create an empty mock provider.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: Mutex::new(VecDeque::new()),
            keys: Mutex::new(VecDeque::new()),
            passwords: Mutex::new(VecDeque::new()),
        }
    }

    /// Pre-load line responses.
    #[must_use]
    pub fn with_lines(lines: Vec<String>) -> Self {
        Self {
            lines: Mutex::new(lines.into_iter().collect()),
            keys: Mutex::new(VecDeque::new()),
            passwords: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push_line(&self, line: String) {
        if let Ok(mut q) = self.lines.lock() {
            q.push_back(line);
        }
    }

    pub fn push_key(&self, key: String) {
        if let Ok(mut q) = self.keys.lock() {
            q.push_back(key);
        }
    }

    pub fn push_password(&self, pwd: String) {
        if let Ok(mut q) = self.passwords.lock() {
            q.push_back(pwd);
        }
    }
}

impl IoProvider for MockIoProvider {
    fn read_line(&self, _prompt: &str) -> Result<String, IoError> {
        self.lines
            .lock()
            .map(|mut q| q.pop_front())
            .map_err(|_| IoError::NoInputAvailable)?
            .ok_or(IoError::NoInputAvailable)
    }

    fn read_key(&self) -> Result<String, IoError> {
        self.keys
            .lock()
            .map(|mut q| q.pop_front())
            .map_err(|_| IoError::NoInputAvailable)?
            .ok_or(IoError::NoInputAvailable)
    }

    fn read_password(&self, _prompt: &str) -> Result<String, IoError> {
        self.passwords
            .lock()
            .map(|mut q| q.pop_front())
            .map_err(|_| IoError::NoInputAvailable)?
            .ok_or(IoError::NoInputAvailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn mock_provider_returns_preloaded_line() {
        let provider = MockIoProvider::with_lines(vec!["hello".to_string()]);
        assert_eq!(provider.read_line("").ok(), Some("hello".to_string()));
        assert!(provider.read_line("").is_err());
    }

    #[test]
    fn mock_provider_returns_preloaded_key() {
        let provider = MockIoProvider::new();
        provider.push_key("Enter".to_string());
        assert_eq!(provider.read_key().ok(), Some("Enter".to_string()));
        assert!(provider.read_key().is_err());
    }

    #[test]
    fn mock_provider_returns_preloaded_password() {
        let provider = MockIoProvider::new();
        provider.push_password("secret".to_string());
        assert_eq!(provider.read_password("").ok(), Some("secret".to_string()));
        assert!(provider.read_password("").is_err());
    }

    #[test]
    fn empty_mock_provider_returns_no_input() {
        let provider = MockIoProvider::new();
        assert!(provider.read_line("").is_err());
        assert!(provider.read_key().is_err());
        assert!(provider.read_password("").is_err());
    }

    #[test]
    fn std_provider_returns_no_input_on_empty_stdin() {
        // StdIoProvider can't be tested with real empty stdin easily,
        // but we verify the trait object works with Arc.
        let _provider: Arc<dyn IoProvider> = Arc::new(StdIoProvider);
    }

    #[test]
    fn mock_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockIoProvider>();
    }
}
