use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use walls_core::config::TuiKeyProfile;

use super::app::{self, App, InputMode, Tab};
use super::open_target;
use super::style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiAction {
    Quit,
    EnterCommandMode,
    CancelInput,
    SubmitCommand,
    CommandBackspace,
    CommandChar(char),
    CommandComplete {
        forward: bool,
    },
    SubmitSearch,
    SearchBackspace,
    SearchChar(char),
    Next,
    Prev,
    Favorite,
    Trash,
    OpenSelected,
    TrashConfirm,
    CancelTrash,
    NukeDownloadsRequest,
    NukeDownloadsConfirm,
    CancelNuke,
    TogglePause,
    ToggleConfigValue,
    AddSource,
    RemoveSource,
    #[allow(dead_code)]
    CycleConfigValue,
    EditConfigItem,
    CancelEdit,
    EditFieldChar(char),
    EditFieldBackspace,
    EditFieldCommit,
    EditFieldUp,
    EditFieldDown,
    EditFieldCycle {
        forward: bool,
    },
    ExitConfigSubnav,
    #[allow(dead_code)]
    SaveEditItem,
    OpenHelp,
    CloseHelp,
    SwitchTab(Tab),
    SwitchTabNext,
    SwitchTabPrev,
    EditSearch,
    EditSearchFilters,
    MoveDown,
    MoveUp,
    MoveFirst,
    MoveLast,
    VimPrefixG,
    PageDown,
    PageUp,
    Enter,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateEffect {
    None,
    Reload,
    Quit,
}

pub(super) fn handle_key(
    app: &mut App,
    key: KeyEvent,
    rt: &tokio::runtime::Handle,
) -> anyhow::Result<bool> {
    let action = action_for_key(app, key);
    let effect = update(app, action, rt)?;
    apply_effect(app, effect)?;
    Ok(effect == UpdateEffect::Quit)
}

fn is_shift_x(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('X' | 'x')) && key.modifiers.contains(KeyModifiers::SHIFT)
}

