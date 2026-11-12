//! Stable animal aliases shared by player identity and matchmaking.

pub const ANIMAL_NAMES: [&str; 40] = [
    "Lion",
    "Tiger",
    "Leopard",
    "Wolf",
    "Fox",
    "Raccoon",
    "Bear",
    "Koala",
    "Panda",
    "Boar",
    "Unicorn",
    "Eagle",
    "Owl",
    "Crocodile",
    "Dinosaur",
    "Dragon",
    "Shark",
    "Scorpion",
    "Buffalo",
    "Ram",
    "Elephant",
    "Rhino",
    "Gorilla",
    "Horse",
    "Deer",
    "Bat",
    "Swan",
    "Flamingo",
    "Snake",
    "Turtle",
    "Octopus",
    "Dolphin",
    "Bee",
    "Butterfly",
    "Spider",
    "Otter",
    "Beaver",
    "Camel",
    "Kangaroo",
    "Badger",
];

/// Build one globally stable alias with an ASCII four-digit suffix.
pub fn animal_alias(animal_index: usize, suffix: usize) -> String {
    format!(
        "{}{:04}",
        ANIMAL_NAMES[animal_index % ANIMAL_NAMES.len()],
        suffix % 10_000
    )
}

#[cfg(test)]
mod tests {
    use super::{ANIMAL_NAMES, animal_alias};
    use std::collections::HashSet;

    #[test]
    fn animal_aliases_are_unique_safe_and_four_digit() {
        assert_eq!(ANIMAL_NAMES.len(), 40);
        let mut seen = HashSet::with_capacity(ANIMAL_NAMES.len());

        for (index, animal) in ANIMAL_NAMES.iter().enumerate() {
            assert!(
                seen.insert(animal.to_ascii_lowercase()),
                "duplicate animal: {animal}"
            );
            let alias = animal_alias(index, 42);
            assert_eq!(alias, format!("{animal}0042"));
            assert!(alias.chars().count() <= crate::name_policy::MAX_CHARS);
            assert!(
                crate::name_policy::is_allowed(&alias),
                "name rejected: {alias}"
            );
        }

        assert_eq!(animal_alias(ANIMAL_NAMES.len(), 10_042), "Lion0042");
    }
}
