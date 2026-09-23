//! Interactive guessing game from *The Rust Programming Language*.
//!
//! The tests exercise this application through its standard streams, leaving
//! the application itself as a single, ordinary `main` function.

use rand::Rng;
use std::cmp::Ordering;
use std::io::{self, BufRead, Write};

/// Pick a secret number and interact with the player until they guess it.
fn main() {
    // Use an inclusive range so every valid secret is 1 through 10.
    let secret_number = rand::thread_rng().gen_range(1..=10);

    // Lock each standard stream once for the duration of the game. Direct
    // `Write` calls also ensure the system tests exercise the redirected file
    // descriptors rather than Rust's test-output capture mechanism.
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    writeln!(output, "Guess the number!").expect("could not write game output");

    loop {
        writeln!(output, "Please input your guess.").expect("could not write game output");
        output.flush().expect("could not flush game output");

        let mut guess = String::new();
        let bytes_read = input
            .read_line(&mut guess)
            .expect("failed to read the guess");
        // EOF cannot produce a winning guess and would otherwise spin forever.
        assert!(bytes_read > 0, "input ended before the number was guessed");

        // As in the book, malformed input is ignored and prompts another turn.
        let guess: u32 = match guess.trim().parse() {
            Ok(number) => number,
            Err(_) => continue,
        };

        writeln!(output, "You guessed: {guess}").expect("could not write game output");

        match guess.cmp(&secret_number) {
            Ordering::Less => writeln!(output, "Too small!").expect("could not write game output"),
            Ordering::Greater => writeln!(output, "Too big!").expect("could not write game output"),
            Ordering::Equal => {
                writeln!(output, "You win!").expect("could not write game output");
                break;
            }
        }
    }
}

/// End-to-end tests that drive the unmodified `main` function through pipes.
#[cfg(test)]
mod tests {
    use super::*;
    use ptyknot::{pty_stdin, pty_stdout};
    use rand::seq::SliceRandom;
    use std::io::BufReader;

    /// Read one protocol line and compare it without its line terminator.
    fn assert_next_line<R: BufRead>(input: &mut R, expected: &str) {
        let mut line = String::new();
        input
            .read_line(&mut line)
            .expect("could not read game output");
        assert_eq!(line.trim_end(), expected);
    }

    /// Send one numeric guess and return the ordering reported by the game.
    ///
    /// A losing turn must end with the next prompt. A winning turn ends the
    /// game immediately, so no further prompt is expected in that case.
    fn submit_guess<R: BufRead, W: Write>(input: &mut R, output: &mut W, guess: u32) -> Ordering {
        writeln!(output, "{guess}").expect("could not send guess");
        assert_next_line(input, &format!("You guessed: {guess}"));

        let mut result = String::new();
        input
            .read_line(&mut result)
            .expect("could not read guess result");
        let answer = match result.trim_end() {
            "Too small!" => Ordering::Less,
            "Too big!" => Ordering::Greater,
            "You win!" => Ordering::Equal,
            result => panic!("unexpected guess result: {result}"),
        };

        if answer != Ordering::Equal {
            assert_next_line(input, "Please input your guess.");
        }
        answer
    }

    /// Check every recorded answer after the winning number becomes known.
    ///
    /// This retrospective check detects a game that reaches the right answer
    /// while having incorrectly classified an earlier guess.
    fn assert_answers_are_consistent(history: &[(u32, Ordering)]) {
        let winners: Vec<_> = history
            .iter()
            .filter(|(_, answer)| *answer == Ordering::Equal)
            .collect();
        assert_eq!(winners.len(), 1, "game must have exactly one winning guess");
        let secret_number = winners[0].0;

        for &(guess, answer) in history {
            assert_eq!(
                answer,
                guess.cmp(&secret_number),
                "answer for guess {guess} is inconsistent with secret number {secret_number}"
            );
        }
    }

    /// Find the secret efficiently and validate the entire resulting history.
    #[test]
    fn binary_search_answers_are_consistent() {
        // Run `main` unchanged in a child whose standard streams are connected
        // to the handles returned to this test.
        ptyknot::ptyknot!(
            knot,
            main,
            < game_output pty_stdout(),
            > game_input pty_stdin()
        );

        let mut game_output = BufReader::new(game_output);

        assert_next_line(&mut game_output, "Guess the number!");
        assert_next_line(&mut game_output, "Please input your guess.");

        // Confirm that malformed input is ignored without producing a result.
        writeln!(game_input, "not a number").expect("could not send invalid guess");
        assert_next_line(&mut game_output, "Please input your guess.");

        // Use each response only to narrow the search. Correctness of all
        // responses is checked independently once the winner is known.
        let (mut low, mut high) = (1, 10);
        let mut history = Vec::new();
        loop {
            let guess = (low + high) / 2;
            let answer = submit_guess(&mut game_output, &mut game_input, guess);
            history.push((guess, answer));

            match answer {
                Ordering::Less => low = guess + 1,
                Ordering::Greater => high = guess - 1,
                Ordering::Equal => break,
            }
        }

        assert_answers_are_consistent(&history);
        // Dropping the knot waits for the child to exit.
        drop(knot);
    }

    /// Try the first ten values in random order and validate every response.
    #[test]
    fn random_permutation_answers_are_consistent() {
        // A fresh child gets a fresh random secret and independent streams.
        ptyknot::ptyknot!(
            knot,
            main,
            < game_output pty_stdout(),
            > game_input pty_stdin()
        );

        let mut game_output = BufReader::new(game_output);
        assert_next_line(&mut game_output, "Guess the number!");
        assert_next_line(&mut game_output, "Please input your guess.");

        // A permutation of the complete domain is guaranteed to discover the
        // secret, regardless of the order chosen by the random shuffle.
        let mut guesses: Vec<u32> = (1..=10).collect();
        guesses.shuffle(&mut rand::thread_rng());

        // Stop when the application stops accepting guesses, retaining the
        // complete conversation for the retrospective consistency check.
        let mut history = Vec::new();
        for guess in guesses {
            let answer = submit_guess(&mut game_output, &mut game_input, guess);
            history.push((guess, answer));
            if answer == Ordering::Equal {
                break;
            }
        }

        assert_answers_are_consistent(&history);
        // Dropping the knot reaps the completed child process.
        drop(knot);
    }
}
