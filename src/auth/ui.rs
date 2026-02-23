use anyhow::{Result, anyhow};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io::{self, Write};

pub(super) fn clear_and_header(stdout: &mut impl Write) -> Result<()> {
    execute!(
        stdout,
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0),
        SetForegroundColor(Color::Cyan),
        Print(" ● dot — minimal ai agent\r\n\r\n"),
        ResetColor,
    )?;
    Ok(())
}

pub(super) fn select_from_menu(prompt: &str, items: &[&str]) -> Result<Option<usize>> {
    let mut stdout = io::stdout();
    let mut selected = 0usize;

    terminal::enable_raw_mode()?;
    execute!(stdout, cursor::Hide)?;

    let result: Result<Option<usize>> = (|| loop {
        clear_and_header(&mut stdout)?;

        execute!(
            stdout,
            SetForegroundColor(Color::White),
            Print(format!(" {}\r\n\r\n", prompt)),
            ResetColor,
        )?;

        for (i, item) in items.iter().enumerate() {
            if i == selected {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Green),
                    Print(format!("  ❯ {}\r\n", item)),
                    ResetColor,
                )?;
            } else {
                execute!(
                    stdout,
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("    {}\r\n", item)),
                    ResetColor,
                )?;
            }
        }

        execute!(
            stdout,
            Print("\r\n"),
            SetForegroundColor(Color::DarkGrey),
            Print("  ↑↓ navigate   enter select   q quit"),
            ResetColor,
        )?;
        stdout.flush()?;

        match event::read()? {
            Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) => match code {
                KeyCode::Up => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if selected < items.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter => return Ok(Some(selected)),
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                _ => {}
            },
            _ => {}
        }
    })();

    let _ = terminal::disable_raw_mode();
    execute!(stdout, cursor::Show, Print("\r\n"))?;
    result
}

pub(super) fn read_input_raw(prompt: &str, mask: bool) -> Result<String> {
    let mut stdout = io::stdout();

    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(format!("  {} ", prompt)),
        SetForegroundColor(Color::DarkGrey),
        Print("› "),
        ResetColor,
    )?;
    stdout.flush()?;

    terminal::enable_raw_mode()?;
    execute!(stdout, cursor::Show)?;

    let mut input = String::new();

    let result: Result<String> = (|| loop {
        match event::read()? {
            Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) => match code {
                KeyCode::Enter => {
                    execute!(stdout, Print("\r\n"))?;
                    return Ok(input.clone());
                }
                KeyCode::Backspace => {
                    if input.pop().is_some() {
                        execute!(stdout, Print("\x08 \x08"))?;
                    }
                }
                KeyCode::Esc => {
                    execute!(stdout, Print("\r\n"))?;
                    return Err(anyhow!("Cancelled"));
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    if mask {
                        execute!(stdout, Print("*"))?;
                    } else {
                        execute!(stdout, Print(c))?;
                    }
                }
                _ => {}
            },
            _ => {}
        }
        stdout.flush()?;
    })();

    let _ = terminal::disable_raw_mode();
    result
}

pub(super) fn read_api_key(prompt: &str) -> Result<String> {
    let mut stdout = io::stdout();
    let _ = terminal::disable_raw_mode();
    execute!(stdout, cursor::Show)?;

    clear_and_header(&mut stdout)?;

    execute!(
        stdout,
        SetForegroundColor(Color::White),
        Print(format!(" {}\r\n\r\n", prompt)),
        ResetColor,
    )?;

    let key = read_input_raw("API key", true)?;
    let trimmed = key.trim().to_string();
    if trimmed.is_empty() {
        return Err(anyhow!("API key cannot be empty"));
    }
    Ok(trimmed)
}
