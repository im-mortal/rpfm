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
Module with all the code to interact with any kind of table data.

This module contains the struct `Table`, used to manage the decoded data of a table. For internal use only.
!*/

use bincode::serialize;
use csv::{QuoteStyle, ReaderBuilder, WriterBuilder};
use serde_derive::{Serialize, Deserialize};

use std::collections::BTreeMap;
use std::{fmt, fmt::Display};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use rpfm_error::{Error, ErrorKind, Result};

use crate::assembly_kit::table_data::RawTable;
use crate::common::{decoder::Decoder, encoder::Encoder, parse_str_as_bool};
use crate::schema::*;

pub mod animtable;
pub mod anim_fragment;
pub mod db;
pub mod loc;
pub mod matched_combat;

//---------------------------------------------------------------------------//
//                              Enum & Structs
//---------------------------------------------------------------------------//

/// This struct contains the data of a Table-like PackedFile after being decoded.
///
/// This is for internal use. If you need to interact with this in any way, do it through the PackedFile that contains it, not directly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {

    /// A copy of the `Definition` this table uses, so we don't have to check the schema everywhere.
    definition: Definition,

    /// The decoded entries of the table. This list is a Vec(rows) of a Vec(fields of a row) of DecodedData (decoded field).
    entries: Vec<Vec<DecodedData>>,
}

/// This enum is used to store different types of data in a unified way. Used, for example, to store the data from each field in a DB Table.
///
/// NOTE: `Sequence` it's a recursive type. A Sequence/List means you got a repeated sequence of fields
/// inside a single field. Used, for example, in certain model tables.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DecodedData {
    Boolean(bool),
    F32(f32),
    I16(i16),
    I32(i32),
    I64(i64),
    StringU8(String),
    StringU16(String),
    OptionalStringU8(String),
    OptionalStringU16(String),
    SequenceU16(Table),
    SequenceU32(Table)
}

/// This holds the dependency data for a specific column of a table.
#[derive(PartialEq, Clone, Default, Debug, Serialize, Deserialize)]
pub struct DependencyData {

    /// If the table is only present in the Ak. Useful to identify unused tables on diagnostics checks.
    pub referenced_table_is_ak_only: bool,

    /// If the referenced column has been moved into a loc file while exporting it from Dave.
    pub referenced_column_is_localised: bool,

    /// The data itself, as in "key, lookup" format.
    pub data: BTreeMap<String, String>,
}

//----------------------------------------------------------------//
// Implementations for `DecodedData`.
//----------------------------------------------------------------//

/// Display implementation of `DecodedData`.
impl Display for DecodedData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecodedData::Boolean(_) => write!(f, "Boolean"),
            DecodedData::F32(_) => write!(f, "F32"),
            DecodedData::I16(_) => write!(f, "I16"),
            DecodedData::I32(_) => write!(f, "I32"),
            DecodedData::I64(_) => write!(f, "I64"),
            DecodedData::StringU8(_) => write!(f, "StringU8"),
            DecodedData::StringU16(_) => write!(f, "StringU16"),
            DecodedData::OptionalStringU8(_) => write!(f, "OptionalStringU8"),
            DecodedData::OptionalStringU16(_) => write!(f, "OptionalStringU16"),
            DecodedData::SequenceU16(_) => write!(f, "SequenceU16"),
            DecodedData::SequenceU32(_) => write!(f, "SequenceU32"),
        }
    }
}

/// PartialEq implementation of `DecodedData`. We need this implementation due to the float comparison being... special.
impl PartialEq for DecodedData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DecodedData::Boolean(x), DecodedData::Boolean(y)) => x == y,
            (DecodedData::F32(x), DecodedData::F32(y)) => ((x * 1_000_000f32).round() / 1_000_000f32) == ((y * 1_000_000f32).round() / 1_000_000f32),
            (DecodedData::I16(x), DecodedData::I16(y)) => x == y,
            (DecodedData::I32(x), DecodedData::I32(y)) => x == y,
            (DecodedData::I64(x), DecodedData::I64(y)) => x == y,
            (DecodedData::StringU8(x), DecodedData::StringU8(y)) => x == y,
            (DecodedData::StringU16(x), DecodedData::StringU16(y)) => x == y,
            (DecodedData::OptionalStringU8(x), DecodedData::OptionalStringU8(y)) => x == y,
            (DecodedData::OptionalStringU16(x), DecodedData::OptionalStringU16(y)) => x == y,
            (DecodedData::SequenceU16(x), DecodedData::SequenceU16(y)) => x == y,
            (DecodedData::SequenceU32(x), DecodedData::SequenceU32(y)) => x == y,
            _ => false
        }
    }
}

/// Implementation of `DecodedData`.
impl DecodedData {

    /// Default implementation of `DecodedData`.
    pub fn default(field_type: &FieldType, default_value: &Option<String>) -> Self {
        match default_value {
            Some(default_value) => match field_type {
                FieldType::Boolean => if let Ok(value) = parse_str_as_bool(default_value) { DecodedData::Boolean(value) } else { DecodedData::Boolean(false) },
                FieldType::F32 => if let Ok(value) = default_value.parse::<f32>() { DecodedData::F32(value) } else { DecodedData::F32(0.0) },
                FieldType::I16 => if let Ok(value) = default_value.parse::<i16>() { DecodedData::I16(value) } else { DecodedData::I16(0) },
                FieldType::I32 => if let Ok(value) = default_value.parse::<i32>() { DecodedData::I32(value) } else { DecodedData::I32(0) },
                FieldType::I64 => if let Ok(value) = default_value.parse::<i64>() { DecodedData::I64(value) } else { DecodedData::I64(0) },
                FieldType::StringU8 => DecodedData::StringU8(default_value.to_owned()),
                FieldType::StringU16 => DecodedData::StringU16(default_value.to_owned()),
                FieldType::OptionalStringU8 => DecodedData::OptionalStringU8(default_value.to_owned()),
                FieldType::OptionalStringU16 => DecodedData::OptionalStringU16(default_value.to_owned()),

                // For these two ignore the default value.
                FieldType::SequenceU16(definition) => DecodedData::SequenceU16(Table::new(definition)),
                FieldType::SequenceU32(definition) => DecodedData::SequenceU32(Table::new(definition)),
            }
            None => match field_type {
                FieldType::Boolean => DecodedData::Boolean(false),
                FieldType::F32 => DecodedData::F32(0.0),
                FieldType::I16 => DecodedData::I16(0),
                FieldType::I32 => DecodedData::I32(0),
                FieldType::I64 => DecodedData::I64(0),
                FieldType::StringU8 => DecodedData::StringU8("".to_owned()),
                FieldType::StringU16 => DecodedData::StringU16("".to_owned()),
                FieldType::OptionalStringU8 => DecodedData::OptionalStringU8("".to_owned()),
                FieldType::OptionalStringU16 => DecodedData::OptionalStringU16("".to_owned()),
                FieldType::SequenceU16(definition) => DecodedData::SequenceU16(Table::new(definition)),
                FieldType::SequenceU32(definition) => DecodedData::SequenceU32(Table::new(definition)),
            }
        }
    }

