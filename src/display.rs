use crate::remote::GetRemoteCliArgs;
use crate::Result;
use std::{collections::HashMap, io::Write};

#[derive(Clone, Debug, Default)]
pub enum Format {
    CSV,
    JSON,
    #[default]
    PIPE,
}

impl From<Format> for u8 {
    fn from(f: Format) -> Self {
        match f {
            Format::CSV => b',',
            Format::PIPE => b'|',
            Format::JSON => 0,
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

#[derive(Builder)]
pub struct Column {
    pub name: String,
    pub value: String,
    #[builder(default)]
    pub optional: bool,
}

impl Column {
    pub fn builder() -> ColumnBuilder {
        ColumnBuilder::default()
    }
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            optional: false,
        }
    }
}

pub fn print<W: Write, D: Into<DisplayBody> + Clone>(
    w: &mut W,
    data: Vec<D>,
    args: GetRemoteCliArgs,
) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }
    match args.format {
        Format::JSON => {
            for d in data {
                let d = d.into();
                let kvs: HashMap<String, String> = d
                    .columns
                    .into_iter()
                    .filter(|c| !c.optional || args.display_optional)
                    .map(|item| (item.name, item.value))
                    .collect();
                writeln!(w, "{}", serde_json::to_string(&kvs)?)?;
            }
        }
        _ => {
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(args.format.into())
                .from_writer(w);
            if !args.no_headers {
                // Get the headers from the first row of columns
                let headers = data[0]
                    .clone()
                    .into()
                    .columns
                    .iter()
                    .filter(|c| !c.optional || args.display_optional)
                    .map(|c| c.name.clone())
                    .collect::<Vec<_>>();
                wtr.write_record(&headers)?;
            }
            for d in data {
                let d = d.into();
                let row = d
                    .columns
                    .into_iter()
                    .filter(|c| !c.optional || args.display_optional)
                    .map(|c| c.value)
                    .collect::<Vec<_>>();
                wtr.write_record(&row)?;
            }
            wtr.flush()?;
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
        let args = GetRemoteCliArgs::builder()
            .no_headers(true)
            .format(Format::JSON)
            .build()
            .unwrap();
        print(&mut w, books, args).unwrap();
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

    #[test]
    fn test_csv_multiple_commas_one_field() {
        let mut w = Vec::new();
        let books = vec![
            Book::new("Faust, Part One", "Goethe"),
            Book::new("The Adventures of Huckleberry Finn", "Mark Twain"),
        ];
        let args = GetRemoteCliArgs::builder()
            .no_headers(true)
            .format(Format::CSV)
            .build()
            .unwrap();
        print(&mut w, books, args).unwrap();
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(w.as_slice());
        assert_eq!(
            "Faust, Part One",
            &reader.records().next().unwrap().unwrap()[0]
        );
    }

    #[derive(Clone)]
    struct BookOptionalColumns {
        pub title: String,
        pub author: String,
        pub isbn: String,
    }

    impl BookOptionalColumns {
        pub fn new(
            title: impl Into<String>,
            author: impl Into<String>,
            isbn: impl Into<String>,
        ) -> Self {
            Self {
                title: title.into(),
                author: author.into(),
                isbn: isbn.into(),
            }
        }
    }

    impl From<BookOptionalColumns> for DisplayBody {
        fn from(b: BookOptionalColumns) -> Self {
            DisplayBody::new(vec![
                Column::new("title", b.title),
                Column::new("author", b.author),
                Column::builder()
                    .name("isbn".to_string())
                    .value(b.isbn)
                    .optional(true)
                    .build()
                    .unwrap(),
            ])
        }
    }

    #[test]
    fn test_csv_optional_columns() {
        let mut w = Vec::new();
        let books = vec![
            BookOptionalColumns::new("The Catcher in the Rye", "J.D. Salinger", "0316769487"),
            BookOptionalColumns::new(
                "The Adventures of Huckleberry Finn",
                "Mark Twain",
                "9780199536559",
            ),
        ];
        let args = GetRemoteCliArgs::builder()
            .format(Format::CSV)
            .build()
            .unwrap();
        print(&mut w, books, args).unwrap();
        assert_eq!(
            "title,author\nThe Catcher in the Rye,J.D. Salinger\nThe Adventures of Huckleberry Finn,Mark Twain\n",
            String::from_utf8(w).unwrap()
        );
    }

    #[test]
    fn test_csv_display_optional_columns_on_args() {
        let mut w = Vec::new();
        let books = vec![
            BookOptionalColumns::new("The Catcher in the Rye", "J.D. Salinger", "0316769487"),
            BookOptionalColumns::new(
                "The Adventures of Huckleberry Finn",
                "Mark Twain",
                "9780199536559",
            ),
        ];
        let args = GetRemoteCliArgs::builder()
            .format(Format::CSV)
            .display_optional(true)
            .build()
            .unwrap();
        print(&mut w, books, args).unwrap();
        assert_eq!(
            "title,author,isbn\nThe Catcher in the Rye,J.D. Salinger,0316769487\nThe Adventures of Huckleberry Finn,Mark Twain,9780199536559\n",
            String::from_utf8(w).unwrap()
        );
    }
}
