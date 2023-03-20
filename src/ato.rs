//! A decoder and encoder for Attempt This Online code share links.
//!
//! Supports schema versions 0 and 1, and is based on the implementation as of
//! commit [b694efd](https://github.com/attempt-this-online/attempt-this-online/blob/b694efd9cfaea87d93827e33ec7f5d812a431833/frontend/lib/urls.ts)
//! (2023-03-19).

use std::io::{self, BufRead, Read};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

use flate2::{
    bufread::{DeflateDecoder, DeflateEncoder},
    Compression,
};
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub schema: Schema,
    pub language: String,
    pub options: String,
    pub header: String,
    pub header_encoding: String,
    pub code: String,
    pub code_encoding: String,
    pub footer: String,
    pub footer_encoding: String,
    pub program_arguments: String,
    pub input: String,
    pub input_encoding: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Schema {
    V0,
    #[default]
    V1,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("URL parse: {0}")]
    Url(#[from] url::ParseError),
    #[error("invalid key `{0}` in query string")]
    InvalidKey(String),
    #[error("multiple schema versions")]
    MultipleVersions,
    #[error("base-64 decode: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("DEFLATE decompress: {0}")]
    Deflate(#[from] io::Error),
    #[error("MessagePack deserialize: {0}")]
    MessagePack(#[from] rmp_serde::decode::Error),
}

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("MessagePack serialize: {0}")]
    MessagePack(#[from] rmp_serde::encode::Error),
    #[error("DEFLATE compress: {0}")]
    Deflate(#[from] io::Error),
}

impl State {
    pub fn new() -> Self {
        State::default()
    }

    /// Decode an Attempt This Online share link.
    pub fn decode(url: &str) -> Result<Self, DecodeError> {
        let (data, language) = State::decode_url(url)?;
        let mut state = match data {
            Some((schema, data)) => State::serialize_mp(schema, &*data)?,
            None => State::default(),
        };
        match language {
            Some(l) if state.language.is_empty() => state.language = l,
            _ => {}
        }
        Ok(state)
    }

    /// Encode an Attempt This Online share link.
    pub fn encode(&self) -> Result<String, EncodeError> {
        let mp = self.deserialize_mp()?;
        State::encode_url(self.schema, &*mp, Compression::best())
    }

    /// Decode and decompress an Attempt This Online share link.
    fn decode_url(url: &str) -> Result<(Option<(Schema, Vec<u8>)>, Option<String>), DecodeError> {
        let u = Url::parse(url).map_err(DecodeError::Url)?;
        let mut data = None;
        let mut language = None;
        for (key, value) in u.query_pairs() {
            let schema = match &*key {
                "0" => Schema::V0,
                "1" => Schema::V1,
                "L" | "l" => {
                    // See https://github.com/attempt-this-online/attempt-this-online/blob/b694efd9cfaea87d93827e33ec7f5d812a431833/frontend/pages/run.tsx#L237-L269
                    language = Some(value.into_owned());
                    continue;
                }
                _ => return Err(DecodeError::InvalidKey(key.into_owned())),
            };
            if data.is_some() {
                // ATO chooses the maximum schema version, when multiple are
                // provided, but that should never be generated.
                return Err(DecodeError::MultipleVersions);
            }
            data = Some((schema, value));
        }
        let data = if let Some((schema, data)) = data {
            let compressed = URL_SAFE_NO_PAD.decode(&*data)?;
            let mut buf = Vec::new();
            DeflateDecoder::new(&*compressed).read_to_end(&mut buf)?;
            Some((schema, buf))
        } else {
            None
        };
        Ok((data, language))
    }

    /// Encode and compress an Attempt This Online share link.
    fn encode_url<R: BufRead>(
        schema: Schema,
        r: R,
        level: Compression,
    ) -> Result<String, EncodeError> {
        let mut z = DeflateEncoder::new(r, level);
        let mut d = Vec::new();
        z.read_to_end(&mut d)?;
        let mut b = URL_SAFE_NO_PAD.encode(&d);
        match schema {
            Schema::V0 => b.insert_str(0, "0="),
            Schema::V1 => b.insert_str(0, "1="),
        }
        let mut u = Url::parse("https://ato.pxeger.com/run").unwrap();
        u.set_query(Some(&b));
        Ok(u.to_string())
    }

    /// Serialize as MessagePack format
    fn serialize_mp(schema: Schema, data: &[u8]) -> Result<Self, DecodeError> {
        match schema {
            Schema::V0 => {
                let data: [String; 9] = rmp_serde::from_read(data)?;
                let [language, header, header_encoding, code, code_encoding, footer, footer_encoding, input, input_encoding] =
                    data;
                Ok(State {
                    schema,
                    language,
                    options: String::new(),
                    header,
                    header_encoding,
                    code,
                    code_encoding,
                    footer,
                    footer_encoding,
                    program_arguments: String::new(),
                    input,
                    input_encoding,
                })
            }
            Schema::V1 => {
                let data: [String; 11] = rmp_serde::from_read(data)?;
                let [language, options, header, header_encoding, code, code_encoding, footer, footer_encoding, program_arguments, input, input_encoding] =
                    data;
                Ok(State {
                    schema,
                    language,
                    options,
                    header,
                    header_encoding,
                    code,
                    code_encoding,
                    footer,
                    footer_encoding,
                    program_arguments,
                    input,
                    input_encoding,
                })
            }
        }
    }

    /// Deserialize from MessagePack format
    fn deserialize_mp(&self) -> Result<Vec<u8>, EncodeError> {
        match self.schema {
            Schema::V0 => Ok(rmp_serde::to_vec(&[
                &self.language,
                &self.header,
                &self.header_encoding,
                &self.code,
                &self.code_encoding,
                &self.footer,
                &self.footer_encoding,
                &self.input,
                &self.input_encoding,
            ])?),
            Schema::V1 => Ok(rmp_serde::to_vec(&[
                &self.language,
                &self.options,
                &self.header,
                &self.header_encoding,
                &self.code,
                &self.code_encoding,
                &self.footer,
                &self.footer_encoding,
                &self.program_arguments,
                &self.input,
                &self.input_encoding,
            ])?),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_v0() {
        // The only v0 link on Code Golf
        // https://codegolf.stackexchange.com/questions/111613/pick-a-random-number-between-0-and-n-using-a-constant-source-of-randomness/229048#229048
        let url = "https://ato.pxeger.com/run?0=ZVI9T8MwEJUYw584YCCpWpRQPqoWWJjYGZEq1760Fo4d7AuImV_BxgJ_hxl-DRcnopFqyb7hvXfP7-z3r_qVNs5-lnAND58NlZPZz9SIaqUE2LkXVvFeY1pcXU2LbGRvbqbFovSughbjoqvaeRp10t-9_YgFEqQDaRl6HOpACp-TiEpnDErSzv7Dt66xhD5JjoAwEKBaI0gRMKm9tpT2eFqmRQal87AEbaG_Wp5lWZIoLKM2NbrSNAbNfBFNsnkCvLpOu2gEJeffmkTSjtFAw36t6gjuN5pDBBDGwB14rLBaoYeYs3odDgJVI6P6hInK2WOCR-teYMObHIc1sjGCEATUk2dhGuwtGAyRtBHEB4Lijl6vmrZZ610K7RdQIhr2RWwFQqmOr-3BIPzh9xtcH47710jlSfQJaTvAAatCEhNu9dSglRrDnCVKy-0zDIU8ijj24nR6dn5xORtDkefLPM-z7k98dOUP";
        let state = State {
            schema: Schema::V0,
            language: "python".into(),
            options: "".into(),
            header: "f = \\".into(),
            header_encoding: "utf-8".into(),
            code: "lambda n:randrange(1<<31)*n>>31;from random import*".into(),
            code_encoding: "utf-8".into(),
            footer: "from statistics import pstdev\nfrom collections import Counter\n\n# test edge case\nprint(Counter(f(1) for _ in range(10)))\n\ndef test(limit, iterations):\n    print(limit, iterations)\n    c = Counter(f(limit) for _ in range(iterations))\n\n    # This is all I remember from my statistics education. I don't know how to calculate a p-value\n    # to show that the distribution is fair; feel free to add that in!\n    print(\"σ =\", pstdev(c.values()))\n    print(\"meta-frequencies:\", dict(Counter(c.values())))\n\ntest(12345678, 100_000)".into(),
            footer_encoding: "utf-8".into(),
            program_arguments: "".into(),
            input: "".into(),
            input_encoding: "utf-8".into(),
        };
        assert_eq!(state, State::decode(url).unwrap());
        assert_eq!(url, state.encode().unwrap());
    }

    #[test]
    fn roundtrip_v1() {
        // A randomly selected v1 link from Code Golf
        // https://codegolf.stackexchange.com/questions/233529/could-you-massage-this-stack-for-me/233580#233580
        let url = "https://ato.pxeger.com/run?1=m724qjhjWbSSbqpS7E07ZQVH25D8oNSS0qI8LmUFN1u3ovxcONfd1qW0ICczObEkFcgLtg0uTywAMiJsXYryC7jsMpeWlqTpWtwMS03OyFdQ0UjLL1KosLGxceQCsSoVNBJsMhM0NYAi7jEKjtZgaetqINfNWkOjQrdSU9MayAkGytVq1hTnF5VwwUxwi1GI0ISYvgBCrYxWMlTSUTJSil1qyGXEZQgRBQA";
        let state = State {
            schema: Schema::V1,
            language: "zsh".into(),
            options: "[\"-e\"]".into(),
            header: "# A=ToReturn\n# F=FromReturn\n# G=Duplicate\n# S=Swap\n# X=Drop\n>i".into(),
            header_encoding: "utf-8".into(),
            code: "echo $(for x<<<A\nfor y (`<i`)(<<<G\\ A;for x;{<<<F;((x-y));<<<S\\ A})|sort\nfor x<<<F\\ X)".into(),
            code_encoding: "utf-8".into(),
            footer: "".into(),
            footer_encoding: "utf-8".into(),
            program_arguments: "[\"1\",\"2\"]".into(),
            input: "1\n2\n1".into(),
            input_encoding: "utf-8".into(),
        };
        assert_eq!(state, State::decode(url).unwrap());
        assert_eq!(url, state.encode().unwrap());

        // A link with code encoded in SBCS
        // https://codegolf.stackexchange.com/questions/60443/s%e1%b4%8d%e1%b4%80%ca%9f%ca%9f-c%e1%b4%80%e1%b4%98%ea%9c%b1-c%e1%b4%8f%c9%b4%e1%b4%a0%e1%b4%87%ca%80%e1%b4%9b%e1%b4%87%ca%80/251892#251892
        let url = "https://ato.pxeger.com/run?1=m700KzUnp3LBgqWlJWm6FjfrHzXMedS47-GO7oc7ttu7WNgem3mq6cSSQ4sPbYo7Mu3hrh2emocWpkYf2npoA0jZzkVeWr5HFpycfGjJwx1NRxY-atxb5up6ctPDXQsPrXOv1jjacGjzo8bdh3YeWxsC1Pxo47pHDTMf7mwG2nJiqTGIvbuH6_CMxKN7Di1yqwRKLylOSi6GOmZ9tJIH0HH5CuH5RTkpSrFQYQA";
        let state = State {
            schema: Schema::V1,
            language: "jelly".into(),
            options: "".into(),
            header: "".into(),
            header_encoding: "utf-8".into(),
            code: "“⁾ḋḷ?D8=ƙʂȤ£²^ĖẸI)¡e[µ°⁾ṢJ*MĠɓ¤Ḃġ⁽vEEɲạ®G{(ŀ³⁻¹ƭTẸⱮ’ṃ“ȥ3’Ọ\nØaż¢FyⱮ".into(),
            code_encoding: "sbcs".into(),
            footer: "".into(),
            footer_encoding: "utf-8".into(),
            program_arguments: "[\"Hello World\"]".into(),
            input: "".into(),
            input_encoding: "utf-8".into(),
        };
        assert_eq!(state, State::decode(url).unwrap());
        assert_eq!(url, state.encode().unwrap());

        // A link with code encoded in base-64
        // https://codegolf.stackexchange.com/questions/249373/draw-the-progress-pride-flag/249394#249394
        let url = "https://ato.pxeger.com/run?1=NVDLTsMwALv3X9DWdBrhwCEoFYQsfZBW0BPK2i59QaKlpUt_hcsu45_4Cn6BFTFLvtiWJfvzlL_KPD8eT0O_u4LfPxmoqrzWNuKbBUJ40ZwJEUJ1LVEa3awQmRYX9KwavOiuZVWM5kgIWiOavs-eu4E-SO2UXmHFy1NHsW9IhwDB_hgkrQya3BJM3ICjeoN9QJP-besRdS6Asxdasxe8-9NEsxxDoKDDEuSGidlToHThPeriPlWMHyy1UpXYrbJ3pgRfTmzK9JyJcDoGGALqyY8Qx2vB4YElRNOGGYfZ9noXq9uvrTDlevU__3LDLw";
        let state = State {
            schema: Schema::V1,
            language: "c_gcc".into(),
            options: "".into(),
            header: "".into(),
            header_encoding: "utf-8".into(),
            code: "Y2hhcipyPSL/AAD/jAD/8AAAiigAUP94AIz/////////tMhu3PBkMhQAAAAiO2ksajttYWluKHgp\ne3dyaXRlKDEsIlA2IDEwNTkgNjcyIDI1NSAiLDE2KTtmb3IoO2k8NjcyOysraSlmb3Ioaj0wO2o8\nMTA1OTsrK2opd3JpdGUoMSxyKygoeD1hYnMoaS0zMzYpK2opPDUwND82K3gvODQ6aS8xMTIpKjMs\nMyk7fQo=".into(),
            code_encoding: "base64".into(),
            footer: "".into(),
            footer_encoding: "utf-8".into(),
            program_arguments: "".into(),
            input: "".into(),
            input_encoding: "utf-8".into(),
        };
        assert_eq!(state, State::decode(url).unwrap(),);
        assert_eq!(url, state.encode().unwrap());
    }

    #[test]
    fn roundtrip_all() {
        let links = include_str!("../tests/ato_links.txt");
        let mut compression_differs = 0usize;
        for link in links.lines() {
            let state = State::decode(link).unwrap();
            let encoded = state.encode().unwrap();
            if encoded != link {
                let (data, language) = State::decode_url(link).unwrap();
                if let Some((schema, decoded_raw)) = data {
                    compression_differs += 1;
                    let encoded_raw = state.deserialize_mp().unwrap();
                    assert_eq!(state.schema, schema);
                    assert_eq!(decoded_raw, encoded_raw);
                } else if let Some(l) = language {
                    assert_eq!(state.language, l);
                }
            }
        }
        if compression_differs != 0 {
            eprintln!("Compression differs for {compression_differs} links");
        }
    }
}
