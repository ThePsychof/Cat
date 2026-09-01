pub fn app_name() -> &'static str {
    "Cat"
}

#[cfg(test)]
mod tests {
    use super::app_name;

    #[test]
    fn gui_identity_is_cat() {
        assert_eq!(app_name(), "Cat");
    }
}
