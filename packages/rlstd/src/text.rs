//! A package's on-screen text: `lab::text_table!("<package>.text");` in its
//! text module defines `tr`, `tr_with`, and `tr_key` over the package's
//! `text/en.json` and `text/zh-CN.json` (see `lab::i18n::Table`).

/// The text functions of the text table `$key` (a package's `text`
/// resource): `tr(key)`, its text in the current locale; `tr_with(key, args!(…))`,
/// with its values; and `tr_key(key)`, for a key its data names (a `-text` field).
#[macro_export]
macro_rules! text_table {
    ($key:literal) => {
        static TEXT: $crate::i18n::Table = $crate::i18n::Table::new($key);

        /// The text of `key` in the current locale.
        #[allow(dead_code)]
        pub fn tr(key: &'static str) -> &'static str {
            TEXT.get(key)
        }

        /// The text of `key`, with its values (`args!`).
        #[allow(dead_code)]
        pub fn tr_with(key: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
            TEXT.format(key, args)
        }

        /// The text of a key its data names (a `-text` field).
        #[allow(dead_code)]
        pub fn tr_key(key: &'static str) -> &'static str {
            TEXT.get(key)
        }
    };
}
