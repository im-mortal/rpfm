//---------------------------------------------------------------------------//
// Copyright (c) 2017-2020 Ismael Gutiérrez González. All rights reserved.
//
// This file is part of the Rusted PackFile Manager (RPFM) project,
// which can be found here: https://github.com/Frodo45127/rpfm.
//
// This file is licensed under the MIT license, which can be found here:
// https://github.com/Frodo45127/rpfm/blob/master/LICENSE.
//---------------------------------------------------------------------------//

/*!
Module with the slots for Table Views.
!*/

use qt_widgets::SlotOfQPoint;
use qt_widgets::QFileDialog;
use qt_widgets::q_file_dialog::AcceptMode;

use qt_gui::QBrush;
use qt_gui::QCursor;
use qt_gui::QGuiApplication;
use qt_gui::SlotOfQStandardItem;

use qt_core::GlobalColor;
use qt_core::QModelIndex;
use qt_core::QItemSelection;
use qt_core::QSignalBlocker;
use qt_core::{SlotOfBool, SlotOfInt, Slot, SlotOfQString, SlotOfQItemSelectionQItemSelection};

use cpp_core::Ref;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::path::PathBuf;

use rpfm_lib::schema::Definition;
use rpfm_lib::SETTINGS;

use crate::ffi::*;
use crate::global_search_ui::GlobalSearchUI;
use crate::packfile_contents_ui::PackFileContentsUI;
use crate::packedfile_views::table::PackedFileTableViewRaw;
use crate::utils::atomic_from_mut_ptr;
use crate::utils::show_dialog;
use crate::UI_STATE;

use super::*;
use super::utils::*;

//-------------------------------------------------------------------------------//
//                              Enums & Structs
//-------------------------------------------------------------------------------//

/// This struct contains the slots of the view of an Table PackedFile.
pub struct PackedFileTableViewSlots {
    pub filter_line_edit: SlotOfQString<'static>,
    pub filter_column_selector: SlotOfInt<'static>,
    pub filter_case_sensitive_button: Slot<'static>,
    pub toggle_lookups: SlotOfBool<'static>,
    pub show_context_menu: SlotOfQPoint<'static>,
    pub context_menu_enabler: SlotOfQItemSelectionQItemSelection<'static>,
    pub item_changed: SlotOfQStandardItem<'static>,
    pub add_rows: Slot<'static>,
    pub insert_rows: Slot<'static>,
    pub delete_rows: Slot<'static>,
    pub clone_and_append: Slot<'static>,
    pub clone_and_insert: Slot<'static>,
    pub copy: Slot<'static>,
    pub copy_as_lua_table: Slot<'static>,
    pub paste: Slot<'static>,
    pub invert_selection: Slot<'static>,
    pub reset_selection: Slot<'static>,
    pub save: Slot<'static>,
    pub undo: Slot<'static>,
    pub redo: Slot<'static>,
    pub import_tsv: SlotOfBool<'static>,
    pub export_tsv: SlotOfBool<'static>,
    pub smart_delete: Slot<'static>,
    pub sidebar: SlotOfBool<'static>,
    pub search: SlotOfBool<'static>,
    pub hide_show_columns: Vec<SlotOfInt<'static>>,
    pub freeze_columns: Vec<SlotOfInt<'static>>,
}

//-------------------------------------------------------------------------------//
//                             Implementations
//-------------------------------------------------------------------------------//

/// Implementation for `PackedFileTableViewSlots`.
impl PackedFileTableViewSlots {

