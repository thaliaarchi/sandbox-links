use std::collections::HashMap;

use lazy_static::lazy_static;
use serde::Deserialize;

pub(crate) const RUN_URL: &'static str = "https://ato.pxeger.com/run";
pub(crate) const LANGUAGES_URL: &'static str = "https://ato.pxeger.com/languages.json";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Language {
    name: String,
    image: String,
    version: String,
    url: String,
    sbcs: bool,
    se_class: Option<String>,
}

pub fn get_languages() -> &'static HashMap<String, Language> {
    lazy_static! {
        static ref LANGUAGES: HashMap<String, Language> = {
            let resp = reqwest::blocking::get(LANGUAGES_URL).unwrap();
            resp.json().unwrap()
        };
    }
    &LANGUAGES
}

pub fn get_language(name: &str) -> Option<&'static Language> {
    get_languages().get(name)
}