    /// This functions checks if the type of an specific `DecodedData` is the one it should have, according to the provided `FieldType`.
    pub fn is_field_type_correct(&self, field_type: &FieldType) -> bool {
        match self {
            DecodedData::Boolean(_) => field_type == &FieldType::Boolean,
            DecodedData::F32(_) => field_type == &FieldType::F32,
            DecodedData::I16(_) => field_type == &FieldType::I16,
            DecodedData::I32(_) => field_type == &FieldType::I32,
            DecodedData::I64(_) => field_type == &FieldType::I64,
            DecodedData::StringU8(_) => field_type == &FieldType::StringU8,
            DecodedData::StringU16(_) => field_type == &FieldType::StringU16,
            DecodedData::OptionalStringU8(_) => field_type == &FieldType::OptionalStringU8,
            DecodedData::OptionalStringU16(_) => field_type == &FieldType::OptionalStringU16,
            DecodedData::SequenceU16(_) => matches!(field_type, FieldType::SequenceU16(_)),
            DecodedData::SequenceU32(_) => matches!(field_type, FieldType::SequenceU32(_)),
        }
    }

    /// This function tries to convert the provided data to the provided fieldtype. This can fail in so many ways you should always check the result.
    ///
    /// NOTE: If you pass the same type as it already has, this becomes an expensive way of cloning.
    pub fn convert_between_types(&self, new_field_type: &FieldType) -> Result<Self> {
        match self {
            Self::Boolean(ref data) => match new_field_type {
                FieldType::Boolean => Ok(self.clone()),
                FieldType::F32 => Ok(Self::F32(if *data { 1.0 } else { 0.0 })),
                FieldType::I16 => Ok(Self::I16(if *data { 1 } else { 0 })),
                FieldType::I32 => Ok(Self::I32(if *data { 1 } else { 0 })),
                FieldType::I64 => Ok(Self::I64(if *data { 1 } else { 0 })),
                FieldType::StringU8 => Ok(Self::StringU8(data.to_string())),
                FieldType::StringU16 => Ok(Self::StringU16(data.to_string())),
                FieldType::OptionalStringU8 => Ok(Self::OptionalStringU8(data.to_string())),
                FieldType::OptionalStringU16 => Ok(Self::OptionalStringU16(data.to_string())),
                FieldType::SequenceU16(_) => Err(ErrorKind::Generic.into()),
                FieldType::SequenceU32(_) => Err(ErrorKind::Generic.into()),
            }

            Self::F32(ref data) => match new_field_type {
                FieldType::Boolean => Ok(Self::Boolean(data > &1.0)),
                FieldType::F32 => Ok(self.clone()),
                FieldType::I16 => Ok(Self::I16(*data as i16)),
                FieldType::I32 => Ok(Self::I32(*data as i32)),
                FieldType::I64 => Ok(Self::I64(*data as i64)),
                FieldType::StringU8 => Ok(Self::StringU8(data.to_string())),
                FieldType::StringU16 => Ok(Self::StringU16(data.to_string())),
                FieldType::OptionalStringU8 => Ok(Self::OptionalStringU8(data.to_string())),
                FieldType::OptionalStringU16 => Ok(Self::OptionalStringU16(data.to_string())),
                FieldType::SequenceU16(_) => Err(ErrorKind::Generic.into()),
                FieldType::SequenceU32(_) => Err(ErrorKind::Generic.into()),
            }

            Self::I16(ref data) => match new_field_type {
                FieldType::Boolean => Ok(Self::Boolean(data > &1)),
                FieldType::F32 => Ok(Self::F32(*data as f32)),
                FieldType::I16 => Ok(self.clone()),
                FieldType::I32 => Ok(Self::I32(*data as i32)),
                FieldType::I64 => Ok(Self::I64(*data as i64)),
                FieldType::StringU8 => Ok(Self::StringU8(data.to_string())),
                FieldType::StringU16 => Ok(Self::StringU16(data.to_string())),
                FieldType::OptionalStringU8 => Ok(Self::OptionalStringU8(data.to_string())),
                FieldType::OptionalStringU16 => Ok(Self::OptionalStringU16(data.to_string())),
                FieldType::SequenceU16(_) => Err(ErrorKind::Generic.into()),
                FieldType::SequenceU32(_) => Err(ErrorKind::Generic.into()),
            }

            Self::I32(ref data) => match new_field_type {
                FieldType::Boolean => Ok(Self::Boolean(data > &1)),
                FieldType::F32 => Ok(Self::F32(*data as f32)),
                FieldType::I16 => Ok(Self::I16(*data as i16)),
                FieldType::I32 => Ok(self.clone()),
                FieldType::I64 => Ok(Self::I64(*data as i64)),
                FieldType::StringU8 => Ok(Self::StringU8(data.to_string())),
                FieldType::StringU16 => Ok(Self::StringU16(data.to_string())),
                FieldType::OptionalStringU8 => Ok(Self::OptionalStringU8(data.to_string())),
                FieldType::OptionalStringU16 => Ok(Self::OptionalStringU16(data.to_string())),
                FieldType::SequenceU16(_) => Err(ErrorKind::Generic.into()),
                FieldType::SequenceU32(_) => Err(ErrorKind::Generic.into()),
            }

            Self::I64(ref data) => match new_field_type {
                FieldType::Boolean => Ok(Self::Boolean(data > &1)),
                FieldType::F32 => Ok(Self::F32(*data as f32)),
                FieldType::I16 => Ok(Self::I16(*data as i16)),
                FieldType::I32 => Ok(Self::I32(*data as i32)),
                FieldType::I64 => Ok(self.clone()),
                FieldType::StringU8 => Ok(Self::StringU8(data.to_string())),
                FieldType::StringU16 => Ok(Self::StringU16(data.to_string())),
                FieldType::OptionalStringU8 => Ok(Self::OptionalStringU8(data.to_string())),
                FieldType::OptionalStringU16 => Ok(Self::OptionalStringU16(data.to_string())),
                FieldType::SequenceU16(_) => Err(ErrorKind::Generic.into()),
                FieldType::SequenceU32(_) => Err(ErrorKind::Generic.into()),
            }

            Self::StringU8(ref data) |
            Self::StringU16(ref data) |
            Self::OptionalStringU8(ref data) |
            Self::OptionalStringU16(ref data) => match new_field_type {
                FieldType::Boolean => Ok(Self::Boolean(parse_str_as_bool(data)?)),
                FieldType::F32 => Ok(Self::F32(data.parse::<f32>()?)),
                FieldType::I16 => Ok(Self::I16(data.parse::<i16>()?)),
                FieldType::I32 => Ok(Self::I32(data.parse::<i32>()?)),
                FieldType::I64 => Ok(Self::I64(data.parse::<i64>()?)),
                FieldType::StringU8 => Ok(Self::StringU8(data.to_string())),
                FieldType::StringU16 => Ok(Self::StringU16(data.to_string())),
                FieldType::OptionalStringU8 => Ok(Self::OptionalStringU8(data.to_string())),
                FieldType::OptionalStringU16 => Ok(Self::OptionalStringU16(data.to_string())),
                FieldType::SequenceU16(_) => Err(ErrorKind::Generic.into()),
                FieldType::SequenceU32(_) => Err(ErrorKind::Generic.into()),
            }
            /*
            Self::SequenceU16(ref data) => match new_field_type {
                FieldType::SequenceU16(ref definition) => Ok(self.clone()),
                FieldType::SequenceU32(ref definition) => Err(ErrorKind::Generic.into()),
                _ => Err(ErrorKind::Generic.into()),
            }

            Self::SequenceU32(ref data) => match new_field_type {
                FieldType::SequenceU16(ref definition) => Err(ErrorKind::Generic.into()),
                FieldType::SequenceU32(ref definition) => Ok(self.clone()),
                _ => Err(ErrorKind::Generic.into()),
            }*/
            _ => Err(ErrorKind::Generic.into()),
        }
    }

