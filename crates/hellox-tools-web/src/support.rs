pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::collapse_whitespace;

    #[test]
    fn whitespace_collapsing_is_stable() {
        assert_eq!(collapse_whitespace(" hello \n  world\t! "), "hello world !");
    }
}