pub(crate) fn action_for_key(app: &App, key: KeyEvent) -> UiAction {
    if app.show_key_help {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => UiAction::CloseHelp,
            _ => UiAction::Ignore,
        };
    }

    if app.pending_trash_confirm {
        return match key.code {
            KeyCode::Esc => UiAction::CancelTrash,
            KeyCode::Char('d') => UiAction::TrashConfirm,
            _ => UiAction::Ignore,
        };
    }

    if app.pending_nuke_confirm {
        return match key.code {
            KeyCode::Esc => UiAction::CancelNuke,
            _ if is_shift_x(key) => UiAction::NukeDownloadsConfirm,
            _ => UiAction::Ignore,
        };
    }

    match app.input_mode {
        InputMode::Command => {
            return match key.code {
                KeyCode::Esc => UiAction::CancelInput,
                KeyCode::Enter => UiAction::SubmitCommand,
                KeyCode::Backspace => UiAction::CommandBackspace,
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    UiAction::CommandComplete { forward: true }
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    UiAction::CommandComplete { forward: false }
                }
                KeyCode::Char(c) => UiAction::CommandChar(c),
                _ => UiAction::Ignore,
            };
        }
        InputMode::SearchInput => {
            return match key.code {
                KeyCode::Esc => UiAction::CancelInput,
                KeyCode::Enter => UiAction::SubmitSearch,
                KeyCode::Backspace => UiAction::SearchBackspace,
                KeyCode::Char(c) => UiAction::SearchChar(c),
                _ => UiAction::Ignore,
            };
        }
        InputMode::Normal => {}
    }

    // Editing steals keys for field nav (arrows only), live typing, commit/save on Enter, cancel.
    // Letter j/k are *not* field nav here (they become Char => type into buffer, or for list nav you Esc first then j/k).
    // This implements "Rather than jk in edit mode. Let's allow the user to hit escape first and then j/k".
    // n/p etc also type (disabled for globals).
    // Enter = commit buffer for current field + persist/save the item (no separate save key; "enter ... should just save the config").
    if app.is_editing() {
        return match key.code {
            KeyCode::Up => UiAction::EditFieldUp,
            KeyCode::Down => UiAction::EditFieldDown,
            KeyCode::Left => UiAction::EditFieldCycle { forward: false },
            KeyCode::Right => UiAction::EditFieldCycle { forward: true },
            KeyCode::Char(' ')
                if matches!(
                    app.current_edit_field_kind(),
                    app::EditFieldKind::Bool | app::EditFieldKind::Choice(_)
                ) =>
            {
                UiAction::EditFieldCycle { forward: true }
            }
            KeyCode::Esc => UiAction::CancelEdit,
            KeyCode::Backspace => UiAction::EditFieldBackspace,
            KeyCode::Enter => UiAction::EditFieldCommit,
            KeyCode::Char(c) => UiAction::EditFieldChar(c),
            _ => UiAction::Ignore,
        };
    }

    if app.ctx.config.tui.key_profile == TuiKeyProfile::Vim {
        if app.vim_pending_g && key.code == KeyCode::Char('g') {
            return UiAction::MoveFirst;
        }
        match key.code {
            KeyCode::Char('h') => return UiAction::SwitchTabPrev,
            KeyCode::Char('l') => return UiAction::SwitchTabNext,
            KeyCode::Char('g') => return UiAction::VimPrefixG,
            KeyCode::Char('G') => return UiAction::MoveLast,
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => UiAction::Quit,
        KeyCode::Char('?') => UiAction::OpenHelp,
        KeyCode::Char(':') => UiAction::EnterCommandMode,
        KeyCode::Char('n') => UiAction::Next,
        KeyCode::Char('p') => UiAction::Prev,
        KeyCode::Char('f') => UiAction::Favorite,
        KeyCode::Char('d') => UiAction::Trash,
        KeyCode::Char('o') => UiAction::OpenSelected,
        _ if is_shift_x(key) => UiAction::NukeDownloadsRequest,
        KeyCode::Char(' ') => UiAction::TogglePause,
        KeyCode::Char('a')
            if app.tab == Tab::Config && app.is_sources_list_block(app.config_cursor) =>
        {
            UiAction::AddSource
        }
        KeyCode::Char('x')
            if app.tab == Tab::Config
                && app.is_sources_list_block(app.config_cursor)
                && app.can_remove_selected_source() =>
        {
            UiAction::RemoveSource
        }
        KeyCode::Char('t') if app.tab == Tab::Config => UiAction::ToggleConfigValue,
        KeyCode::Char('e') if app.tab == Tab::Config => UiAction::EditConfigItem,
        KeyCode::Char('e') if app.tab == Tab::Search => UiAction::EditSearchFilters,
        KeyCode::Char(c @ '1'..='6') => {
            let index = c
                .to_digit(10)
                .expect("key guard only allows ASCII digits 1-6") as usize
                - 1;
            UiAction::SwitchTab(Tab::from_index(index))
        }
        KeyCode::Right => UiAction::SwitchTabNext,
        KeyCode::Left => UiAction::SwitchTabPrev,
        KeyCode::Char('/') => UiAction::EditSearch,
        KeyCode::Char('i') if app.tab == Tab::Search => UiAction::EditSearch,
        KeyCode::Down | KeyCode::Char('j') => UiAction::MoveDown,
        KeyCode::Up | KeyCode::Char('k') => UiAction::MoveUp,
        KeyCode::Home => UiAction::MoveFirst,
        KeyCode::End => UiAction::MoveLast,
        KeyCode::PageDown => UiAction::PageDown,
        KeyCode::PageUp => UiAction::PageUp,
        KeyCode::Esc
            if app.tab == Tab::Config
                && app.config_in_subnav
                && app.is_sources_list_block(app.config_cursor) =>
        {
            UiAction::ExitConfigSubnav
        }
        KeyCode::Enter => UiAction::Enter,
        _ => UiAction::Ignore,
    }
}

