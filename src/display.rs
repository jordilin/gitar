use crate::Result;
use std::{fmt::Display, io::Write};

#[derive(Clone, Debug, Default)]
pub enum Format {
    CSV,
    #[default]
    PIPE,
}

impl Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::CSV => write!(f, ","),
            Format::PIPE => write!(f, " | "),
        }
    }
}

impl From<&Format> for &str {
    fn from(f: &Format) -> Self {
        match f {
            Format::CSV => ",",
            Format::PIPE => " | ",
        }
    }
}

pub struct DisplayBody {
    pub columns: Vec<Column>,
}

impl DisplayBody {
    pub fn new(columns: Vec<Column>) -> Self {
        Self { columns }
    }
}

pub struct Column {
    pub name: String,
    pub value: String,
}

impl Column {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

pub fn print<W: Write, D: Into<DisplayBody> + Clone>(
    w: &mut W,
    data: Vec<D>,
    no_headers: bool,
    format: &Format,
) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }
    if !no_headers {
        // Get the headers from the first row of columns
        let headers = data[0]
            .clone()
            .into()
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>();
        writeln!(w, "{}", headers.join(format.into()))?;
    }
    for d in data {
        let d = d.into();
        let num_columns = d.columns.len();
        for i in 0..num_columns {
            write!(w, "{}", d.columns[i].value)?;
            if i < num_columns - 1 {
                write!(w, "{}", format)?;
            }
        }
        writeln!(w)?;
    }
    Ok(())
}
