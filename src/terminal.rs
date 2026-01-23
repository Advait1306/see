use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Point as AlacPoint, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::tty::{self, Options as PtyOptions};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct TerminalEventListener {
    sender: std::sync::mpsc::Sender<Event>,
}

impl EventListener for TerminalEventListener {
    fn send_event(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}

#[derive(Clone)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            cell_width: 9,
            cell_height: 18,
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }

    fn last_column(&self) -> alacritty_terminal::index::Column {
        alacritty_terminal::index::Column(self.cols.saturating_sub(1) as usize)
    }

    fn bottommost_line(&self) -> alacritty_terminal::index::Line {
        alacritty_terminal::index::Line(self.rows as i32 - 1)
    }
}

pub struct Terminal {
    term: Arc<FairMutex<Term<TerminalEventListener>>>,
    notifier: Notifier,
    event_receiver: std::sync::mpsc::Receiver<Event>,
    pub working_directory: PathBuf,
    size: TerminalSize,
}

impl Terminal {
    pub fn new(working_directory: PathBuf) -> Result<Self> {
        let size = TerminalSize::default();
        let (event_sender, event_receiver) = std::sync::mpsc::channel();

        let listener = TerminalEventListener {
            sender: event_sender,
        };

        let config = TermConfig::default();
        let term_size = TermSize::new(size.cols as usize, size.rows as usize);
        let term = Term::new(config, &term_size, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        let mut env = std::collections::HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());

        let pty_options = PtyOptions {
            shell: None,
            working_directory: Some(working_directory.clone()),
            env,
            ..Default::default()
        };

        let window_size = WindowSize {
            num_cols: size.cols,
            num_lines: size.rows,
            cell_width: size.cell_width,
            cell_height: size.cell_height,
        };

        let pty = tty::new(&pty_options, window_size, 0)?;
        let event_loop = EventLoop::new(term.clone(), listener, pty, false, false)?;
        let notifier = Notifier(event_loop.channel());
        let _event_loop_handle = event_loop.spawn();

        Ok(Self {
            term,
            notifier,
            event_receiver,
            working_directory,
            size,
        })
    }

    pub fn write(&self, input: &[u8]) {
        let _ = self.notifier.0.send(Msg::Input(input.to_vec().into()));
    }

    pub fn resize(&mut self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
        self.size = TerminalSize {
            cols,
            rows,
            cell_width,
            cell_height,
        };

        let window_size = WindowSize {
            num_cols: cols,
            num_lines: rows,
            cell_width,
            cell_height,
        };

        let _ = self.notifier.0.send(Msg::Resize(window_size));
        self.term.lock().resize(self.size.clone());
    }

    pub fn with_term<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Term<TerminalEventListener>) -> R,
    {
        let term = self.term.lock();
        f(&term)
    }

    pub fn drain_events(&self) -> bool {
        // Consume all pending events and return true if there were any
        let mut had_events = false;
        while self.event_receiver.try_recv().is_ok() {
            had_events = true;
        }
        had_events
    }

    pub fn scroll(&self, delta: i32) {
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Delta(delta));
    }

    pub fn start_selection(&self, selection_type: SelectionType, point: AlacPoint, side: Side) {
        let mut term = self.term.lock();
        let selection = Selection::new(selection_type, point, side);
        term.selection = Some(selection);
    }

    pub fn update_selection(&self, point: AlacPoint, side: Side) {
        let mut term = self.term.lock();
        if let Some(mut selection) = term.selection.take() {
            selection.update(point, side);
            term.selection = Some(selection);
        }
    }

    pub fn clear_selection(&self) {
        let mut term = self.term.lock();
        term.selection = None;
    }

    pub fn has_selection(&self) -> bool {
        let term = self.term.lock();
        term.selection.is_some()
    }

    pub fn selection_to_string(&self) -> Option<String> {
        let term = self.term.lock();
        term.selection_to_string()
    }

    pub fn display_offset(&self) -> usize {
        let term = self.term.lock();
        term.grid().display_offset()
    }
}
