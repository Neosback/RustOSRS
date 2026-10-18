//! Reading and writing the name book (`osrs_core::object_names`) as JSON, and turning it into the
//! names the building census reads.

use osrs_core::object_names::{NameSource, ObjectNames};

pub fn name_book_from_json(text: &str) -> Result<ObjectNames, String> {
    let book: ObjectNames = serde_json::from_str(text).map_err(|e| e.to_string())?;
    book.validate()?;
    Ok(book)
}

pub fn name_book_to_json(book: &ObjectNames) -> Result<String, String> {
    serde_json::to_string_pretty(book).map_err(|e| e.to_string())
}

/// The name and symbol the census should read for object `id`: a name a person gave wins over
/// the cache's, and the role they chose (door, window...) is added as a word, so `says("door")`
/// finds it even when the name does not contain the word.
pub fn census_name(
    book: &ObjectNames,
    id: u32,
    cache_name: &str,
    symbol: &str,
) -> (String, String) {
    let Some(entry) = book.get(id) else {
        return (cache_name.to_lowercase(), symbol.to_owned());
    };
    let mut name = cache_name.to_lowercase();
    if matches!(entry.source, Some(NameSource::You | NameSource::Accepted))
        && let Some(own) = &entry.name
    {
        name = own.to_lowercase();
    }
    let mut symbol = symbol.to_owned();
    if let Some(role) = &entry.role {
        symbol = format!("{symbol} {role}").trim().to_owned();
    }
    (name, symbol)
}

/// What a person wrote, as the census wants it: the name (in lower case) and the role as the
/// symbol, so `says("door")` finds either. Only objects a person named or accepted appear.
pub fn named_objects(book: &ObjectNames) -> Vec<(u32, String, String)> {
    book.entries
        .iter()
        .filter(|(_, e)| matches!(e.source, Some(NameSource::You | NameSource::Accepted)))
        .map(|(id, e)| {
            (
                *id,
                e.name.clone().unwrap_or_default().to_lowercase(),
                e.role.clone().unwrap_or_default(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::object_names::Witness;

    #[test]
    fn a_name_and_role_a_person_gave_reach_the_census() {
        let witness = Witness::default();
        let mut book = ObjectNames::new(0);
        assert_eq!(
            census_name(&book, 5, "Null", "loc.00"),
            ("null".into(), "loc.00".into())
        );
        book.set_name(5, "Shutter", witness.clone());
        book.set_role(5, "window", witness);
        let (name, symbol) = census_name(&book, 5, "null", "");
        assert_eq!(name, "shutter");
        assert!(symbol.contains("window"));
    }

    #[test]
    fn a_book_round_trips_through_json_and_a_stranger_file_is_refused() {
        let witness = Witness {
            cache_name: "null".into(),
            symbol: "loc.00".into(),
        };
        let mut book = ObjectNames::new(240);
        book.set_name(9, "Window", witness.clone());
        book.set_role(9, "window", witness.clone());
        book.set_family(9, "brick town", witness);
        let text = name_book_to_json(&book).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            name_book_from_json(&text).unwrap_or_else(|e| panic!("{e}")),
            book
        );
        assert!(name_book_from_json("{\"format\":\"x\",\"version\":1}").is_err());
        assert!(name_book_from_json("not json").is_err());
        let named = named_objects(&book);
        assert_eq!(named, vec![(9, "window".to_owned(), "window".to_owned())]);
    }
}
