//! Text helpers shared by the UI.

/// Formats a count with its singular or plural noun.
///
/// The plural is used for every count other than exactly one, including zero.
pub fn plural(count: usize, singular: &str, plural: &str) -> String {
    let noun = if count == 1 { singular } else { plural };
    format!("{count} {noun}")
}

#[cfg(test)]
mod tests {
    use super::plural;

    #[test]
    fn zero_uses_plural() {
        assert_eq!(plural(0, "resultado", "resultados"), "0 resultados");
    }

    #[test]
    fn one_uses_singular() {
        assert_eq!(plural(1, "resultado", "resultados"), "1 resultado");
    }

    #[test]
    fn two_uses_plural() {
        assert_eq!(plural(2, "resultado", "resultados"), "2 resultados");
    }
}
