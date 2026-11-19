//! Replay self-play records and reject illegal moves or inconsistent outcomes.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{env, process};

use engine::{Color, Status};
use selfplay::corpus::read_corpus;
use selfplay::game::{Outcome, WinReason};
use selfplay::opening::{OpeningAnswer, OpeningProvenance};
use selfplay::record::GameRecord;
use serde::Serialize;

const USAGE: &str = "Usage: selfplayverify --corpus <DIR>";

#[derive(Serialize)]
struct OutcomeCounts {
    black: usize,
    white: usize,
    draw: usize,
}

#[derive(Serialize)]
struct Summary {
    game_count: usize,
    verified_plies: usize,
    certified_suffixes: usize,
    games_ending_within_16_plies: usize,
    min_plies: usize,
    max_plies: usize,
    mean_plies: f64,
    median_plies: f64,
    length_histogram_plies: BTreeMap<usize, usize>,
    opening_offers: BTreeMap<String, usize>,
    opening_answers: BTreeMap<String, usize>,
    outcomes: OutcomeCounts,
}

impl Summary {
    fn from_records(records: &[GameRecord]) -> Self {
        let mut lengths: Vec<usize> = records.iter().map(|record| record.plies.len()).collect();
        lengths.sort();

        let game_count = records.len();
        let verified_plies = lengths.iter().sum();
        let min_plies = *lengths.first().expect("non-empty after empty check");
        let max_plies = *lengths.last().expect("non-empty after empty check");
        let mean_plies = verified_plies as f64 / game_count as f64;
        let median_plies = if game_count % 2 == 1 {
            lengths[game_count / 2] as f64
        } else {
            let mid = game_count / 2;
            (lengths[mid - 1] + lengths[mid]) as f64 / 2.0
        };
        let games_ending_within_16_plies = lengths.iter().filter(|len| **len <= 16).count();

        let mut length_histogram_plies = BTreeMap::new();
        for length in &lengths {
            *length_histogram_plies.entry(*length).or_insert(0) += 1;
        }

        let mut opening_offers = BTreeMap::new();
        let mut opening_answers = BTreeMap::new();
        let mut outcomes = OutcomeCounts {
            black: 0,
            white: 0,
            draw: 0,
        };
        let mut certified_suffixes = 0;

        for record in records {
            let offer_key = match &record.header.opening.provenance {
                OpeningProvenance::Generated => "Generated".to_owned(),
                OpeningProvenance::Listed { list, index } => format!("{list}#{index}"),
            };
            *opening_offers.entry(offer_key).or_insert(0) += 1;

            let answer_key = match record.header.opening.answer {
                OpeningAnswer::Take(Color::Black) => "take_black",
                OpeningAnswer::Take(Color::White) => "take_white",
                OpeningAnswer::AddTwo {
                    final_choice: Color::Black,
                    ..
                } => "add_two_take_black",
                OpeningAnswer::AddTwo {
                    final_choice: Color::White,
                    ..
                } => "add_two_take_white",
            };
            *opening_answers.entry(answer_key.to_owned()).or_insert(0) += 1;

            match record.outcome {
                Outcome::Won { winner, .. } => match winner {
                    Color::Black => outcomes.black += 1,
                    Color::White => outcomes.white += 1,
                },
                Outcome::Draw => outcomes.draw += 1,
            }

            if record.certified_suffix.is_some() {
                certified_suffixes += 1;
            }
        }

        Self {
            game_count,
            verified_plies,
            certified_suffixes,
            games_ending_within_16_plies,
            min_plies,
            max_plies,
            mean_plies,
            median_plies,
            length_histogram_plies,
            opening_offers,
            opening_answers,
            outcomes,
        }
    }
}

fn parse_corpus<I, S>(args: I) -> Result<PathBuf, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut corpus = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        let arg = arg.as_ref();
        match arg {
            "--corpus" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--corpus requires a value".to_owned())?;
                corpus = Some(PathBuf::from(value.as_ref()));
            }
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    corpus.ok_or_else(|| "--corpus is required".to_owned())
}

fn verify_record(game_index: usize, record: &GameRecord) -> Result<(), String> {
    let mut board = record
        .header
        .opening
        .board()
        .map_err(|error| format!("game {game_index}: opening replay failed: {error}"))?;

    for (ply, recorded) in record.plies.iter().enumerate() {
        board.play(recorded.mv).map_err(|error| {
            format!(
                "game {game_index}: move {:?} at ply {ply} was rejected: {error}",
                recorded.mv
            )
        })?;
    }

    let status = board.status();
    let matches = match record.outcome {
        Outcome::Won {
            winner,
            reason: WinReason::FiveOrMore,
        } => status == Status::Won(winner),
        Outcome::Won {
            winner,
            reason: WinReason::Resignation(..),
        } => status == Status::Ongoing && board.to_move() == winner.other(),
        Outcome::Draw => status == Status::Draw,
    };

    if !matches {
        return Err(format!(
            "game {game_index}: final status {status:?} does not match recorded outcome {:?}",
            record.outcome
        ));
    }

    Ok(())
}

fn run(corpus_dir: PathBuf) -> Result<(), String> {
    let records =
        read_corpus(&corpus_dir).map_err(|error| format!("could not read corpus: {error}"))?;

    if records.is_empty() {
        return Err("corpus contains no game records".to_owned());
    }

    for (game_index, record) in records.iter().enumerate() {
        verify_record(game_index, record)?;
    }

    let summary = Summary::from_records(&records);
    let json = serde_json::to_string_pretty(&summary)
        .map_err(|error| format!("could not serialize summary: {error}"))?;
    println!("{json}");

    Ok(())
}

fn main() {
    match parse_corpus(env::args().skip(1)) {
        Ok(corpus_dir) => {
            if let Err(error) = run(corpus_dir) {
                eprintln!("selfplayverify failed: {error}");
                process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            process::exit(2);
        }
    }
}
