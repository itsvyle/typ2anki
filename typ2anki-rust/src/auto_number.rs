use std::{
    io::{self, Read},
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use crate::{
    card_wrapper::{BarebonesCardInfo, CardInfo},
    config,
    output::{OutputManager, OutputMessage},
    parse_file::{self},
};
use anyhow::Context as _;
use regex::Regex;
use time::OffsetDateTime;

static SIX_DIGIT_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^\d{6}$"#).unwrap());
static TEN_DIGIT_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^\d{10}$"#).unwrap());

/// Detect "YYmmdd" patterns for id (or empty string), and auto increment with the current time, handling conflicts; only does it on non empty cards
/// can be ran as a formatter basically, on the file given in config.auto_number_file
/// Either does "previous minute + 1", or "current time" - in all cases, it's deduplicated by incrementing minute further if needed
/// I recommend using dprint plugin exec - you can put this *before* typstyle in the order of execution
pub fn run_auto_number(output: impl OutputManager + 'static) -> anyhow::Result<()> {
    let output = Arc::new(output);
    let cfg = config::get();
    let file_path: PathBuf = cfg
        .auto_number_file
        .clone()
        .context("auto_number is not set in config")?
        .into();
    let mut contents = get_file_contents(file_path.to_str().context("Invalid file path")?)?;
    let mut cards = parse_file::parse_cards_string(&contents, &output, false)
        .into_iter()
        .map(|f| CardInfo::from_string(0, &f, file_path.clone()))
        .filter_map(|f| match f {
            Ok(card) => Some(card),
            Err(e) => {
                output.send(OutputMessage::ParsingError(format!(
                    "Error parsing card for auto_number: {}",
                    e
                )));
                None
            }
        })
        .filter_map(|c| {
            let deck = c.deck_name.clone();
            let id = c.card_id.clone();
            if c.is_empty() {
                Some((
                    c,
                    BarebonesCardInfo {
                        card_id: id,
                        deck_name: deck,
                        question: "".to_string(),
                        answer: "".to_string(),
                        byte_range: (0, 0),
                        prelude_range: None,
                    },
                ))
            } else {
                match c.to_barebones() {
                    Ok(b) => Some((c, b)),
                    Err(e) => {
                        output.send(OutputMessage::ParsingError(format!(
                            "Error converting card to barebones for auto_number: {}",
                            e
                        )));
                        None
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let current_date = {
        let now = OffsetDateTime::now_local().unwrap();
        format!(
            "{:02}{:02}{:02}{:02}{:02}",
            now.year() % 100,
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute()
        )
    };

    let mut prev_id: String = "".to_string();

    for i in 0..cards.len() {
        let must_id = !cards[i].1.is_empty() && is_to_autoid(&cards[i].0.card_id);
        if must_id {
            let mut new_id;
            if cards[i].0.card_id.is_empty() {
                new_id = current_date.clone();
            } else if TEN_DIGIT_NUMBER.is_match(&prev_id) {
                new_id = increment_date_id(&prev_id);
            } else {
                new_id = current_date.clone();
            }

            for _ in 0..1000 {
                let conflict = cards.iter().any(|c| c.0.card_id == new_id);
                if conflict {
                    new_id = increment_date_id(&new_id);
                } else {
                    break;
                }
            }
            contents = contents.replacen(
                &format!("id: \"{}\"", cards[i].0.card_id),
                &format!("id: \"{}\"", new_id),
                1,
            );

            cards[i].0.card_id = new_id;
        }
        prev_id = cards[i].0.card_id.clone();
    }
    print!("{contents}");
    Ok(())
}

fn get_file_contents(path: &str) -> anyhow::Result<String> {
    if path == "stdin" {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        return Ok(input);
    }
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file contents from path: {}", path))?;
    Ok(contents)
}

fn is_to_autoid(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    SIX_DIGIT_NUMBER.is_match(s)
}

fn increment_date_id(s: &str) -> String {
    if s.len() != 10 {
        return s.to_string();
    }
    let mut hour: u8 = s[6..8].parse::<u8>().unwrap_or(0);
    let mut minute: u8 = s[8..10].parse::<u8>().unwrap_or(0);

    if minute == 59 {
        minute = 0;
        hour += 1;
    } else {
        minute += 1;
    }
    format!("{}{:02}{:02}", &s[0..6], hour, minute)
}
