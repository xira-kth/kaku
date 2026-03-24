mod app;
mod args;
mod input;
mod watch;

use std::fs;
use std::io::{self, Read, Write, stdout};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Attribute, Color, Print, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};

use crate::app::AppState;
use crate::args::CliArgs;
use crate::input::{PromptAction, PromptState};
use crate::watch::FileWatcher;
use kaku_core::parse_document;
use kaku_render::{LayoutOptions, layout_document};

fn main() {
    match run() {
        Ok(()) => {}
        Err(message) => {
            eprintln!("kaku: {message}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let args = CliArgs::parse()?;
    if args.help {
        println!("{}", CliArgs::usage());
        return Ok(());
    }

    let source = load_input(args.path.as_deref(), args.read_stdin)?;
    let document = parse_document(&source);

    if args.print {
        let layout = layout_document(
            &document,
            &LayoutOptions {
                width: terminal::size().map(|(w, _)| usize::from(w)).unwrap_or(100),
                theme: args.theme,
                syntax_highlighting: args.syntax_highlighting,
            },
        );
        print_layout(&layout, args.toc_open)?;
        return Ok(());
    }

    run_pager(args, source, document)
}

fn run_pager(args: CliArgs, _source: String, document: kaku_core::Document) -> Result<(), String> {
    let mut stdout = stdout();
    let (width, height) = terminal::size().map_err(|error| error.to_string())?;
    let mut app = AppState::new(
        document,
        usize::from(width),
        usize::from(height),
        args.theme,
        args.syntax_highlighting,
        args.toc_open,
    );
    let mut watcher = if args.watch {
        args.path
            .as_deref()
            .map(FileWatcher::new)
            .transpose()
            .map_err(|error| error.to_string())?
    } else {
        None
    };
    let mut prompt = None;

    enable_raw_mode().map_err(|error| error.to_string())?;
    execute!(stdout, EnterAlternateScreen, Hide).map_err(|error| error.to_string())?;

    let result = pager_loop(&mut stdout, &mut app, &args, &mut watcher, &mut prompt);
    let cleanup_result = cleanup_terminal(&mut stdout);

    result.and(cleanup_result)
}

fn pager_loop(
    stdout: &mut io::Stdout,
    app: &mut AppState,
    args: &CliArgs,
    watcher: &mut Option<FileWatcher>,
    prompt: &mut Option<PromptState>,
) -> Result<(), String> {
    loop {
        draw(stdout, app, prompt.as_ref())?;

        if watcher.as_mut().is_some_and(FileWatcher::has_changes) {
            if let Some(path) = args.path.as_deref() {
                let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
                let width = terminal::size()
                    .map(|(width, _)| usize::from(width))
                    .unwrap_or(100);
                app.replace_document(parse_document(&source), width);
            }
        }

        if event::poll(Duration::from_millis(100)).map_err(|error| error.to_string())? {
            match event::read().map_err(|error| error.to_string())? {
                Event::Key(key) => {
                    if handle_key(key, app, args, watcher, prompt)? {
                        break;
                    }
                }
                Event::Resize(width, height) => {
                    app.resize(usize::from(width), usize::from(height));
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn handle_key(
    key: KeyEvent,
    app: &mut AppState,
    args: &CliArgs,
    watcher: &mut Option<FileWatcher>,
    prompt: &mut Option<PromptState>,
) -> Result<bool, String> {
    if let Some(state) = prompt {
        match state.handle_key(key) {
            PromptAction::Continue => return Ok(false),
            PromptAction::Cancel => {
                app.status = "search cancelled".to_string();
                *prompt = None;
                return Ok(false);
            }
            PromptAction::Submit(query) => {
                app.apply_search_query(query);
                *prompt = None;
                return Ok(false);
            }
        }
    }

    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('j') | KeyCode::Down => {
            if app.toc_open {
                app.select_next_toc();
            } else {
                app.scroll_down(1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.toc_open {
                app.select_prev_toc();
            } else {
                app.scroll_up(1);
            }
        }
        KeyCode::PageDown | KeyCode::Char(' ') => app.page_down(),
        KeyCode::PageUp => app.page_up(),
        KeyCode::Char('g') if key.modifiers.is_empty() => app.go_top(),
        KeyCode::Char('G') | KeyCode::End => app.go_bottom(),
        KeyCode::Char('/') => {
            *prompt = Some(PromptState::new());
        }
        KeyCode::Char('n') => app.next_search_match(),
        KeyCode::Char('N') => app.previous_search_match(),
        KeyCode::Char('t') => app.toggle_toc(),
        KeyCode::Enter if app.toc_open => app.jump_to_selected_toc(),
        KeyCode::Char('o') => {
            if let Some(link_index) = app.first_visible_link() {
                if let Some(link) = app.document.links.get(link_index) {
                    open_link(&link.destination)?;
                    app.status = format!("opened {}", link.destination);
                }
            } else {
                app.status = "no visible link".to_string();
            }
        }
        KeyCode::Char('r') => {
            reload_from_disk(app, args)?;
            if let Some(path) = args.path.as_deref() {
                *watcher = Some(FileWatcher::new(path).map_err(|error| error.to_string())?);
            }
        }
        KeyCode::Esc if app.toc_open => app.toggle_toc(),
        _ => {}
    }

    Ok(false)
}

fn reload_from_disk(app: &mut AppState, args: &CliArgs) -> Result<(), String> {
    let Some(path) = args.path.as_deref() else {
        app.status = "reload is only available for files".to_string();
        return Ok(());
    };

    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100);
    app.replace_document(parse_document(&source), width);
    Ok(())
}

fn draw(
    stdout: &mut io::Stdout,
    app: &AppState,
    prompt: Option<&PromptState>,
) -> Result<(), String> {
    let (width, height) = terminal::size().map_err(|error| error.to_string())?;
    let width = usize::from(width);
    let height = usize::from(height);
    let body_height = height.saturating_sub(1);
    let toc_width = if app.toc_open { width.min(30) } else { 0 };
    let body_width = width.saturating_sub(toc_width);

    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All)).map_err(|error| error.to_string())?;

    if app.toc_open {
        draw_toc(stdout, app, toc_width, body_height)?;
    }
    draw_body(stdout, app, toc_width, body_width, body_height)?;
    draw_status(stdout, app, prompt, width, height)?;
    stdout.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn draw_toc(
    stdout: &mut io::Stdout,
    app: &AppState,
    width: usize,
    height: usize,
) -> Result<(), String> {
    let entries = app.toc_entries();
    let start = app.toc_selected.saturating_sub(height.saturating_sub(1));
    let end = (start + height).min(entries.len());

    for (row, entry) in entries[start..end].iter().enumerate() {
        let y = u16::try_from(row).map_err(|_| "screen too tall".to_string())?;
        let indent = " ".repeat(entry.level.as_usize().saturating_sub(1) * 2);
        let marker = if start + row == app.toc_selected {
            "› "
        } else {
            "  "
        };
        let available = width.saturating_sub(marker.len() + indent.len()).max(1);
        let title = truncate(&entry.title, available);

        queue!(
            stdout,
            MoveTo(0, y),
            SetForegroundColor(if start + row == app.toc_selected {
                Color::Yellow
            } else {
                Color::DarkGrey
            }),
            Print(format!("{marker}{indent}{title:<available$}")),
            SetForegroundColor(Color::Reset)
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn draw_body(
    stdout: &mut io::Stdout,
    app: &AppState,
    offset_x: usize,
    width: usize,
    height: usize,
) -> Result<(), String> {
    for row in 0..height {
        let Some(line) = app.visible_lines().get(row) else {
            continue;
        };

        let y = u16::try_from(row).map_err(|_| "screen too tall".to_string())?;
        let x = u16::try_from(offset_x).map_err(|_| "screen too wide".to_string())?;
        let rendered = if line.plain_text.trim().is_empty() {
            String::new()
        } else {
            truncate_ansi(&line.to_ansi_string(), width)
        };
        queue!(stdout, MoveTo(x, y), Print(rendered)).map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn draw_status(
    stdout: &mut io::Stdout,
    app: &AppState,
    prompt: Option<&PromptState>,
    width: usize,
    height: usize,
) -> Result<(), String> {
    let y = u16::try_from(height.saturating_sub(1)).map_err(|_| "screen too tall".to_string())?;
    let text = if let Some(prompt) = prompt {
        format!("/{}", prompt.value())
    } else {
        format!(
            "{}  [{} / {}]",
            app.status,
            app.scroll + 1,
            app.layout.lines.len().max(1)
        )
    };
    let padded = format!("{:<width$}", truncate(&text, width), width = width);
    queue!(
        stdout,
        MoveTo(0, y),
        SetAttribute(Attribute::Reverse),
        Print(padded),
        SetAttribute(Attribute::Reset)
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn print_layout(layout: &kaku_render::Layout, toc_only: bool) -> Result<(), String> {
    if toc_only {
        for entry in &layout.toc {
            println!(
                "{}{}",
                " ".repeat(entry.level.as_usize().saturating_sub(1) * 2),
                entry.title
            );
        }
        return Ok(());
    }

    let mut stdout = io::stdout().lock();
    for line in &layout.lines {
        writeln!(stdout, "{}", line.to_ansi_string()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn load_input(path: Option<&Path>, read_stdin: bool) -> Result<String, String> {
    if read_stdin {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|error| error.to_string())?;
        return Ok(buffer);
    }

    let Some(path) = path else {
        return Err("missing input path".to_string());
    };
    fs::read_to_string(path).map_err(|error| error.to_string())
}

fn open_link(target: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(target);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(target);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", target]);
        command
    };

    command.status().map_err(|error| error.to_string())?;
    Ok(())
}

fn cleanup_terminal(stdout: &mut io::Stdout) -> Result<(), String> {
    disable_raw_mode().map_err(|error| error.to_string())?;
    execute!(stdout, Show, LeaveAlternateScreen).map_err(|error| error.to_string())?;
    Ok(())
}

fn truncate(input: &str, width: usize) -> String {
    input.chars().take(width).collect()
}

fn truncate_ansi(input: &str, width: usize) -> String {
    let mut visible = 0;
    let mut out = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            out.push(ch);
            for next in chars.by_ref() {
                out.push(next);
                if next == 'm' {
                    break;
                }
            }
            continue;
        }

        if visible >= width {
            break;
        }

        visible += 1;
        out.push(ch);
    }

    out.push_str("\u{1b}[0m");
    out
}

#[cfg(test)]
mod tests {
    use super::{print_layout, truncate_ansi};
    use kaku_core::parse_document;
    use kaku_render::{LayoutOptions, ThemeName, layout_document};

    #[test]
    fn print_mode_layout_builds() {
        let doc = parse_document("# Title\n\nhello");
        let layout = layout_document(
            &doc,
            &LayoutOptions {
                width: 80,
                theme: ThemeName::Dark,
                syntax_highlighting: false,
            },
        );
        assert!(print_layout(&layout, false).is_ok());
    }

    #[test]
    fn ansi_truncation_keeps_reset() {
        let text = "\u{1b}[31mhello\u{1b}[0m";
        assert!(truncate_ansi(text, 3).ends_with("\u{1b}[0m"));
    }
}
