use std::io::Cursor;

use thiserror::Error;

use crate::ato::{get_language, Language, LinkState};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub language: Option<&'static Language>,
    pub options: Vec<String>,
    pub header: String,
    pub header_encoding: Encoding,
    pub code: String,
    pub code_encoding: Encoding,
    pub footer: String,
    pub footer_encoding: Encoding,
    pub program_arguments: Vec<String>,
    pub input: String,
    pub input_encoding: Encoding,
}

/// See https://github.com/attempt-this-online/attempt-this-online/blob/b694efd9cfaea87d93827e33ec7f5d812a431833/frontend/lib/encoding.ts
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Utf8,
    Sbcs,
    Base64,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid language `{0}`")]
    InvalidLanguage(String),
    #[error("invalid encoding `{0}`")]
    InvalidEncoding(String),
    #[error("invalid JSON for arguments: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("object argument: {0}")]
    ObjectArg(serde_json::Value),
}

impl LinkState {
    pub fn parse(self) -> Result<State, ParseError> {
        let language = if !self.language.is_empty() {
            Some(
                get_language(&self.language)
                    .ok_or_else(|| ParseError::InvalidLanguage(self.language))?,
            )
        } else {
            None
        };
        Ok(State {
            language,
            options: parse_arg_list(self.options)?,
            header: self.header,
            header_encoding: self.header_encoding.try_into()?,
            code: self.code,
            code_encoding: self.code_encoding.try_into()?,
            footer: self.footer,
            footer_encoding: self.footer_encoding.try_into()?,
            program_arguments: parse_arg_list(self.program_arguments)?,
            input: self.input,
            input_encoding: self.input_encoding.try_into()?,
        })
    }
}

// See https://github.com/attempt-this-online/attempt-this-online/blob/b694efd9cfaea87d93827e33ec7f5d812a431833/frontend/components/argvList.tsx
fn parse_arg_list(args: String) -> Result<Vec<String>, ParseError> {
    if args.is_empty() {
        Ok(Vec::new())
    } else {
        let values: Vec<serde_json::Value> =
            serde_json::from_reader(Cursor::new(args.into_bytes()))?;
        let mut args = Vec::with_capacity(values.len());
        for v in values {
            match v {
                serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                    return Err(ParseError::ObjectArg(v))
                }
                _ => args.push(v.to_string()),
            }
        }
        Ok(args)
    }
}

impl TryFrom<String> for Encoding {
    type Error = ParseError;

    fn try_from(enc: String) -> Result<Self, Self::Error> {
        match &*enc {
            "utf-8" | "" => Ok(Encoding::Utf8),
            "sbcs" => Ok(Encoding::Sbcs),
            "base64" => Ok(Encoding::Base64),
            _ => Err(ParseError::InvalidEncoding(enc)),
        }
    }
}