pub(crate) fn update(
    app: &mut App,
    action: UiAction,
    rt: &tokio::runtime::Handle,
) -> anyhow::Result<UpdateEffect> {
    if !matches!(action, UiAction::VimPrefixG) {
        app.vim_pending_g = false;
    }
    match action {
        UiAction::Quit => return Ok(UpdateEffect::Quit),
        UiAction::EnterCommandMode => {
            app.input_mode = InputMode::Command;
            app.cmd_line.clear();
        }
        UiAction::CancelInput => {
            app.input_mode = InputMode::Normal;
            app.cmd_line.clear();
        }
        UiAction::SubmitCommand => {
            match app.run_command(rt)? {
                None => return Ok(UpdateEffect::Quit),
                Some((msg, kind)) => app.set_message(kind, msg),
            }
            app.input_mode = InputMode::Normal;
            app.cmd_line.clear();
            return Ok(UpdateEffect::Reload);
        }
        UiAction::CommandBackspace => {
            app.cmd_line.pop();
        }
        UiAction::CommandChar(c) => app.cmd_line.push(c),
        UiAction::CommandComplete { forward } => app.complete_command(forward),
        UiAction::SubmitSearch => {
            app.input_mode = InputMode::Normal;
            match tokio::task::block_in_place(|| rt.block_on(app.run_search())) {
                Ok(()) => app.set_message(
                    style::StatusKind::Success,
                    format!("search: {} results", app.search_results.len()),
                ),
                Err(e) => app.set_message(style::StatusKind::Error, format!("search error: {e}")),
            };
        }
        UiAction::SearchBackspace => {
            app.search_query.pop();
            app.search_filters.q = app.search_query.clone();
        }
        UiAction::SearchChar(c) => {
            app.search_query.push(c);
            app.search_filters.q = app.search_query.clone();
        }
        UiAction::Next => {
            match tokio::task::block_in_place(|| rt.block_on(app.ctx.advance_next_manual())) {
                Ok(Some(p)) => {
                    app.set_message(style::StatusKind::Success, format!("next: {}", p.display()))
                }
                Ok(None) => app.set_message(
                    style::StatusKind::Neutral,
                    crate::recovery::tui_next_no_change(),
                ),
                Err(e) => {
                    app.set_message(style::StatusKind::Error, crate::recovery::next_error(&e))
                }
            }
            return Ok(UpdateEffect::Reload);
        }
        UiAction::Prev => {
            match app.ctx.advance_prev() {
                Ok(Some(p)) => {
                    app.set_message(style::StatusKind::Success, format!("prev: {}", p.display()))
                }
                Ok(None) => app.set_message(
                    style::StatusKind::Neutral,
                    crate::recovery::tui_no_previous(),
                ),
                Err(e) => {
                    app.set_message(style::StatusKind::Error, crate::recovery::prev_error(&e))
                }
            }
            return Ok(UpdateEffect::Reload);
        }
        UiAction::Favorite => match app.favorite_current() {
            Ok(msg) => {
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
            Err(e) => app.set_message(
                style::StatusKind::Error,
                crate::recovery::favorite_error(&e),
            ),
        },
        UiAction::OpenSelected => match open_selected(app) {
            Ok(Some(message)) => app.set_message(style::StatusKind::Success, message),
            Ok(None) => app.set_message(
                style::StatusKind::Warning,
                "open: nothing openable under cursor",
            ),
            Err(error) => app.set_message(style::StatusKind::Error, format!("open error: {error}")),
        },
        UiAction::Trash => {
            let prompt = app.trash_current_prompt();
            if prompt.contains("d confirm") {
                app.pending_trash_confirm = true;
                app.pending_nuke_confirm = false;
                app.set_message(style::StatusKind::Warning, prompt);
            } else {
                app.set_message(style::StatusKind::Error, prompt);
            }
        }
        UiAction::TrashConfirm => match app.trash_current() {
            Ok(msg) => {
                app.pending_trash_confirm = false;
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
            Err(e) => {
                app.pending_trash_confirm = false;
                app.set_message(style::StatusKind::Error, format!("trash error: {e}"));
            }
        },
        UiAction::CancelTrash => {
            app.pending_trash_confirm = false;
            app.set_message(style::StatusKind::Neutral, "trash cancelled");
        }
        UiAction::NukeDownloadsRequest => {
            let prompt = app.nuke_downloads_prompt();
            if prompt.contains("Shift+X confirm") {
                app.pending_nuke_confirm = true;
                app.pending_trash_confirm = false;
            }
            app.set_message(style::StatusKind::Warning, prompt);
        }
        UiAction::NukeDownloadsConfirm => match app.nuke_downloads() {
            Ok(msg) => {
                app.pending_nuke_confirm = false;
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
            Err(e) => {
                app.pending_nuke_confirm = false;
                app.set_message(
                    style::StatusKind::Error,
                    format!("provider reset error: {e}"),
                );
            }
        },
        UiAction::CancelNuke => {
            app.pending_nuke_confirm = false;
            app.set_message(style::StatusKind::Neutral, "provider reset cancelled");
        }
        UiAction::TogglePause => match app.ctx.toggle_pause() {
            Ok(()) => app.set_message(
                style::StatusKind::Success,
                format!("paused: {}", app.ctx.state.paused),
            ),
            Err(e) => app.set_message(style::StatusKind::Error, format!("pause error: {e}")),
        },
        UiAction::ToggleConfigValue => match app.toggle_focused_config_value() {
            Ok(Some(msg)) => {
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
            Ok(None) => app.set_message(
                style::StatusKind::Warning,
                "config: no toggle for focused block",
            ),
            Err(e) => app.set_message(style::StatusKind::Error, format!("config save error: {e}")),
        },
        UiAction::AddSource => {
            if let Err(error) = app.add_wallhaven_source() {
                app.set_message(
                    style::StatusKind::Error,
                    format!("add source error: {error}"),
                );
            }
        }
        UiAction::RemoveSource => match app.remove_selected_source() {
            Ok(Some(msg)) => {
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
            Ok(None) => {}
            Err(error) => app.set_message(
                style::StatusKind::Error,
                format!("remove source error: {error}"),
            ),
        },
        UiAction::CycleConfigValue => match app.cycle_focused_config_value() {
            Ok(Some(msg)) => {
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
            Ok(None) => app.set_message(
                style::StatusKind::Warning,
                "config: no cycle for focused block",
            ),
            Err(e) => app.set_message(style::StatusKind::Error, format!("config save error: {e}")),
        },
        UiAction::EditConfigItem => {
            app.start_edit_for_current();
        }
        UiAction::EditSearchFilters => {
            app.start_search_filter_edit();
        }
        UiAction::CancelEdit => {
            app.cancel_edit();
        }
        UiAction::EditFieldChar(c) => {
            if matches!(
                app.current_edit_field_kind(),
                app::EditFieldKind::Bool | app::EditFieldKind::Choice(_)
            ) {
                // Choice/bool fields use Space/arrow cycling, not free text.
            } else if let Some(sess) = &mut app.editing {
                sess.field_buffer.push(c);
                app.refresh_edit_validation();
            }
        }
        UiAction::EditFieldBackspace => {
            if matches!(
                app.current_edit_field_kind(),
                app::EditFieldKind::Bool | app::EditFieldKind::Choice(_)
            ) {
                // Choice/bool fields use Space/arrow cycling, not free text.
            } else if let Some(sess) = &mut app.editing {
                sess.field_buffer.pop();
                app.refresh_edit_validation();
            }
        }
        UiAction::EditFieldCycle { forward } => {
            app.cycle_current_edit_field(forward);
        }
        UiAction::ExitConfigSubnav => {
            app.exit_config_subnav();
        }
        UiAction::EditFieldCommit => {
            if app.editing.is_some() {
                app.commit_edit_field_buffer();
                // Commit the field to draft, then persist/save the item (atomic write + reload).
                // Keep the edit form open (user can continue to other fields of this item or Esc to leave).
                // This makes "type ... and hit enter" save the config without a separate save step.
                let _ = app.save_edit_item(false);
                // Re-fill buffer from the (now committed) draft value so the focused line shows "val|" (not empty |)
                // ready for further typing on this field, and indicates the committed state.
                if app.is_editing() {
                    let val = app.current_edit_field_value();
                    if let Some(s) = &mut app.editing {
                        s.field_buffer = val;
                    }
                }
            }
        }
        UiAction::EditFieldUp => {
            // Pure field move inside edit form (triggered by arrows; letter j/k no longer do this).
            // No auto commit/persist on arrow (uncommitted typing on a field is lost if you arrow away; hit Enter to commit+save a field).
            let buf = if let Some(sess) = &app.editing {
                let c = sess.field_cursor.saturating_sub(1);
                app.edit_field_value_at(&sess.target, c)
            } else {
                String::new()
            };
            if let Some(sess) = &mut app.editing {
                if sess.field_cursor > 0 {
                    sess.field_cursor -= 1;
                }
                sess.field_buffer = buf;
            }
        }
        UiAction::EditFieldDown => {
            // Pure field move inside edit form (triggered by arrows; letter j/k no longer do this).
            // No auto commit/persist on arrow (uncommitted typing on a field is lost if you arrow away; hit Enter to commit+save a field).
            let max_fields = app.edit_field_count();
            let buf = if let Some(sess) = &app.editing {
                let c = (sess.field_cursor + 1).min(max_fields.saturating_sub(1));
                app.edit_field_value_at(&sess.target, c)
            } else {
                String::new()
            };
            if let Some(sess) = &mut app.editing {
                if sess.field_cursor + 1 < max_fields {
                    sess.field_cursor += 1;
                }
                sess.field_buffer = buf;
            }
        }
        UiAction::SaveEditItem => {
            // The (now un-bound in edit UI) Save action does full commit+persist and exits the edit form.
            let _ = app.save_edit_item(true);
        }
        UiAction::OpenHelp => {
            app.show_key_help = true;
        }
        UiAction::CloseHelp => {
            app.show_key_help = false;
        }
        UiAction::SwitchTab(tab) => {
            app.switch_tab(tab);
        }
        UiAction::SwitchTabNext => {
            app.switch_tab(Tab::from_index((app.tab.index() + 1) % 6));
        }
        UiAction::SwitchTabPrev => {
            app.switch_tab(Tab::from_index((app.tab.index() + 5) % 6));
        }
        UiAction::EditSearch => {
            app.tab = Tab::Search;
            app.cursor = 0;
            app.config_in_subnav = false;
            app.editing = None;
            app.input_mode = InputMode::SearchInput;
        }
        UiAction::MoveDown => app.move_down(),
        UiAction::MoveUp => app.move_up(),
        UiAction::MoveFirst => app.move_first(),
        UiAction::MoveLast => app.move_last(),
        UiAction::VimPrefixG => {
            app.vim_pending_g = true;
        }
        UiAction::PageDown => app.page_down(),
        UiAction::PageUp => app.page_up(),
        UiAction::Enter => return handle_enter(app, rt),
        UiAction::Ignore => {}
    }
    Ok(UpdateEffect::None)
}

fn open_selected(app: &App) -> anyhow::Result<Option<String>> {
    let Some(target) = app.selected_open_target() else {
        return Ok(None);
    };
    open_target::spawn(&target)?;
    Ok(Some(format!("opened: {}", target.display_value())))
}

pub(crate) fn apply_effect(app: &mut App, effect: UpdateEffect) -> anyhow::Result<()> {
    if effect == UpdateEffect::Reload {
        app.reload_ctx()?;
    }
    Ok(())
}

fn handle_enter(app: &mut App, rt: &tokio::runtime::Handle) -> anyhow::Result<UpdateEffect> {
    match app.tab {
        Tab::History => {
            if let Some(path) = app.apply_history_selection() {
                app.set_message(
                    style::StatusKind::Success,
                    format!("applied: {}", path.display()),
                );
                return Ok(UpdateEffect::Reload);
            }
        }
        Tab::Browse => {
            if let Some(msg) =
                tokio::task::block_in_place(|| rt.block_on(app.apply_browse_selection()))?
            {
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
        }
        Tab::Search => {
            if app.search_results.is_empty() {
                app.input_mode = InputMode::SearchInput;
            } else if let Some(msg) =
                tokio::task::block_in_place(|| rt.block_on(app.apply_search_selection()))?
            {
                app.set_message(style::StatusKind::Success, msg);
                return Ok(UpdateEffect::Reload);
            }
        }
        Tab::Config if app.is_sources_list_block(app.config_cursor) && !app.config_in_subnav => {
            app.enter_config_subnav();
        }
        _ => {}
    }
    Ok(UpdateEffect::None)
}