    /// This function prints whatever you have in each variants to a String.
    pub fn data_to_string(&self) -> String {
        match self {
            DecodedData::Boolean(data) => data.to_string(),
            DecodedData::F32(data) => format!("{:.3}", data),
            DecodedData::I16(data) => data.to_string(),
            DecodedData::I32(data) => data.to_string(),
            DecodedData::I64(data) => data.to_string(),
            DecodedData::StringU8(data) |
            DecodedData::StringU16(data) |
            DecodedData::OptionalStringU8(data) |
            DecodedData::OptionalStringU16(data) => data.to_owned(),
            DecodedData::SequenceU16(_) => "SequenceU16".to_owned(),
            DecodedData::SequenceU32(_) => "SequenceU32".to_owned(),
        }
    }
}

//----------------------------------------------------------------//
// Implementations for `Table`.
//----------------------------------------------------------------//

/// Implementation of `Table`.
impl Table {

    /// This function creates a new Table from an existing definition.
    pub fn new(definition: &Definition) -> Self {
        Table {
            definition: definition.clone(),
            entries: vec![],
        }
    }

    /// This function returns a copy of the definition of this Table.
    pub fn get_definition(&self) -> Definition {
        self.definition.clone()
    }

    /// This function returns a reference to the definition of this Table.
    pub fn get_ref_definition(&self) -> &Definition {
        &self.definition
    }

    /// This function returns a copy of the entries of this Table.
    pub fn get_table_data(&self) -> Vec<Vec<DecodedData>> {
        self.entries.to_vec()
    }

    /// This function returns a reference to the entries of this Table.
    pub fn get_ref_table_data(&self) -> &[Vec<DecodedData>] {
        &self.entries
    }

    /// This function returns the amount of entries in this Table.
    pub fn get_entry_count(&self) -> usize {
        self.entries.len()
    }

    /// This function replaces the definition of this table with the one provided.
    ///
    /// This updates the table's data to follow the format marked by the new definition, so you can use it to *update* the version of your table.
    pub fn set_definition(&mut self, new_definition: &Definition) {

        // It's simple: we compare both schemas, and get the original and final positions of each column.
        // If a row is new, his original position is -1. If has been removed, his final position is -1.
        let mut positions: Vec<(i32, i32)> = vec![];
        for (new_pos, new_field) in new_definition.get_fields_processed().iter().enumerate() {
            if let Some(old_pos) = self.definition.get_fields_processed().iter().position(|x| x.get_name() == new_field.get_name()) {
                positions.push((old_pos as i32, new_pos as i32))
            } else { positions.push((-1, new_pos as i32)); }
        }

        // Then, for each field in the old definition, check if exists in the new one.
        for (old_pos, old_field) in self.definition.get_fields_processed().iter().enumerate() {
            if !new_definition.get_fields_processed().iter().any(|x| x.get_name() == old_field.get_name()) { positions.push((old_pos as i32, -1)); }
        }

        // We sort the columns by their destination.
        positions.sort_by_key(|x| x.1);

        // Then, we create the new data using the old one and the column changes.
        let mut new_entries: Vec<Vec<DecodedData>> = vec![];
        for row in &mut self.entries {
            let mut entry = vec![];
            for (old_pos, new_pos) in &positions {

                // If the new position is -1, it means the column got removed. We skip it.
                if *new_pos == -1 { continue; }

                // If the old position is -1, it means we got a new column. We need to get his type and create a `Default` field with it.
                else if *old_pos == -1 {
                    entry.push(DecodedData::default(&new_definition.get_fields_processed()[*new_pos as usize].get_ref_field_type(), &new_definition.get_fields_processed()[*new_pos as usize].get_default_value()));
                }

                // Otherwise, we got a moved column. Grab his field from the old data and put it in his new place.
                else {
                    entry.push(row[*old_pos as usize].clone());
                }
            }
            new_entries.push(entry);
        }

        // Then, we finally replace our definition and our data.
        self.definition = new_definition.clone();
        self.entries = new_entries;
    }

    /// This function replaces the data of this table with the one provided.
    ///
    /// This can (and will) fail if the data is not of the format defined by the definition of the table.
    pub fn set_table_data(&mut self, data: &[Vec<DecodedData>]) -> Result<()> {
        for row in data {

            // First, we need to make sure all rows we have are exactly what we expect.
            let fields_processed = self.definition.get_fields_processed();

            if row.len() != fields_processed.len() { return Err(ErrorKind::TableRowWrongFieldCount(fields_processed.len() as u32, row.len() as u32).into()) }
            for (index, cell) in row.iter().enumerate() {

                // Next, we need to ensure each file is of the type we expected.
                let field = if let Some(field) = fields_processed.get(index) { field } else { return Err(ErrorKind::Generic.into()) };
                if !cell.is_field_type_correct(field.get_ref_field_type()) {
                    return Err(ErrorKind::TableWrongFieldType(format!("{}", cell), format!("{}", field.get_ref_field_type())).into())
                }
            }
        }

        // If we passed all the checks, replace the data.
        self.entries = data.to_vec();
        Ok(())
    }