    /// This function creates the entire slot pack for images.
    pub unsafe fn new(
        packed_file_view: &PackedFileTableViewRaw,
        global_search_ui: GlobalSearchUI,
        mut pack_file_contents_ui: PackFileContentsUI,
        packed_file_path: &Rc<RefCell<Vec<String>>>,
        table_definition: &Definition,
    ) -> Self {

        // When we want to filter the table...
        let filter_line_edit = SlotOfQString::new(clone!(
            mut packed_file_view => move |_| {
            packed_file_view.filter_table();
        }));

        let filter_column_selector = SlotOfInt::new(clone!(
            mut packed_file_view => move |_| {
            packed_file_view.filter_table();
        }));

        let filter_case_sensitive_button = Slot::new(clone!(
            mut packed_file_view => move || {
            packed_file_view.filter_table();
        }));

        // When we want to toggle the lookups on and off.
        let toggle_lookups = SlotOfBool::new(clone!(
            packed_file_view,
            table_definition => move |_| {
            packed_file_view.toggle_lookups(&table_definition);
        }));

        // When we want to show the context menu.
        let show_context_menu = SlotOfQPoint::new(clone!(
            mut packed_file_view => move |_| {
            packed_file_view.context_menu.exec_1a_mut(&QCursor::pos_0a());
        }));

        // When we want to trigger the context menu update function.
        let context_menu_enabler = SlotOfQItemSelectionQItemSelection::new(clone!(
            mut packed_file_view => move |_,_| {
            packed_file_view.context_menu_update();
        }));

        // When we want to respond to a change in one item in the model.
        let item_changed = SlotOfQStandardItem::new(clone!(
            mut packed_file_view => move |item| {

                // If we are NOT UNDOING, paint the item as edited and add the edition to the undo list.
                if !packed_file_view.undo_lock.load(Ordering::SeqCst) {

                    let mut edition = vec![];
                    let item_old = packed_file_view.undo_model.item_2a(item.row(), item.column());
                    edition.push(((item.row(), item.column()), atomic_from_mut_ptr((&*item_old).clone())));
                    let operation = TableOperations::Editing(edition);
                    packed_file_view.history_undo.write().unwrap().push(operation);
                    packed_file_view.history_redo.write().unwrap().clear();

                    {
                        // We block the saving for painting, so this doesn't get rettriggered again.
                        //let mut blocker = QSignalBlocker::from_q_object(packed_file_view.table_model);
                        let color = if SETTINGS.read().unwrap().settings_bool["use_dark_theme"] { GlobalColor::DarkYellow } else { GlobalColor::Yellow };
                        //item.set_background(&QBrush::from_global_color(color));
                        //blocker.unblock();
                    }

                    // For pasting, only update the undo_model the last iteration of the paste.
                    if packed_file_view.save_lock.load(Ordering::SeqCst) {
                        update_undo_model(packed_file_view.table_model, packed_file_view.undo_model);
                    }

                    packed_file_view.context_menu_update();
                }


/*
                // If we have the dependency stuff enabled, check if it's a valid reference.
                if SETTINGS.lock().unwrap().settings_bool["use_dependency_checker"] {
                    let column = unsafe { item.as_mut().unwrap().column() };
                    if table_definition.fields[column as usize].field_is_reference.is_some() {
                        Self::check_references(&dependency_data, column, item);
                    }
                }*/

                // If we are editing the Dependency Manager, check for PackFile errors too.
                //if let TableType::DependencyManager(_) = *table_type.borrow() { Self::check_dependency_packfile_errors(model); }
            }
        ));

        // When you want to append a row to the table...
        let add_rows = Slot::new(clone!(
            mut packed_file_view => move || {
                packed_file_view.append_rows(false);
            }
        ));

        // When you want to insert a row in a specific position of the table...
        let insert_rows = Slot::new(clone!(
            mut packed_file_view => move || {
                packed_file_view.insert_rows(false);
            }
        ));

        // When you want to delete one or more rows...
        let delete_rows = Slot::new(clone!(
            mut packed_file_view => move || {

                // Get all the selected rows.
                let selection = packed_file_view.table_view_primary.selection_model().selection();
                let indexes = packed_file_view.table_filter.map_selection_to_source(&selection).indexes();
                let indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
                let mut rows_to_delete: Vec<i32> = indexes_sorted.iter().filter_map(|x| if x.is_valid() { Some(x.row()) } else { None }).collect();

                // Dedup the list and reverse it.
                rows_to_delete.sort();
                rows_to_delete.dedup();
                rows_to_delete.reverse();
                let rows_splitted = delete_rows(packed_file_view.table_model, &rows_to_delete);

                // If we deleted something, try to save the PackedFile to the main PackFile.
                if !rows_to_delete.is_empty() {
                    packed_file_view.history_undo.write().unwrap().push(TableOperations::RemoveRows(rows_splitted));
                    packed_file_view.history_redo.write().unwrap().clear();
                    update_undo_model(packed_file_view.table_model, packed_file_view.undo_model);
                }
            }
        ));

        // When you want to clone and insert one or more rows.
        let clone_and_append = Slot::new(clone!(
            mut packed_file_view => move || {
            packed_file_view.append_rows(true);
        }));

        // When you want to clone and append one or more rows.
        let clone_and_insert = Slot::new(clone!(
            mut packed_file_view => move || {
            packed_file_view.insert_rows(true);
        }));

        // When you want to copy one or more cells.
        let copy = Slot::new(clone!(
            packed_file_view => move || {
            packed_file_view.copy_selection();
        }));

        // When you want to copy a table as a lua table.
        let copy_as_lua_table = Slot::new(clone!(
            packed_file_view => move || {
            packed_file_view.copy_selection_as_lua_table();
        }));

        // When you want to copy one or more cells.
        let paste = Slot::new(clone!(
            mut packed_file_view => move || {
            packed_file_view.paste();
        }));

        // When we want to invert the selection of the table.
        let invert_selection = Slot::new(clone!(
            mut packed_file_view => move || {
            let rows = packed_file_view.table_filter.row_count_0a();
            let columns = packed_file_view.table_filter.column_count_0a();
            if rows > 0 && columns > 0 {
                let mut selection_model = packed_file_view.table_view_primary.selection_model();
                let first_item = packed_file_view.table_filter.index_2a(0, 0);
                let last_item = packed_file_view.table_filter.index_2a(rows - 1, columns - 1);
                let selection = QItemSelection::new_2a(&first_item, &last_item);
                selection_model.select_q_item_selection_q_flags_selection_flag(&selection, QFlags::from(SelectionFlag::Toggle));
            }
        }));

        // When we want to reset the selected items of the table to their original value.
        let reset_selection = Slot::new(clone!(
            mut packed_file_view => move || {
            packed_file_view.reset_selection();
        }));

        // When we want to save the contents of the UI to the backend...
        //
        // NOTE: in-edition saves to backend are only triggered when the GlobalSearch has search data, to keep it updated.
        let save = Slot::new(clone!(
            packed_file_path,
            packed_file_view => move || {
            if !UI_STATE.get_global_search_no_lock().pattern.is_empty() {
                if let Some(packed_file) = UI_STATE.get_open_packedfiles().get(&*packed_file_path.borrow()) {
                    if let Err(error) = packed_file.save(&packed_file_path.borrow(), global_search_ui, &mut pack_file_contents_ui) {
                        show_dialog(packed_file_view.table_view_primary, error, false);
                    }
                }
            }
        }));

        // When we want to undo the last action.
        let undo = Slot::new(clone!(
            mut packed_file_view => move || {
                packed_file_view.undo_redo(true, 0);
                update_undo_model(packed_file_view.table_model, packed_file_view.undo_model);
                packed_file_view.context_menu_update();
            }
        ));

        // When we want to redo the last undone action.
        let redo = Slot::new(clone!(
            mut packed_file_view => move || {
                packed_file_view.undo_redo(false, 0);
                update_undo_model(packed_file_view.table_model, packed_file_view.undo_model);
                packed_file_view.context_menu_update();
            }
        ));

        // When we want to import a TSV file.
        let import_tsv = SlotOfBool::new(clone!(
            mut packed_file_path,
            mut packed_file_view => move |_| {

                // Create a File Chooser to get the destination path and configure it.
                let mut file_dialog = QFileDialog::from_q_widget_q_string(
                    packed_file_view.table_view_primary,
                    &QString::from_std_str("Select TSV File to Import..."),
                );

                file_dialog.set_name_filter(&QString::from_std_str("TSV Files (*.tsv)"));

                // Run it and, if we receive 1 (Accept), try to import the TSV file.
                if file_dialog.exec() == 1 {
                    let path = PathBuf::from(file_dialog.selected_files().at(0).to_std_string());

                    CENTRAL_COMMAND.send_message_qt(Command::ImportTSV((packed_file_path.borrow().to_vec(), path)));
                    let response = CENTRAL_COMMAND.recv_message_qt_try();
                    match response {
                        Response::TableType(data) => {
                            let old_data = packed_file_view.get_copy_of_table();

                            packed_file_view.undo_lock.store(true, Ordering::SeqCst);
                            packed_file_view.load_data(&data);
                            let table_name = match data {
                                TableType::DB(_) => packed_file_path.borrow()[1].to_string(),
                                _ => "".to_owned(),
                            };
                            packed_file_view.build_columns(&table_name);
                            packed_file_view.undo_lock.store(false, Ordering::SeqCst);

                            packed_file_view.history_undo.write().unwrap().push(TableOperations::ImportTSV(old_data));
                            packed_file_view.history_redo.write().unwrap().clear();
                            update_undo_model(packed_file_view.table_model, packed_file_view.undo_model);
                        },
                        Response::Error(error) => return show_dialog(packed_file_view.table_view_primary, error, false),
                        _ => panic!("{}{:?}", THREADS_COMMUNICATION_ERROR, response),
                    }

                    //unsafe { update_search_stuff.as_mut().unwrap().trigger(); }
                    packed_file_view.context_menu_update();
                }
            }
        ));

        // When we want to export the table as a TSV File.
        let export_tsv = SlotOfBool::new(clone!(
            packed_file_path,
            packed_file_view => move |_| {

                // Create a File Chooser to get the destination path and configure it.
                let mut file_dialog = QFileDialog::from_q_widget_q_string(
                    packed_file_view.table_view_primary,
                    &QString::from_std_str("Export TSV File..."),
                );

                file_dialog.set_accept_mode(AcceptMode::AcceptSave);
                file_dialog.set_confirm_overwrite(true);
                file_dialog.set_name_filter(&QString::from_std_str("TSV Files (*.tsv)"));
                file_dialog.set_default_suffix(&QString::from_std_str("tsv"));

                // Run it and, if we receive 1 (Accept), export the DB Table, saving it's contents first.
                if file_dialog.exec() == 1 {

                    let path = PathBuf::from(file_dialog.selected_files().at(0).to_std_string());
                    if let Some(packed_file) = UI_STATE.get_open_packedfiles().get(&*packed_file_path.borrow()) {
                        if let Err(error) = packed_file.save(&packed_file_path.borrow(), global_search_ui, &mut pack_file_contents_ui) {
                            return show_dialog(packed_file_view.table_view_primary, error, false);
                        }
                    }

                    CENTRAL_COMMAND.send_message_qt(Command::ExportTSV((packed_file_path.borrow().to_vec(), path)));
                    let response = CENTRAL_COMMAND.recv_message_qt_try();
                    match response {
                        Response::Success => return,
                        Response::Error(error) => return show_dialog(packed_file_view.table_view_primary, error, false),
                        _ => panic!("{}{:?}", THREADS_COMMUNICATION_ERROR, response),
                    }
                }
            }
        ));

        // When you want to use the "Smart Delete" feature...
        let smart_delete = Slot::new(clone!(
            mut packed_file_view => move || {

                // Get the selected indexes, the split them in two groups: one with full rows selected and another with single cells selected.
                let indexes = packed_file_view.table_view_primary.selection_model().selection().indexes();
                let mut indexes_sorted = (0..indexes.count_0a()).map(|x| indexes.at(x)).collect::<Vec<Ref<QModelIndex>>>();
                sort_indexes_visually(&mut indexes_sorted, packed_file_view.table_view_primary);
                let indexes_sorted = get_real_indexes(&indexes_sorted, packed_file_view.table_filter);

                let mut cells: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
                for model_index in &indexes_sorted {
                    if model_index.is_valid() {
                        let row = model_index.row();
                        let column = model_index.column();

                        // Check if we have any cell in that row and add/insert the new one.
                        match cells.get_mut(&row) {
                            Some(row) => row.push(column),
                            None => { cells.insert(row, vec![column]); },
                        }
                    }
                }

                let full_rows = cells.iter()
                    .filter(|(_, y)| y.len() as i32 == packed_file_view.table_model.column_count_0a())
                    .map(|(x, _)| *x)
                    .collect::<Vec<i32>>();

                let individual_cells = cells.iter()
                    .filter(|(_, y)| y.len() as i32 != packed_file_view.table_model.column_count_0a())
                    .map(|(x, y)| (*x, y.to_vec()))
                    .collect::<Vec<(i32, Vec<i32>)>>();

                // First, we do the editions. This means:
                // - Checkboxes: unchecked.
                // - Numbers: 0.
                // - Strings: empty.
                let mut editions = 0;
                for (row, columns) in &individual_cells {
                    for column in columns {
                        let mut item = packed_file_view.table_model.item_2a(*row, *column);
                        let current_value = item.text().to_std_string();
                        match packed_file_view.table_definition.fields[*column as usize].field_type {
                            FieldType::Boolean => {
                                let current_value = item.check_state();
                                if current_value != CheckState::Unchecked {
                                    item.set_check_state(CheckState::Unchecked);
                                    editions += 1;
                                }
                            }

                            FieldType::Float => {
                                if !current_value.is_empty() {
                                    item.set_data_2a(&QVariant::from_float(0.0f32), 2);
                                    editions += 1;
                                }
                            }

                            FieldType::Integer => {
                                if !current_value.is_empty() {
                                    item.set_data_2a(&QVariant::from_int(0i32), 2);
                                    editions += 1;
                                }
                            }

                            FieldType::LongInteger => {
                                if !current_value.is_empty() {
                                    item.set_data_2a(&QVariant::from_i64(0i64), 2);
                                    editions += 1;
                                }
                            }

                            _ => {
                                if !current_value.is_empty() {
                                    item.set_text(&QString::from_std_str(""));
                                    editions += 1;
                                }
                            }
                        }
                    }
                }

                // Then, we delete all the fully selected rows.
                let rows_splitted = super::utils::delete_rows(packed_file_view.table_model, &full_rows);

                // Then, we have to fix the undo history. For that, we take out all the editions, merge them,
                // then merge them with the table edition into a carolina.
                if editions > 0 || !rows_splitted.is_empty() {

                    // Update the search stuff, if needed.
                    //unsafe { update_search_stuff.as_mut().unwrap().trigger(); }

                     {
                        let mut changes = vec![];
                        if !rows_splitted.is_empty() {
                            changes.push(TableOperations::RemoveRows(rows_splitted));
                        }

                        let len = packed_file_view.history_undo.read().unwrap().len();
                        let editions: Vec<((i32, i32), AtomicPtr<QStandardItem>)> = packed_file_view.history_undo.write().unwrap()
                            .drain(len - editions..)
                            .filter_map(|x| if let TableOperations::Editing(y) = x { Some(y) } else { None })
                            .flatten()
                            .collect();

                        if !editions.is_empty() {
                            changes.push(TableOperations::Editing(editions));
                        }

                        if !changes.is_empty() {
                            packed_file_view.history_undo.write().unwrap().push(TableOperations::Carolina(changes));
                            packed_file_view.history_redo.write().unwrap().clear();
                            update_undo_model(packed_file_view.table_model, packed_file_view.undo_model);
                            packed_file_view.context_menu_update();
                        }
                    }
                }
            }
        ));

        let sidebar = SlotOfBool::new(clone!(
            mut packed_file_view => move |_| {
            match packed_file_view.sidebar_scroll_area.is_visible() {
                true => packed_file_view.sidebar_scroll_area.hide(),
                false => packed_file_view.sidebar_scroll_area.show()
            }
        }));

        let search = SlotOfBool::new(clone!(
            mut packed_file_view => move |_| {
            match packed_file_view.search_widget.is_visible() {
                true => packed_file_view.search_widget.hide(),
                false => packed_file_view.search_widget.show()
            }
        }));


        let mut hide_show_columns = vec![];
        let mut freeze_columns = vec![];
        let mut fields = table_definition.fields.iter()
            .enumerate()
            .map(|(x, y)| (x as i32, y.ca_order))
            .collect::<Vec<(i32, i16)>>();
        fields.sort_by(|(_, a), (_, b)| a.cmp(&b));
        let ca_order = fields.iter().map(|x| x.0).collect::<Vec<i32>>();

        for index in ca_order {
            let hide_show_slot = SlotOfInt::new(clone!(
                mut packed_file_view => move |state| {
                    let state = if state == 2 { true } else { false };
                    packed_file_view.table_view_primary.set_column_hidden(index, state);
                }
            ));

            let freeze_slot = SlotOfInt::new(clone!(
                mut packed_file_view => move |_| {
                    toggle_freezer_safe(&mut packed_file_view.table_view_primary, index);
                }
            ));

            hide_show_columns.push(hide_show_slot);
            freeze_columns.push(freeze_slot);
        }


        // Return the slots, so we can keep them alive for the duration of the view.
        Self {
            filter_line_edit,
            filter_column_selector,
            filter_case_sensitive_button,
            toggle_lookups,
            show_context_menu,
            context_menu_enabler,
            item_changed,
            add_rows,
            insert_rows,
            delete_rows,
            clone_and_append,
            clone_and_insert,
            copy,
            copy_as_lua_table,
            paste,
            invert_selection,
            reset_selection,
            save,
            undo,
            redo,
            import_tsv,
            export_tsv,
            smart_delete,
            sidebar,
            search,
            hide_show_columns,
            freeze_columns
        }
    }
}
