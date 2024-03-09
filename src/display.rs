use crate::Result;
use std::{collections::HashMap, fmt::Display, io::Write};

#[derive(Clone, Debug, Default)]
pub enum Format {
    CSV,
    JSON,
    #[default]
    PIPE,
}

impl Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::CSV => write!(f, ","),
            Format::PIPE => write!(f, " | "),
            Format::JSON => write!(f, ""),
        }
    }
}

impl From<&Format> for &str {
    fn from(f: &Format) -> Self {
        match f {
            Format::CSV => ",",
            Format::PIPE => " | ",
            Format::JSON => "",
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
    match format {
        Format::JSON => {
            for d in data {
                let d = d.into();
                let kvs: HashMap<String, String> = d
                    .columns
                    .into_iter()
                    .map(|item| (item.name, item.value))
                    .collect();
                writeln!(w, "{}", serde_json::to_string(&kvs)?)?;
            }
        }
        _ => {
            // CSV and PIPE (" | ") formats
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
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone)]
    struct Book {
        pub title: String,
        pub author: String,
    }

    impl Book {
        pub fn new(title: impl Into<String>, author: impl Into<String>) -> Self {
            Self {
                title: title.into(),
                author: author.into(),
            }
        }
    }

    impl From<Book> for DisplayBody {
        fn from(b: Book) -> Self {
            DisplayBody::new(vec![
                Column::new("title", b.title),
                Column::new("author", b.author),
            ])
        }
    }

    #[test]
    fn test_json() {
        let mut w = Vec::new();
        let books = vec![
            Book::new("The Catcher in the Rye", "J.D. Salinger"),
            Book::new("The Adventures of Huckleberry Finn", "Mark Twain"),
        ];
        print(&mut w, books, true, &Format::JSON).unwrap();
        let s = String::from_utf8(w).unwrap();
        assert_eq!(2, s.lines().count());
        for line in s.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(v.is_object());
            let obj = v.as_object().unwrap();
            assert_eq!(obj.len(), 2);
            assert!(obj.contains_key("title"));
            assert!(obj.contains_key("author"));
        }
    }
}