    /// This function decodes all the fields of a table from raw bytes.
    ///
    /// If return_incomplete == true, this function will return an error with the incompletely decoded table when it fails.
    fn decode(&mut self,
        data: &[u8],
        entry_count: u32,
        mut index: &mut usize,
        return_incomplete: bool,
    ) -> Result<()> {

        // Do not specify size here, because a badly written definition can end up triggering an OOM crash if we do.
        self.entries = vec![];
        for row in 0..entry_count {
            let mut decoded_row = Vec::with_capacity(self.definition.get_ref_fields().len());
            for column in 0..self.definition.get_ref_fields().len() {
                let field = &self.definition.get_ref_fields()[column];
                let decoded_cell = match field.get_ref_field_type() {
                    FieldType::Boolean => {
                        if let Ok(data) = data.decode_packedfile_bool(*index, &mut index) { Ok(DecodedData::Boolean(data)) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as a <b><i>Boolean</b></i> value: the value is not a boolean, or there are insufficient bytes left to decode it as a boolean value.</p>", row + 1, column + 1))) }
                    }
                    FieldType::F32 => {
                        if let Ok(data) = data.decode_packedfile_float_f32(*index, &mut index) { Ok(DecodedData::F32(data)) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as a <b><i>F32</b></i> value: the value is not a valid F32, or there are insufficient bytes left to decode it as a F32 value.</p>", row + 1, column + 1))) }
                    }
                    FieldType::I16 => {
                        if let Ok(data) = data.decode_packedfile_integer_i16(*index, &mut index) { Ok(DecodedData::I16(data)) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as a <b><i>I16</b></i> value: the value is not a valid I16, or there are insufficient bytes left to decode it as an I16 value.</p>", row + 1, column + 1))) }
                    }
                    FieldType::I32 => {
                        if let Ok(data) = data.decode_packedfile_integer_i32(*index, &mut index) { Ok(DecodedData::I32(data)) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as a <b><i>I32</b></i> value: the value is not a valid I32, or there are insufficient bytes left to decode it as an I32 value.</p>", row + 1, column + 1))) }
                    }
                    FieldType::I64 => {
                        if let Ok(data) = data.decode_packedfile_integer_i64(*index, &mut index) { Ok(DecodedData::I64(data)) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as a <b><i>I64</b></i> value: either the value is not a valid I64, or there are insufficient bytes left to decode it as an I64 value.</p>", row + 1, column + 1))) }
                    }
                    FieldType::StringU8 => {
                        if let Ok(data) = data.decode_packedfile_string_u8(*index, &mut index) { Ok(DecodedData::StringU8(Self::escape_special_chars(&data))) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as an <b><i>UTF-8 String</b></i> value: the value is not a valid UTF-8 String, or there are insufficient bytes left to decode it as an UTF-8 String.</p>", row + 1, column + 1))) }
                    }
                    FieldType::StringU16 => {
                        if let Ok(data) = data.decode_packedfile_string_u16(*index, &mut index) { Ok(DecodedData::StringU16(Self::escape_special_chars(&data))) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as an <b><i>UTF-16 String</b></i> value: the value is not a valid UTF-16 String, or there are insufficient bytes left to decode it as an UTF-16 String.</p>", row + 1, column + 1))) }
                    }
                    FieldType::OptionalStringU8 => {
                        if let Ok(data) = data.decode_packedfile_optional_string_u8(*index, &mut index) { Ok(DecodedData::OptionalStringU8(Self::escape_special_chars(&data))) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as an <b><i>Optional UTF-8 String</b></i> value: the value is not a valid Optional UTF-8 String, or there are insufficient bytes left to decode it as an Optional UTF-8 String.</p>", row + 1, column + 1))) }
                    }
                    FieldType::OptionalStringU16 => {
                        if let Ok(data) = data.decode_packedfile_optional_string_u16(*index, &mut index) { Ok(DecodedData::OptionalStringU16(Self::escape_special_chars(&data))) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to decode the <i><b>Row {}, Cell {}</b></i> as an <b><i>Optional UTF-16 String</b></i> value: the value is not a valid Optional UTF-16 String, or there are insufficient bytes left to decode it as an Optional UTF-16 String.</p>", row + 1, column + 1))) }
                    }

                    // This type is just a recursive type.
                    FieldType::SequenceU16(definition) => {
                        if let Ok(entry_count) = data.decode_packedfile_integer_u16(*index, &mut index) {
                            let mut sub_table = Table::new(definition);
                            sub_table.decode(&data, entry_count.into(), index, return_incomplete)?;
                            Ok(DecodedData::SequenceU16(sub_table)) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to get the Entry Count of<i><b>Row {}, Cell {}</b></i>: the value is not a valid U32, or there are insufficient bytes left to decode it as an U32 value.</p>", row + 1, column + 1))) }
                    }

                    // This type is just a recursive type.
                    FieldType::SequenceU32(definition) => {
                        if let Ok(entry_count) = data.decode_packedfile_integer_u32(*index, &mut index) {
                            let mut sub_table = Table::new(definition);
                            sub_table.decode(&data, entry_count, index, return_incomplete)?;
                            Ok(DecodedData::SequenceU32(sub_table)) }
                        else { Err(ErrorKind::HelperDecodingEncodingError(format!("<p>Error trying to get the Entry Count of<i><b>Row {}, Cell {}</b></i>: the value is not a valid U32, or there are insufficient bytes left to decode it as an U32 value.</p>", row + 1, column + 1))) }
                    }
                };

                match decoded_cell {
                    Ok(data) =>  {

                        // If the field is a bitwise, split it into multiple fields. This is currently limited to integer types.
                        if field.get_is_bitwise() > 1 {
                            let data = match data {
                                DecodedData::I16(ref data) => *data as i64,
                                DecodedData::I32(ref data) => *data as i64,
                                DecodedData::I64(ref data) => *data,
                                _ => return Err(ErrorKind::Generic.into())
                            };

                            for bitwise_column in 0..field.get_is_bitwise() {
                                decoded_row.push(DecodedData::Boolean(data & (1 << bitwise_column) != 0));
                            }
                        }

                        // If the field has enum values, we turn it into a string. Same as before, only for integer types.
                        else if !field.get_enum_values().is_empty() {
                            let data = match data {
                                DecodedData::I16(ref data) => *data as i32,
                                DecodedData::I32(ref data) => *data,
                                DecodedData::I64(ref data) => *data as i32,
                                _ => return Err(ErrorKind::Generic.into())
                            };

                            match field.get_enum_values().get(&data) {
                                Some(data) => decoded_row.push(DecodedData::StringU8(data.to_owned())),
                                None => decoded_row.push(DecodedData::StringU8(data.to_string()))
                            }
                        }

                        else {
                            decoded_row.push(data);
                        }
                    },
                    Err(error) => if return_incomplete { return Err(ErrorKind::TableIncompleteError(format!("{}", error), serialize(self)?).into()) }
                    else { return Err(error.into()) }
                }
            }
            self.entries.push(decoded_row);
        }
        Ok(())
    }

    /// This function encodes all the fields of a table to raw bytes.
    fn encode(&self, mut packed_file: &mut Vec<u8>) -> Result<()> {
        let fields = self.definition.get_ref_fields();
        let fields_processed = self.definition.get_fields_processed();
        for row in &self.entries {

            // First, we need to make sure all rows we're going to encode are exactly what we expect.
            if row.len() != fields_processed.len() { return Err(ErrorKind::TableRowWrongFieldCount(fields_processed.len() as u32, row.len() as u32).into()) }
            let mut data_column = 0;
            for field in fields {

                if field.get_is_bitwise() > 1 {
                    let mut data: i64 = 0;
                    for bitwise_column in 0..field.get_is_bitwise() {
                        if let DecodedData::Boolean(boolean) = row[data_column] {
                            if boolean {
                                data |= 1 << bitwise_column;
                            }
                        }

                        else {
                            return Err(ErrorKind::TableWrongFieldType(format!("{}", row[data_column]), format!("{}", field.get_ref_field_type())).into())
                        }

                        data_column += 1;
                    }

                    // If there are no problems, encode the data.
                    match field.get_field_type() {
                        FieldType::I16 => packed_file.encode_integer_i16(data as i16),
                        FieldType::I32 => packed_file.encode_integer_i32(data as i32),
                        FieldType::I64 => packed_file.encode_integer_i64(data),
                        _ => return Err(ErrorKind::TableWrongFieldType(format!("{}", row[data_column]), format!("{}", field.get_ref_field_type())).into())
                    }
                }

                else {

                    match row[data_column] {
                        DecodedData::Boolean(data) => packed_file.encode_bool(data),
                        DecodedData::F32(data) => packed_file.encode_float_f32(data),
                        DecodedData::I16(data) => packed_file.encode_integer_i16(data),
                        DecodedData::I32(data) => packed_file.encode_integer_i32(data),
                        DecodedData::I64(data) => packed_file.encode_integer_i64(data),
                        DecodedData::StringU8(ref data) |
                        DecodedData::StringU16(ref data) |
                        DecodedData::OptionalStringU8(ref data) |
                        DecodedData::OptionalStringU16(ref data) => {

                            // If the field has enum values, try to match them. If the matching fails, try to just encode them.
                            // If that fails, put a default value on that cell.
                            let values = field.get_enum_values();
                            if !values.is_empty() {
                                let data = match values.iter().find(|(_, y)| y.to_lowercase() == data.to_lowercase()) {
                                    Some((x, _)) => {
                                        match field.get_field_type() {
                                            FieldType::I16 => DecodedData::I16(*x as i16),
                                            FieldType::I32 => DecodedData::I32(*x),
                                            FieldType::I64 => DecodedData::I64(*x as i64),
                                            _ => return Err(ErrorKind::TableWrongFieldType(format!("{}", row[data_column]), format!("{}", field.get_ref_field_type())).into())
                                        }
                                    }
                                    None => match row[data_column].convert_between_types(field.get_ref_field_type()) {
                                        Ok(data) => data,
                                        Err(_) => DecodedData::default(field.get_ref_field_type(), field.get_default_value())
                                    }
                                };

                                // If there are no problems, encode the data.
                                match data {
                                    DecodedData::I16(data) => packed_file.encode_integer_i16(data),
                                    DecodedData::I32(data) => packed_file.encode_integer_i32(data),
                                    DecodedData::I64(data) => packed_file.encode_integer_i64(data),
                                    _ => return Err(ErrorKind::TableWrongFieldType(format!("{}", row[data_column]), format!("{}", field.get_ref_field_type())).into())
                                }
                            }
                            else {

                                // If there are no problems, encode the data.
                                match row[data_column] {
                                    DecodedData::StringU8(ref data) => packed_file.encode_packedfile_string_u8(&Self::unescape_special_chars(&data)),
                                    DecodedData::StringU16(ref data) => packed_file.encode_packedfile_string_u16(&Self::unescape_special_chars(&data)),
                                    DecodedData::OptionalStringU8(ref data) => packed_file.encode_packedfile_optional_string_u8(&Self::unescape_special_chars(&data)),
                                    DecodedData::OptionalStringU16(ref data) => packed_file.encode_packedfile_optional_string_u16(&Self::unescape_special_chars(&data)),
                                    _ => return Err(ErrorKind::TableWrongFieldType(format!("{}", row[data_column]), format!("{}", field.get_ref_field_type())).into())
                                }
                            }
                        }
                        DecodedData::SequenceU16(ref data) => {
                            if let FieldType::SequenceU16(_) = fields[data_column].get_ref_field_type() {
                                packed_file.encode_integer_u16(data.entries.len() as u16);
                                data.encode(&mut packed_file)?;
                            }
                        },
                        DecodedData::SequenceU32(ref data) => {
                            if let FieldType::SequenceU32(_) = fields[data_column].get_ref_field_type() {
                                packed_file.encode_integer_u32(data.entries.len() as u32);
                                data.encode(&mut packed_file)?;
                            }
                        },
                    }

                    data_column += 1;
                }
            }
        }

        Ok(())
    }

    /// This function returns a new empty row for the provided definition.
    pub fn get_new_row(definition: &Definition) -> Vec<DecodedData> {
        definition.get_ref_fields().iter()
            .map(|field|
                match field.get_ref_field_type() {
                    FieldType::Boolean => {
                        if let Some(default_value) = field.get_default_value() {
                            if default_value.to_lowercase() == "true" {
                                vec![DecodedData::Boolean(true)]
                            } else {
                                vec![DecodedData::Boolean(false)]
                            }
                        } else {
                            vec![DecodedData::Boolean(false)]
                        }
                    }
                    FieldType::F32 => {
                        if let Some(default_value) = field.get_default_value() {
                            if let Ok(default_value) = default_value.parse::<f32>() {
                                vec![DecodedData::F32(default_value); 1]
                            } else {
                                vec![DecodedData::F32(0.0); 1]
                            }
                        } else {
                            vec![DecodedData::F32(0.0); 1]
                        }
                    },
                    FieldType::I16 => {
                        if field.get_is_bitwise() > 1 {
                            vec![DecodedData::Boolean(false); field.get_is_bitwise() as usize]
                        }
                        else if let Some(default_value) = field.get_default_value() {
                            if let Ok(default_value) = default_value.parse::<i16>() {
                                vec![DecodedData::I16(default_value); 1]
                            } else {
                                vec![DecodedData::I16(0); 1]
                            }
                        } else {
                            vec![DecodedData::I16(0); 1]
                        }
                    },
                    FieldType::I32 => {
                        if field.get_is_bitwise() > 1 {
                            vec![DecodedData::Boolean(false); field.get_is_bitwise() as usize]
                        }
                        else if let Some(default_value) = field.get_default_value() {
                            if let Ok(default_value) = default_value.parse::<i32>() {
                                vec![DecodedData::I32(default_value); 1]
                            } else {
                                vec![DecodedData::I32(0); 1]
                            }
                        } else {
                            vec![DecodedData::I32(0); 1]
                        }
                    },
                    FieldType::I64 => {
                        if field.get_is_bitwise() > 1 {
                            vec![DecodedData::Boolean(false); field.get_is_bitwise() as usize]
                        }
                        else if let Some(default_value) = field.get_default_value() {
                            if let Ok(default_value) = default_value.parse::<i64>() {
                                vec![DecodedData::I64(default_value); 1]
                            } else {
                                vec![DecodedData::I64(0); 1]
                            }
                        } else {
                            vec![DecodedData::I64(0); 1]
                        }
                    },
                    FieldType::StringU8 => {
                        if let Some(default_value) = field.get_default_value() {
                            vec![DecodedData::StringU8(default_value.to_owned()); 1]
                        } else {
                            vec![DecodedData::StringU8(String::new()); 1]
                        }
                    }
                    FieldType::StringU16 => {
                        if let Some(default_value) = field.get_default_value() {
                            vec![DecodedData::StringU16(default_value.to_owned()); 1]
                        } else {
                            vec![DecodedData::StringU16(String::new()); 1]
                        }
                    }
                    FieldType::OptionalStringU8 => {
                        if let Some(default_value) = field.get_default_value() {
                            vec![DecodedData::OptionalStringU8(default_value.to_owned()); 1]
                        } else {
                            vec![DecodedData::OptionalStringU8(String::new()); 1]
                        }
                    }
                    FieldType::OptionalStringU16 => {
                        if let Some(default_value) = field.get_default_value() {
                            vec![DecodedData::OptionalStringU16(default_value.to_owned()); 1]
                        } else {
                            vec![DecodedData::OptionalStringU16(String::new()); 1]
                        }
                    },
                    FieldType::SequenceU16(ref definition) => vec![DecodedData::SequenceU16(Table::new(&definition)); 1],
                    FieldType::SequenceU32(ref definition) => vec![DecodedData::SequenceU32(Table::new(&definition)); 1]
                }
            )
            .flatten()
            .collect()

    }

    /// This function returns the list of table/columns that reference the provided columns, and if there may be a loc entry that changing our column may need a change.
    ///
    /// This supports more than one reference level, except for locs.
    /// TODO: Make loc editions be as deep as needed.
    pub fn get_tables_and_columns_referencing_our_own(
        schema_option: &Option<Schema>,
        table_name: &str,
        column_name: &str,
        definition: &Definition
    ) -> Option<(BTreeMap<String, Vec<String>>, bool)> {
        if let Some(ref schema) = *schema_option {

            // Make sure the table name is correct.
            let short_table_name = if table_name.ends_with("_tables") { table_name.split_at(table_name.len() - 7).0 } else { table_name };
            let mut tables: BTreeMap<String, Vec<String>> = BTreeMap::new();

            // We get all the db definitions from the schema, then iterate all of them to find what tables/columns reference our own.
            for versioned_file in schema.get_ref_versioned_file_db_all() {
                if let VersionedFile::DB(ref_table_name, ref_definition) = versioned_file {
                    let mut columns: Vec<String> = vec![];
                    for ref_version in ref_definition {
                        for ref_field in ref_version.get_fields_processed() {
                            if let Some((ref_ref_table, ref_ref_field)) = ref_field.get_is_reference() {

                                // As this applies to all versions of a table, skip repeated fields.
                                if ref_ref_table == short_table_name && ref_ref_field == column_name && !columns.iter().any(|x| x == ref_field.get_name()) {
                                    columns.push(ref_field.get_name().to_owned());

                                    // If we find a referencing column, get recursion working to check if there is any column referencing this one that needs to be edited.
                                    if let Some((ref_of_ref, _)) = Self::get_tables_and_columns_referencing_our_own(schema_option, ref_table_name, ref_field.get_name(), ref_version) {
                                        for refs in &ref_of_ref {
                                            match tables.get_mut(refs.0) {
                                                Some(columns) => for value in refs.1 {
                                                    if !columns.contains(value) {
                                                        columns.push(value.to_owned());
                                                    }
                                                }
                                                None => { tables.insert(refs.0.to_owned(), refs.1.to_vec()); },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Only add them if we actually found columns.
                    if !columns.is_empty() {
                        tables.insert(ref_table_name.to_owned(), columns);
                    }
                }
            }

            // Also, check if we have to be careful about localised fields.
            let has_loc_fields = if let Some(field) = definition.get_fields_processed().iter().find(|x| x.get_name() == column_name) {
                if (field.get_is_key() || field.get_name() == "key") && !definition.get_localised_fields().is_empty() {
                    true
                } else { false }
            } else { false };

            Some((tables, has_loc_fields))
        } else {
           None
        }
    }

    /// This function tries to find the row and column of the provided data, if it exists in this table.
    pub fn get_source_location_of_reference_data(
        &self,
        column_name: &str,
        row_data: &str
    ) -> Option<(usize, usize)> {
        if let Some(column_index) = self.get_ref_definition().get_fields_processed().iter().position(|x| x.get_name() == column_name) {
            for (row_index, row) in self.get_ref_table_data().iter().enumerate() {
                if let Some(cell_data) = row.get(column_index) {
                    if &cell_data.data_to_string() == row_data {
                        return Some((column_index, row_index))
                    }
                }
            }
        }

        None
    }

    //----------------------------------------------------------------//
    // TSV Functions for PackedFiles.
    //----------------------------------------------------------------//

    /// This function imports a TSV file into a decoded table.
    fn import_tsv(
        definition: &Definition,
        path: &PathBuf,
        name: &str,
    ) -> Result<Self> {

        // We want the reader to have no quotes, tab as delimiter and custom headers, because otherwise
        // Excel, Libreoffice and all the programs that edit this kind of files break them on save.
        let mut reader = ReaderBuilder::new()
            .delimiter(b'\t')
            .quoting(false)
            .has_headers(false)
            .flexible(true)
            .from_path(&path)?;

        // If we succesfully load the TSV file into a reader, check the first two lines to ensure
        // it's a valid TSV for our specific table.
        let mut entries = vec![];
        for (row, record) in reader.records().enumerate() {
            if let Ok(record) = record {

                // The first line should contain the "table_folder_name"/"Loc PackedFile/PackFile List", and the version (1 for Locs).
                // If it doesn't match with the name we provided, return an error.
                if row == 0 {
                    if record.get(0).unwrap_or("error") != name { return Err(ErrorKind::ImportTSVWrongTypeTable.into()); }
                    if record.get(1).unwrap_or("-1").parse::<i32>().map_err(|_| Error::from(ErrorKind::ImportTSVInvalidVersion))? != definition.get_version() {
                        return Err(ErrorKind::ImportTSVWrongVersion.into());
                    }
                }

                // The second line contains the column headers. Is just to help people in other programs, so we skip it.
                else if row == 1 { continue }

                // Then read the rest of the rows as a normal TSV.
                else if record.len() == definition.get_fields_processed().len() {
                    let mut entry = vec![];
                    for (column, field) in record.iter().enumerate() {
                        match definition.get_fields_processed()[column].get_ref_field_type() {
                            FieldType::Boolean => {
                                let value = field.to_lowercase();
                                if value == "true" || value == "1" { entry.push(DecodedData::Boolean(true)); }
                                else if value == "false" || value == "0" { entry.push(DecodedData::Boolean(false)); }
                                else { return Err(ErrorKind::ImportTSVIncorrectRow(row, column).into()); }
                            }
                            FieldType::F32 => entry.push(DecodedData::F32(field.parse::<f32>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::I16 => entry.push(DecodedData::I16(field.parse::<i16>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::I32 => entry.push(DecodedData::I32(field.parse::<i32>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::I64 => entry.push(DecodedData::I64(field.parse::<i64>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::StringU8 => entry.push(DecodedData::StringU8(field.to_owned())),
                            FieldType::StringU16 => entry.push(DecodedData::StringU16(field.to_owned())),
                            FieldType::OptionalStringU8 => entry.push(DecodedData::OptionalStringU8(field.to_owned())),
                            FieldType::OptionalStringU16 => entry.push(DecodedData::OptionalStringU16(field.to_owned())),

                            // For now fail on Sequences. These are a bit special and I don't know if the're even possible in TSV.
                            FieldType::SequenceU16(_) => return Err(ErrorKind::ImportTSVIncorrectRow(row, column).into()),
                            FieldType::SequenceU32(_) => return Err(ErrorKind::ImportTSVIncorrectRow(row, column).into())
                        }
                    }
                    entries.push(entry);
                }

                // If it fails here, return an error with the len of the record instead a field.
                else { return Err(ErrorKind::ImportTSVIncorrectRow(row, record.len()).into()); }
            }
            else { return Err(ErrorKind::ImportTSVIncorrectRow(row, 0).into()); }
        }

        // If we reached this point without errors, we replace the old data with the new one and return success.
        let mut table = Table::new(definition);
        table.entries = entries;
        Ok(table)
    }

    /// This function imports a TSV file into a new Table File.
    fn import_tsv_to_binary_file(
        schema: &Schema,
        source_path: &PathBuf,
        destination_path: &PathBuf,
    ) -> Result<()> {

        // We want the reader to have no quotes, tab as delimiter and custom headers, because otherwise
        // Excel, Libreoffice and all the programs that edit this kind of files break them on save.
        let mut reader = ReaderBuilder::new()
            .delimiter(b'\t')
            .quoting(false)
            .has_headers(true)
            .flexible(true)
            .from_path(&source_path)?;

        // If we succesfully load the TSV file into a reader, check the first line to ensure it's a valid TSV file.
        let table_type;
        let table_version;
        {
            let headers = reader.headers()?;
            table_type = if let Some(table_type) = headers.get(0) { table_type.to_owned() } else { return Err(ErrorKind::ImportTSVWrongTypeTable.into()) };
            table_version = if let Some(table_version) = headers.get(1) { table_version.parse::<i32>().map_err(|_| Error::from(ErrorKind::ImportTSVInvalidVersion))? } else { return Err(ErrorKind::ImportTSVInvalidVersion.into()) };
        }

        // Get his definition depending on his first line's contents.
        let definition = if table_type == loc::TSV_NAME_LOC { schema.get_ref_versioned_file_loc()?.get_version(table_version)?.clone() }
        else { schema.get_ref_versioned_file_db(&table_type)?.get_version(table_version)?.clone() };

        // Try to import the entries of the file.
        let mut entries = vec![];
        for (row, record) in reader.records().enumerate() {
            if let Ok(record) = record {

                // The second line contains the column headers. Is just to help people in other programs, not needed to be check.
                if row == 0 { continue }

                // Then read the rest of the rows as a normal TSV.
                else if record.len() == definition.get_fields_processed().len() {
                    let mut entry = vec![];
                    for (column, field) in record.iter().enumerate() {
                        match definition.get_fields_processed()[column].get_ref_field_type() {
                            FieldType::Boolean => {
                                let value = field.to_lowercase();
                                if value == "true" || value == "1" { entry.push(DecodedData::Boolean(true)); }
                                else if value == "false" || value == "0" { entry.push(DecodedData::Boolean(false)); }
                                else { return Err(ErrorKind::ImportTSVIncorrectRow(row, column).into()); }
                            }
                            FieldType::F32 => entry.push(DecodedData::F32(field.parse::<f32>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::I16 => entry.push(DecodedData::I16(field.parse::<i16>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::I32 => entry.push(DecodedData::I32(field.parse::<i32>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::I64 => entry.push(DecodedData::I64(field.parse::<i64>().map_err(|_| Error::from(ErrorKind::ImportTSVIncorrectRow(row, column)))?)),
                            FieldType::StringU8 => entry.push(DecodedData::StringU8(field.to_owned())),
                            FieldType::StringU16 => entry.push(DecodedData::StringU16(field.to_owned())),
                            FieldType::OptionalStringU8 => entry.push(DecodedData::OptionalStringU8(field.to_owned())),
                            FieldType::OptionalStringU16 => entry.push(DecodedData::OptionalStringU16(field.to_owned())),
                            FieldType::SequenceU16(_) |
                            FieldType::SequenceU32(_) => return Err(ErrorKind::ImportTSVIncorrectRow(row, column).into())
                        }
                    }
                    entries.push(entry);
                }

                // If it fails here, return an error with the len of the record instead a field.
                else { return Err(ErrorKind::ImportTSVIncorrectRow(row, record.len()).into()); }
            }

            else { return Err(ErrorKind::ImportTSVIncorrectRow(row, 0).into()); }
        }

        // If we reached this point without errors, we create the File in memory and add the entries to it.
        let data = if table_type == loc::TSV_NAME_LOC {
            let mut file = loc::Loc::new(&definition);
            file.set_table_data(&entries)?;
            file.save()
        }
        else {
            let mut file = db::DB::new(&table_type, None, &definition);
            file.set_table_data(&entries)?;
            file.save()
        }?;

        // Then, we try to write it on disk. If there is an error, report it.
        let mut file = BufWriter::new(File::create(&destination_path)?);
        file.write_all(&data)?;

        // If all worked, return success.
        Ok(())
    }

    /// This function exports the provided data to a TSV file.
    fn export_tsv(
        &self,
        path: &PathBuf,
        table_name: &str,
    ) -> Result<()> {

        // We want the writer to have no quotes, tab as delimiter and custom headers, because otherwise
        // Excel, Libreoffice and all the programs that edit this kind of files break them on save.
        let mut writer = WriterBuilder::new()
            .delimiter(b'\t')
            .quote_style(QuoteStyle::Never)
            .has_headers(false)
            .flexible(true)
            .from_writer(vec![]);

        // We serialize the info of the table (name and version) in the first line, and the column names in the second one.
        writer.serialize((table_name, self.definition.get_version()))?;
        writer.serialize(self.definition.get_ref_fields().iter().map(|x| x.get_name().to_owned()).collect::<Vec<String>>())?;

        // Then we serialize each entry in the DB Table.
        for entry in &self.entries { writer.serialize(&entry)?; }

        // Then, we try to write it on disk. If there is an error, report it.
        let mut file = File::create(&path)?;
        file.write_all(String::from_utf8(writer.into_inner().unwrap())?.as_bytes())?;

        Ok(())
    }

    /// This function exports the provided file to a TSV file..
    fn export_tsv_from_binary_file(
        schema: &Schema,
        source_path: &PathBuf,
        destination_path: &PathBuf
    ) -> Result<()> {

        // We want the writer to have no quotes, tab as delimiter and custom headers, because otherwise
        // Excel, Libreoffice and all the programs that edit this kind of files break them on save.
        let mut writer = WriterBuilder::new()
            .delimiter(b'\t')
            .quote_style(QuoteStyle::Never)
            .has_headers(false)
            .flexible(true)
            .from_path(destination_path)?;

        // We don't know what type this file is, so we try to decode it as a Loc. If that fails, we try
        // to decode it as a DB using the name of his parent folder. If that fails too, run before it explodes!
        let mut file = BufReader::new(File::open(source_path)?);
        let mut data = vec![];
        file.read_to_end(&mut data)?;

        let (table_type, version, entries) = if let Ok(data) = loc::Loc::read(&data, schema, false) {
            (loc::TSV_NAME_LOC, data.get_definition().get_version(), data.get_table_data())
        }
        else {
            let table_type = source_path.parent().unwrap().file_name().unwrap().to_str().unwrap();
            if let Ok(data) = db::DB::read(&data, table_type, schema, false) { (table_type, data.get_definition().get_version(), data.get_table_data()) }
            else { return Err(ErrorKind::ImportTSVWrongTypeTable.into()) }
        };

        let definition = if table_type == loc::TSV_NAME_LOC { schema.get_ref_versioned_file_loc()?.get_version(version)?.clone() }
        else { schema.get_ref_versioned_file_db(&table_type)?.get_version(version)?.clone() };

        // We serialize the info of the table (name and version) in the first line, and the column names in the second one.
        writer.serialize((&table_type, version))?;
        writer.serialize(definition.get_ref_fields().iter().map(|x| x.get_name().to_owned()).collect::<Vec<String>>())?;

        // Then we serialize each entry in the DB Table.
        for entry in entries { writer.serialize(&entry)?; }
        writer.flush().map_err(From::from)
    }

    /// This function escapes certain characters of the provided string.
    fn escape_special_chars(data: &str)-> String {
         let mut output = Vec::with_capacity(data.len() + 10);
         for c in data.as_bytes() {
            match c {
                b'\n' => output.extend_from_slice(b"\\\\n"),
                b'\t' => output.extend_from_slice(b"\\\\t"),
                _ => output.push(*c),
            }
        }
        unsafe { String::from_utf8_unchecked(output) }
    }

    /// This function unescapes certain characters of the provided string.
    fn unescape_special_chars(data: &str)-> String {
         data.replace("\\\\t", "\t").replace("\\\\n", "\n")
    }
}

/// Implementation of `From<&RawTable>` for `Table`.
impl From<&RawTable> for Table {
    fn from(raw_table: &RawTable) -> Self {
        if let Some(ref raw_definition) = raw_table.definition {
            let mut table = Self::new(&From::from(raw_definition));
            for row in &raw_table.rows {
                let mut entry = vec![];

                // Some games (Thrones, Attila, Rome 2 and Shogun 2) may have missing fields when said field is empty.
                // To compensate it, if we don't find a field from the definition in the table, we add it empty.
                for field_def in table.definition.get_ref_fields() {
                    let mut exists = false;
                    for field in &row.fields {
                        if field_def.get_name() == field.field_name {
                            exists = true;
                            entry.push(match field_def.get_ref_field_type() {
                                FieldType::Boolean => DecodedData::Boolean(field.field_data == "true" || field.field_data == "1"),
                                FieldType::F32 => DecodedData::F32(if let Ok(data) = field.field_data.parse::<f32>() { data } else { 0.0 }),
                                FieldType::I16 => DecodedData::I16(if let Ok(data) = field.field_data.parse::<i16>() { data } else { 0 }),
                                FieldType::I32 => DecodedData::I32(if let Ok(data) = field.field_data.parse::<i32>() { data } else { 0 }),
                                FieldType::I64 => DecodedData::I64(if let Ok(data) = field.field_data.parse::<i64>() { data } else { 0 }),
                                FieldType::StringU8 => DecodedData::StringU8(if field.field_data == "Frodo Best Waifu" { String::new() } else { field.field_data.to_string() }),
                                FieldType::StringU16 => DecodedData::StringU16(if field.field_data == "Frodo Best Waifu" { String::new() } else { field.field_data.to_string() }),
                                FieldType::OptionalStringU8 => DecodedData::OptionalStringU8(if field.field_data == "Frodo Best Waifu" { String::new() } else { field.field_data.to_string() }),
                                FieldType::OptionalStringU16 => DecodedData::OptionalStringU16(if field.field_data == "Frodo Best Waifu" { String::new() } else { field.field_data.to_string() }),

                                // This type is not used in the raw tables so, if we find it, we skip it.
                                FieldType::SequenceU16(_) | FieldType::SequenceU32(_) => continue,
                            });
                            break;
                        }
                    }

                    // If the field doesn't exist, we create it empty.
                    if !exists {
                        entry.push(DecodedData::OptionalStringU8(String::new()));
                    }
                }
                table.entries.push(entry);
            }
            table
        }
        else {
            Self::new(&Definition::new(-100))
        }
    }
}
